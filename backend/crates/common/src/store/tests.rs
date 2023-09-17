use crate::api_types::NewDivergenceInvestigationRequest;
use crate::block_choice::BlockChoicePolicy;
use crate::test_utils::{fast_rng, gen::gen_indexers};
use crate::{
    queries,
    store::{diesel_queries, PoiLiveness, Store},
};
use diesel::Connection;
use std::collections::BTreeSet;

fn test_db_url() -> String {
    std::env::var("GRAPHIX_TEST_DB_URL").expect("GRAPHIX_TEST_DB_URL must be set to run tests")
}

#[tokio::test]
async fn no_deployments_at_first() {
    let store = Store::new(&test_db_url()).await.unwrap();
    let initial_deployments = store.sg_deployments().unwrap();
    assert!(initial_deployments.is_empty());
}

#[tokio::test]
#[should_panic] // FIXME
async fn deployments_with_name() {
    let store = Store::new(&test_db_url()).await.unwrap();

    let ipfs_cid1 = "QmNY7gDNXHECV8SXoEY7hbfg4BX1aDMxTBDiFuG4huaSGA";
    let ipfs_cid2 = "QmYzsCjrVwwXtdsNm3PZVNziLGmb9o513GUzkq5wwhgXDT";

    store.create_sg_deployment("mainnet", ipfs_cid1).unwrap();
    store.create_sg_deployment("mainnet", ipfs_cid2).unwrap();
    store.set_deployment_name(ipfs_cid2, "foo").unwrap();

    let deployments = store.deployments_with_name("foo").unwrap();
    assert!(deployments.len() == 1);
    assert_eq!(deployments[0].id, ipfs_cid1);
    assert_eq!(deployments[0].name, "foo");
}

#[tokio::test]
async fn create_and_delete_divergence_investigation_request() {
    let store = Store::new(&test_db_url()).await.unwrap();
    let req_uuid = store
        .create_divergence_investigation(NewDivergenceInvestigationRequest {
            pois: vec![],
            query_block_caches: None,
            query_entity_changes: None,
            query_eth_call_caches: None,
        })
        .unwrap();

    let req = store
        .get_first_divergence_investigation_request()
        .unwrap()
        .unwrap();
    assert_eq!(req.0, req_uuid);
    store
        .delete_divergence_investigation_request(&req_uuid)
        .unwrap();
    assert!(store
        .get_first_divergence_investigation_request()
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn poi_db_roundtrip() {
    let mut rng = fast_rng(0);
    let indexers = gen_indexers(&mut rng, 100);

    let indexing_statuses = queries::query_indexing_statuses(indexers).await;
    let pois =
        queries::query_proofs_of_indexing(indexing_statuses, BlockChoicePolicy::Earliest).await;

    let store = Store::new(&test_db_url()).await.unwrap();
    let mut conn = store.test_conn();
    conn.test_transaction::<_, (), _>(|conn| {
        diesel_queries::write_pois(conn, &pois.clone(), PoiLiveness::NotLive).unwrap();
        let all_deployments: Vec<String> =
            pois.iter().map(|poi| poi.deployment.0.clone()).collect();
        let read_pois =
            diesel_queries::pois(conn, None, Some(&all_deployments), None, None, false).unwrap();

        // The triple is (deployment, indexer_id, poi)
        let poi_triples: BTreeSet<(String, String, Vec<u8>)> = pois
            .clone()
            .into_iter()
            .map(|poi| {
                (
                    poi.deployment.0,
                    poi.indexer.id().to_owned(),
                    poi.proof_of_indexing.0.to_vec(),
                )
            })
            .collect();
        let read_poi_triples: BTreeSet<(String, String, Vec<u8>)> = read_pois
            .into_iter()
            .map(|poi| (poi.sg_deployment.cid, poi.indexer.name.unwrap(), poi.poi))
            .collect();
        assert!(poi_triples == read_poi_triples);

        let live_pois =
            diesel_queries::pois(conn, None, Some(&all_deployments), None, None, true).unwrap();
        assert!(live_pois.is_empty());

        Ok(())
    });

    // Same test as above, but this time the pois are live.
    conn.test_transaction::<_, (), _>(|conn| {
        diesel_queries::write_pois(conn, &pois.clone(), PoiLiveness::Live).unwrap();
        let all_deployments: Vec<String> =
            pois.iter().map(|poi| poi.deployment.0.clone()).collect();

        // Assert that all pois are live pois
        assert_eq!(
            diesel_queries::pois(conn, None, Some(&all_deployments), None, None, true)
                .unwrap()
                .into_iter()
                .map(|poi| poi.id)
                .collect::<Vec<_>>(),
            diesel_queries::pois(conn, None, Some(&all_deployments), None, None, false)
                .unwrap()
                .into_iter()
                .map(|poi| poi.id)
                .collect::<Vec<_>>()
        );

        let read_pois =
            diesel_queries::pois(conn, None, Some(&all_deployments), None, None, true).unwrap();

        // The triple is (deployment, indexer_id, poi)
        let poi_triples: BTreeSet<(String, String, Vec<u8>)> = pois
            .clone()
            .into_iter()
            .map(|poi| {
                (
                    poi.deployment.0,
                    poi.indexer.id().to_owned(),
                    poi.proof_of_indexing.0.to_vec(),
                )
            })
            .collect();
        let read_poi_triples: BTreeSet<(String, String, Vec<u8>)> = read_pois
            .into_iter()
            .map(|poi| (poi.sg_deployment.cid, poi.indexer.name.unwrap(), poi.poi))
            .collect();
        assert!(poi_triples == read_poi_triples);
        Ok(())
    });
}

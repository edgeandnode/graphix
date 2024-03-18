mod common;

use graphix_common_types::inputs::SgDeploymentsQuery;
use graphix_store::models::{Network, NewNetwork};
use testcontainers::clients::Cli;

use crate::common::EmptyStoreForTesting;

#[tokio::test]
async fn empty_store_has_no_deployments() {
    let docker_cli = Cli::default();
    let store = EmptyStoreForTesting::new(&docker_cli).await.unwrap();
    let initial_deployments = store
        .sg_deployments(SgDeploymentsQuery::default())
        .await
        .unwrap();
    assert!(initial_deployments.is_empty());
}

#[tokio::test]
async fn create_then_delete_network() {
    let docker_cli = Cli::default();
    let store = EmptyStoreForTesting::new(&docker_cli).await.unwrap();

    assert_eq!(store.networks().await.unwrap(), vec![]);

    store
        .create_network(&NewNetwork {
            name: "mainnet".to_string(),
            caip2: Some("eip155:1".to_string()),
        })
        .await
        .unwrap();

    assert_eq!(
        store.networks().await.unwrap(),
        vec![Network {
            id: 1,
            name: "mainnet".to_string(),
            caip2: Some("eip155:1".to_string())
        }]
    );

    store.delete_network("mainnet").await.unwrap();

    assert_eq!(store.networks().await.unwrap(), vec![]);
}

#[tokio::test]
#[should_panic] // FIXME
async fn deployments_with_name() {
    let docker_cli = Cli::default();
    let store = EmptyStoreForTesting::new(&docker_cli).await.unwrap();

    let ipfs_cid1 = "QmNY7gDNXHECV8SXoEY7hbfg4BX1aDMxTBDiFuG4huaSGA";
    let ipfs_cid2 = "QmYzsCjrVwwXtdsNm3PZVNziLGmb9o513GUzkq5wwhgXDT";

    store
        .create_sg_deployment("mainnet", ipfs_cid1)
        .await
        .unwrap();
    store
        .create_sg_deployment("mainnet", ipfs_cid2)
        .await
        .unwrap();
    store.set_deployment_name(ipfs_cid2, "foo").await.unwrap();

    let deployments = {
        let filter = SgDeploymentsQuery {
            name: Some("foo".to_string()),
            ..Default::default()
        };
        store.sg_deployments(filter).await.unwrap()
    };
    assert!(deployments.len() == 1);
    assert_eq!(deployments[0].cid, ipfs_cid1);
    // FIXME:
    //assert_eq!(deployments[0].name, Some("foo".to_string()));
}

#[tokio::test]
async fn create_divergence_investigation_request() {
    let docker_cli = Cli::default();
    let store = EmptyStoreForTesting::new(&docker_cli).await.unwrap();

    let uuid = store
        .create_divergence_investigation_request(serde_json::json!({}))
        .await
        .unwrap();

    let req = store
        .get_first_pending_divergence_investigation_request()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(req.0, uuid);
}

//#[tokio::test]
//async fn poi_db_roundtrip() {
//    let docker_cli = Cli::default();
//    let store = EmptyStoreForTesting::new(&docker_cli).await.unwrap();
//
//    let mut rng = fast_rng(0);
//    let indexers = gen_indexers(&mut rng, 100);
//
//    let indexing_statuses = queries::query_indexing_statuses(indexers, metrics()).await;
//    let pois =
//        queries::query_proofs_of_indexing(indexing_statuses, BlockChoicePolicy::Earliest).await;
//
//    let mut conn = store.conn().unwrap();
//    conn.test_transaction(|conn| test_pois(conn, &pois, PoiLiveness::NotLive, false));
//    conn.test_transaction(|conn| test_pois(conn, &pois, PoiLiveness::Live, true));
//}

//#[tokio::test]
//async fn test_additional_pois() {
//    let docker_cli = Cli::default();
//    let store = EmptyStoreForTesting::new(&docker_cli).await.unwrap();
//
//    let mut rng = fast_rng(0);
//    let indexers = gen_indexers(&mut rng, 100);
//
//    let indexing_statuses = queries::query_indexing_statuses(indexers, metrics()).await;
//    let pois =
//        queries::query_proofs_of_indexing(indexing_statuses, BlockChoicePolicy::Earliest).await;
//
//    let mut conn = store.conn().unwrap();
//
//    conn.test_transaction(|conn| -> Result<(), anyhow::Error> {
//        // Write the original PoIs as Live
//        diesel_queries::write_pois(conn, &pois, PoiLiveness::Live).unwrap();
//
//        // Choose a deployment to add an additional PoI
//        let mut additional_poi = pois[0].clone(); // clone one of the original PoIs
//        additional_poi.block.number += 1; // bump the number to N + 1
//        additional_poi.block.hash = Some(gen_bytes32(&mut rng));
//        additional_poi.proof_of_indexing = gen_bytes32(&mut rng); // Optionally, set new PoI value if needed
//
//        // Write the additional PoI
//        diesel_queries::write_pois(conn, &[additional_poi.clone()], PoiLiveness::Live).unwrap();
//
//        // Query the live PoIs for the specific deployment
//        let specific_deployment_pois = diesel_queries::pois(
//            conn,
//            None,
//            Some(&[additional_poi.deployment.0.clone()]),
//            None,
//            None,
//            true,
//        )
//        .unwrap();
//
//        // Assert that only the new PoI exists as live PoIs for that deployment
//        assert_eq!(specific_deployment_pois.len(), 1);
//        assert_eq!(
//            specific_deployment_pois[0].block.number as u64,
//            additional_poi.block.number
//        );
//
//        Ok(())
//    });
//}

//fn test_pois(
//    conn: &mut diesel::PgConnection,
//    pois: &[ProofOfIndexing],
//    liveness: PoiLiveness,
//    live_poi_test: bool,
//) -> Result<(), anyhow::Error> {
//    diesel_queries::write_pois(conn, pois, liveness).unwrap();
//    let all_deployments: Vec<String> = pois.iter().map(|poi| poi.deployment.0.clone()).collect();
//
//    // Common logic to create poi_triples
//    let poi_triples: BTreeSet<(String, String, Vec<u8>)> = pois
//        .iter()
//        .map(|poi| {
//            (
//                poi.deployment.0.clone(),
//                poi.indexer.address_string(),
//                poi.proof_of_indexing.0.to_vec(),
//            )
//        })
//        .collect();
//    let read_pois = diesel_queries::pois(
//        conn,
//        None,
//        Some(&all_deployments),
//        None,
//        None,
//        live_poi_test,
//    )
//    .unwrap();
//    let read_poi_triples: BTreeSet<(String, String, Vec<u8>)> = read_pois
//        .into_iter()
//        .map(|poi| (poi.sg_deployment.cid, poi.indexer.name.unwrap(), poi.poi))
//        .collect();
//
//    assert!(poi_triples == read_poi_triples);
//
//    if live_poi_test {
//        // Specific test for live pois
//        assert_eq!(
//            diesel_queries::pois(conn, None, Some(&all_deployments), None, None, true)
//                .unwrap()
//                .into_iter()
//                .map(|poi| poi.id)
//                .collect::<Vec<_>>(),
//            diesel_queries::pois(conn, None, Some(&all_deployments), None, None, false)
//                .unwrap()
//                .into_iter()
//                .map(|poi| poi.id)
//                .collect::<Vec<_>>()
//        );
//    } else {
//        // Specific test for not live pois
//        let live_pois =
//            diesel_queries::pois(conn, None, Some(&all_deployments), None, None, true).unwrap();
//        assert!(live_pois.is_empty());
//    }
//
//    Ok(())
//}

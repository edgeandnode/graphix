use std::collections::BTreeSet;

use crate::db::{diesel_queries, PoiLiveness, Store};
use diesel::Connection;

use crate::tests::{fast_rng, gen::gen_indexers};

fn test_db_url() -> String {
    std::env::var("GRAPHIX_TEST_DB_URL").expect("GRAPHIX_TEST_DB_URL must be set to run tests")
}

#[tokio::test]
async fn poi_db_roundtrip() {
    let mut rng = fast_rng(0);
    let indexers = gen_indexers(&mut rng, 100);

    let indexing_statuses = crate::indexing_statuses::query_indexing_statuses(indexers).await;
    let pois = crate::proofs_of_indexing::query_proofs_of_indexing(indexing_statuses).await;

    let store = Store::new(&test_db_url()).unwrap();
    let mut conn = store.test_conn();
    conn.test_transaction::<_, (), _>(|conn| {
        diesel_queries::write_pois(conn, &pois.clone(), PoiLiveness::NotLive).unwrap();
        let all_deployments: Vec<String> =
            pois.iter().map(|poi| poi.deployment.0.clone()).collect();
        let read_pois =
            diesel_queries::pois(conn, Some(&all_deployments), None, None, false).unwrap();

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
            diesel_queries::pois(conn, Some(&all_deployments), None, None, true).unwrap();
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
            diesel_queries::pois(conn, Some(&all_deployments), None, None, true)
                .unwrap()
                .into_iter()
                .map(|poi| poi.id)
                .collect::<Vec<_>>(),
            diesel_queries::pois(conn, Some(&all_deployments), None, None, false)
                .unwrap()
                .into_iter()
                .map(|poi| poi.id)
                .collect::<Vec<_>>()
        );

        let read_pois =
            diesel_queries::pois(conn, Some(&all_deployments), None, None, true).unwrap();

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

use std::collections::BTreeSet;
use std::ops::Deref;

use diesel::Connection;
use testcontainers::clients::Cli;
use testcontainers::Container;

use crate::block_choice::BlockChoicePolicy;
use crate::graphql_api::types::SgDeploymentsQuery;
use crate::prelude::ProofOfIndexing;
use crate::prometheus_metrics::metrics;
use crate::queries;
use crate::store::{diesel_queries, PoiLiveness, Store};
use crate::test_utils::fast_rng;
use crate::test_utils::gen::{gen_bytes32, gen_indexers};

/// A wrapper around a [`Store`] that is backed by a containerized Postgres
/// database.
pub struct EmptyStoreForTesting<'a> {
    _container: Container<'a, testcontainers_modules::postgres::Postgres>,
    store: Store,
}

impl<'a> EmptyStoreForTesting<'a> {
    pub async fn new(docker_client: &'a Cli) -> anyhow::Result<EmptyStoreForTesting<'a>> {
        use testcontainers_modules::postgres::Postgres;

        let container = docker_client.run(Postgres::default());
        let connection_string = &format!(
            "postgres://postgres:postgres@127.0.0.1:{}/postgres",
            container.get_host_port_ipv4(5432)
        );
        let store = Store::new(connection_string).await?;
        Ok(Self {
            _container: container,
            store,
        })
    }
}

impl<'a> Deref for EmptyStoreForTesting<'a> {
    type Target = Store;

    fn deref(&self) -> &Self::Target {
        &self.store
    }
}

#[tokio::test]
async fn no_deployments_at_first() {
    let docker_cli = Cli::default();
    let store = EmptyStoreForTesting::new(&docker_cli).await.unwrap();
    let initial_deployments = store.sg_deployments(SgDeploymentsQuery::default()).unwrap();
    assert!(initial_deployments.is_empty());
}

#[tokio::test]
#[should_panic] // FIXME
async fn deployments_with_name() {
    let docker_cli = Cli::default();
    let store = EmptyStoreForTesting::new(&docker_cli).await.unwrap();

    let ipfs_cid1 = "QmNY7gDNXHECV8SXoEY7hbfg4BX1aDMxTBDiFuG4huaSGA";
    let ipfs_cid2 = "QmYzsCjrVwwXtdsNm3PZVNziLGmb9o513GUzkq5wwhgXDT";

    store.create_sg_deployment("mainnet", ipfs_cid1).unwrap();
    store.create_sg_deployment("mainnet", ipfs_cid2).unwrap();
    store.set_deployment_name(ipfs_cid2, "foo").unwrap();

    let deployments = {
        let mut filter = SgDeploymentsQuery::default();
        filter.name = Some("foo".to_string());
        store.sg_deployments(filter).unwrap()
    };
    assert!(deployments.len() == 1);
    assert_eq!(deployments[0].id, ipfs_cid1);
    assert_eq!(deployments[0].name, Some("foo".to_string()));
}

#[tokio::test]
async fn create_divergence_investigation_request() {
    let docker_cli = Cli::default();
    let store = EmptyStoreForTesting::new(&docker_cli).await.unwrap();

    let uuid = store
        .create_divergence_investigation_request(serde_json::json!({}))
        .unwrap();

    let req = store
        .get_first_pending_divergence_investigation_request()
        .unwrap()
        .unwrap();
    assert_eq!(req.0, uuid);
}

#[tokio::test]
async fn poi_db_roundtrip() {
    let docker_cli = Cli::default();
    let store = EmptyStoreForTesting::new(&docker_cli).await.unwrap();

    let mut rng = fast_rng(0);
    let indexers = gen_indexers(&mut rng, 100);

    let indexing_statuses = queries::query_indexing_statuses(indexers, metrics()).await;
    let pois =
        queries::query_proofs_of_indexing(indexing_statuses, BlockChoicePolicy::Earliest).await;

    let mut conn = store.conn().unwrap();
    conn.test_transaction(|conn| test_pois(conn, &pois, PoiLiveness::NotLive, false));
    conn.test_transaction(|conn| test_pois(conn, &pois, PoiLiveness::Live, true));
}

#[tokio::test]
async fn test_additional_pois() {
    let docker_cli = Cli::default();
    let store = EmptyStoreForTesting::new(&docker_cli).await.unwrap();

    let mut rng = fast_rng(0);
    let indexers = gen_indexers(&mut rng, 100);

    let indexing_statuses = queries::query_indexing_statuses(indexers, metrics()).await;
    let pois =
        queries::query_proofs_of_indexing(indexing_statuses, BlockChoicePolicy::Earliest).await;

    let mut conn = store.conn().unwrap();

    conn.test_transaction(|conn| -> Result<(), anyhow::Error> {
        // Write the original PoIs as Live
        diesel_queries::write_pois(conn, &pois, PoiLiveness::Live).unwrap();

        // Choose a deployment to add an additional PoI
        let mut additional_poi = pois[0].clone(); // clone one of the original PoIs
        additional_poi.block.number += 1; // bump the number to N + 1
        additional_poi.block.hash = Some(gen_bytes32(&mut rng));
        additional_poi.proof_of_indexing = gen_bytes32(&mut rng); // Optionally, set new PoI value if needed

        // Write the additional PoI
        diesel_queries::write_pois(conn, &[additional_poi.clone()], PoiLiveness::Live).unwrap();

        // Query the live PoIs for the specific deployment
        let specific_deployment_pois = diesel_queries::pois(
            conn,
            None,
            Some(&[additional_poi.deployment.0.clone()]),
            None,
            None,
            true,
        )
        .unwrap();

        // Assert that only the new PoI exists as live PoIs for that deployment
        assert_eq!(specific_deployment_pois.len(), 1);
        assert_eq!(
            specific_deployment_pois[0].block.number as u64,
            additional_poi.block.number
        );

        Ok(())
    });
}

fn test_pois(
    conn: &mut diesel::PgConnection,
    pois: &[ProofOfIndexing],
    liveness: PoiLiveness,
    live_poi_test: bool,
) -> Result<(), anyhow::Error> {
    diesel_queries::write_pois(conn, pois, liveness).unwrap();
    let all_deployments: Vec<String> = pois.iter().map(|poi| poi.deployment.0.clone()).collect();

    // Common logic to create poi_triples
    let poi_triples: BTreeSet<(String, String, Vec<u8>)> = pois
        .into_iter()
        .map(|poi| {
            (
                poi.deployment.0.clone(),
                poi.indexer.id().to_owned(),
                poi.proof_of_indexing.0.to_vec(),
            )
        })
        .collect();
    let read_pois = diesel_queries::pois(
        conn,
        None,
        Some(&all_deployments),
        None,
        None,
        live_poi_test,
    )
    .unwrap();
    let read_poi_triples: BTreeSet<(String, String, Vec<u8>)> = read_pois
        .into_iter()
        .map(|poi| (poi.sg_deployment.cid, poi.indexer.name.unwrap(), poi.poi))
        .collect();

    assert!(poi_triples == read_poi_triples);

    if live_poi_test {
        // Specific test for live pois
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
    } else {
        // Specific test for not live pois
        let live_pois =
            diesel_queries::pois(conn, None, Some(&all_deployments), None, None, true).unwrap();
        assert!(live_pois.is_empty());
    }

    Ok(())
}

use std::{collections::BTreeSet, sync::Arc};
use tracing_test::traced_test;

use eventuals::Eventual;
use futures::{Stream, StreamExt};
use itertools::Itertools;
use rand::{prelude::IteratorRandom, thread_rng, Rng};

use crate::{
    config::IndexerUrls,
    indexing_statuses, proofs_of_indexing,
    types::{BlockPointer, POICrossCheckReport, ProofOfIndexing, SubgraphDeployment},
};

use super::*;

#[tokio::test]
async fn proofs_of_indexing() {
    let rng = fast_rng();

    // Run th test 100 times to increase likelyhood that randomness triggers a bug
    for max_indexers in 0..100 {
        let indexers = gen_indexers(rng.clone(), max_indexers);

        let (mut indexers_writer, indexers_reader) = Eventual::new();
        indexers_writer.write(indexers.clone());

        let indexing_statuses_reader = indexing_statuses::indexing_statuses(indexers_reader);
        let proofs_of_indexing_reader =
            proofs_of_indexing::proofs_of_indexing(indexing_statuses_reader);

        let actual_pois = proofs_of_indexing_reader
            .subscribe()
            .next()
            .await
            .unwrap()
            .into_iter()
            .collect::<BTreeSet<_>>();

        // Assert that for every deployment, the POIs are for the same block
        // (across all indexers)
        assert!(actual_pois
            .iter()
            .group_by(|poi| poi.deployment.clone())
            .into_iter()
            .all(|(_, pois)| { pois.into_iter().map(|poi| &poi.block).all_equal() }));

        // NOTE: Add more assertions later.
    }
}

fn gen_basic_cross_checking_inputs() -> (
    impl Rng,
    SubgraphDeployment,
    Vec<BlockPointer>,
    BlockPointer,
    BlockPointer,
    Vec<PartialProofOfIndexing>,
) {
    let mut rng = thread_rng();

    let deployment = gen_deployments().get(0).unwrap().clone();
    let blocks = gen_blocks();

    // Generate a sequence of canonical POIs
    let canonical_pois = gen_pois(blocks.clone(), &mut rng);

    // Decide on a random range of blocks (at least one block) to work with
    let blocks = Vec::from(&blocks[0..rng.gen_range(1..blocks.len())]);
    let first_block = blocks.first().unwrap().clone();
    let latest_block = blocks.last().unwrap().clone();

    (
        rng,
        deployment,
        blocks,
        first_block,
        latest_block,
        canonical_pois,
    )
}

#[tokio::test]
#[traced_test]
async fn cross_check_identical_pois() {
    // Run this test 100 times with a random latest block
    for _ in 0..100 {
        let (_, deployment, _, _, latest_block, canonical_pois) = gen_basic_cross_checking_inputs();

        let deployment_details = vec![DeploymentDetails {
            deployment: deployment.clone(),
            network: "mainnet".into(),
            latest_block: latest_block.clone(),
            canonical_pois: canonical_pois.clone(),
        }];

        // Generate two indexers with identical indexing results
        let indexer1 = Arc::new(MockIndexer {
            id: "indexer1".into(),
            urls: IndexerUrls {
                status: "http://indexer-1.com/".parse().unwrap(),
            },
            deployment_details: deployment_details.clone(),
            fail_indexing_statuses: false,
            fail_proofs_of_indexing: false,
        });
        let indexer2 = Arc::new(MockIndexer {
            id: "indexer2".into(),
            urls: IndexerUrls {
                status: "http://indexer-2.com/".parse().unwrap(),
            },
            deployment_details: deployment_details.clone(),
            fail_indexing_statuses: false,
            fail_proofs_of_indexing: false,
        });

        let (mut indexers_writer, indexers_reader) = Eventual::new();
        indexers_writer.write(vec![indexer1.clone(), indexer2.clone()]);

        let indexing_statuses_reader = indexing_statuses::indexing_statuses(indexers_reader);
        let proofs_of_indexing_reader =
            proofs_of_indexing::proofs_of_indexing(indexing_statuses_reader);

        let (proofs_of_indexing_reader, reports_reader) =
            proofs_of_indexing::cross_checking(proofs_of_indexing_reader.clone());

        let reports = reports_reader.take(1).collect::<Vec<_>>().await;

        let expected_poi1 = ProofOfIndexing {
            indexer: indexer1,
            deployment: deployment.clone(),
            block: latest_block.clone(),
            proof_of_indexing: canonical_pois
                .iter()
                .find(|poi| poi.block.eq(&latest_block))
                .unwrap()
                .proof_of_indexing
                .clone(),
        };
        let expected_poi2 = ProofOfIndexing {
            indexer: indexer2,
            deployment: deployment.clone(),
            block: latest_block.clone(),
            proof_of_indexing: canonical_pois
                .iter()
                .find(|poi| poi.block.eq(&latest_block))
                .unwrap()
                .proof_of_indexing
                .clone(),
        };

        assert_eq!(
            reports,
            vec![POICrossCheckReport {
                poi1: expected_poi1.clone(),
                poi2: expected_poi2.clone(),
                diverging_block: None,
            }]
        );

        let proofs_of_indexing = proofs_of_indexing_reader.take(2).collect::<Vec<_>>().await;
        assert_eq!(proofs_of_indexing, vec![expected_poi1, expected_poi2]);
    }
}

#[tokio::test]
#[traced_test]
async fn cross_check_pois_with_mismatch_in_random_block() {
    // Run this test 1000 times with a random latest block and random diverging block
    for _ in 0..1000 {
        let (mut rng, deployment, blocks, _, latest_block, canonical_pois) =
            gen_basic_cross_checking_inputs();

        // Create a sequence of POIs that diverges at the very beginning
        let mut diverging_pois = canonical_pois.clone();
        let diverging_block_number = rng.gen_range(0..blocks.len());
        let diverging_block = blocks.get(diverging_block_number).unwrap();
        let mut in_diverging_range = false;
        for poi in diverging_pois.iter_mut() {
            if poi.block.eq(&diverging_block) || in_diverging_range {
                poi.proof_of_indexing = gen_bytes32(&mut rng);
                in_diverging_range = true;
            }
        }

        // Generate first indexer with canonical POIs
        let deployment_details1 = vec![DeploymentDetails {
            deployment: deployment.clone(),
            network: "mainnet".into(),
            latest_block: latest_block.clone(),
            canonical_pois: canonical_pois.clone(),
        }];
        let indexer1 = Arc::new(MockIndexer {
            id: "indexer1".into(),
            urls: IndexerUrls {
                status: "http://indexer-1.com/".parse().unwrap(),
            },
            deployment_details: deployment_details1.clone(),
            fail_indexing_statuses: false,
            fail_proofs_of_indexing: false,
        });

        // Generate second indexer with diverging POI at the diverging block
        let deployment_details2 = vec![DeploymentDetails {
            deployment: deployment.clone(),
            network: "mainnet".into(),
            latest_block: latest_block.clone(),
            canonical_pois: diverging_pois.clone(),
        }];
        let indexer2 = Arc::new(MockIndexer {
            id: "indexer2".into(),
            urls: IndexerUrls {
                status: "http://indexer-2.com/".parse().unwrap(),
            },
            deployment_details: deployment_details2.clone(),
            fail_indexing_statuses: false,
            fail_proofs_of_indexing: false,
        });

        let (mut indexers_writer, indexers_reader) = Eventual::new();
        indexers_writer.write(vec![indexer1.clone(), indexer2.clone()]);

        let indexing_statuses_reader = indexing_statuses::indexing_statuses(indexers_reader);
        let proofs_of_indexing_reader =
            proofs_of_indexing::proofs_of_indexing(indexing_statuses_reader);

        let (mut proofs_of_indexing_reader, reports_reader) =
            proofs_of_indexing::cross_checking(proofs_of_indexing_reader.clone());

        let reports = reports_reader.take(1).collect::<Vec<_>>().await;

        let expected_poi1 = ProofOfIndexing {
            indexer: indexer1,
            deployment: deployment.clone(),
            block: diverging_block.clone(),
            proof_of_indexing: canonical_pois
                .iter()
                .find(|poi| poi.block.eq(&diverging_block))
                .unwrap()
                .proof_of_indexing
                .clone(),
        };
        let expected_poi2 = ProofOfIndexing {
            indexer: indexer2,
            deployment: deployment.clone(),
            block: diverging_block.clone(),
            proof_of_indexing: diverging_pois
                .iter()
                .find(|poi| poi.block.eq(&diverging_block))
                .unwrap()
                .proof_of_indexing
                .clone(),
        };

        assert_eq!(
            reports,
            vec![POICrossCheckReport {
                poi1: expected_poi1.clone(),
                poi2: expected_poi2.clone(),
                diverging_block: Some(()),
            }]
        );

        // Read the POIs collected during bisecting until we have found the POIs
        // for the diverging block; these are the last two POIs reported
        //
        // NOTE: We need to do it this way, because we don't know how many POIs
        // are collected and reported as part of the cross-checking (which runs
        // a bisect to find the first bad block). If we tried to just collect
        // the entire stream, the test would run forever.
        let mut found_expected_poi1 = false;
        let mut found_expected_poi2 = false;
        while !found_expected_poi1 || !found_expected_poi2 {
            let poi = proofs_of_indexing_reader.next().await;
            match poi {
                Some(poi) if poi.eq(&expected_poi1) => found_expected_poi1 = true,
                Some(poi) if poi.eq(&expected_poi2) => found_expected_poi2 = true,
                _ => {}
            }
        }
    }
}

#[tokio::test]
#[traced_test]
async fn random_poi_cross_checking() {
    let rng = fast_rng();

    for _ in 0..100 {
        let indexers = gen_indexers(rng.clone(), 4);

        let (mut indexers_writer, indexers_reader) = Eventual::new();
        indexers_writer.write(indexers.clone());

        let indexing_statuses_reader = indexing_statuses::indexing_statuses(indexers_reader);
        let proofs_of_indexing_reader =
            proofs_of_indexing::proofs_of_indexing(indexing_statuses_reader);

        let (_proofs_of_indexing_reader, _reports_reader) =
            proofs_of_indexing::cross_checking(proofs_of_indexing_reader.clone());

        // TODO: Check POIs and reports
        // NOTE: The code below freezes the test
        // let proofs_of_indexing = proofs_of_indexing_reader.collect::<Vec<_>>().await;
        // let reports = reports_reader.value().await.unwrap();
    }
}

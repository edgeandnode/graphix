use std::{iter::repeat_with, sync::Arc};

use rand::{distributions::Alphanumeric, seq::IteratorRandom, Rng, RngCore};

use crate::{
    config::IndexerUrls,
    types::{BlockPointer, Bytes32, SubgraphDeployment},
};

use super::{DeploymentDetails, MockIndexer, PartialProofOfIndexing};

pub fn gen_deployments() -> Vec<SubgraphDeployment> {
    vec![
        "QmAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "QmBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
        "QmCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
        "QmDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD",
    ]
    .into_iter()
    .map(|s| SubgraphDeployment(s.to_owned()))
    .collect()
}

pub fn gen_blocks() -> Vec<BlockPointer> {
    vec![
        (
            0,
            "0x0000000000000000000000000000000000000000000000000000000000000000",
        ),
        (
            1,
            "0x0000000000000000000000000000000000000000000000000000000000000001",
        ),
        (
            2,
            "0x0000000000000000000000000000000000000000000000000000000000000002",
        ),
        (
            3,
            "0x0000000000000000000000000000000000000000000000000000000000000003",
        ),
        (
            4,
            "0x0000000000000000000000000000000000000000000000000000000000000004",
        ),
        (
            5,
            "0x0000000000000000000000000000000000000000000000000000000000000005",
        ),
        (
            6,
            "0x0000000000000000000000000000000000000000000000000000000000000006",
        ),
        (
            7,
            "0x0000000000000000000000000000000000000000000000000000000000000007",
        ),
        (
            8,
            "0x0000000000000000000000000000000000000000000000000000000000000008",
        ),
        (
            9,
            "0x0000000000000000000000000000000000000000000000000000000000000009",
        ),
    ]
    .into_iter()
    .map(|(number, hash)| BlockPointer {
        number,
        hash: hash.try_into().unwrap(),
    })
    .collect()
}

pub fn gen_bytes32(rng: &mut impl RngCore) -> Bytes32 {
    let mut bytes = [0; 32];
    rng.fill_bytes(&mut bytes);
    Bytes32::try_from(hex::encode(bytes).as_str()).unwrap()
}

pub fn gen_indexers<R>(mut rng: R, max_indexers: usize) -> Vec<Arc<MockIndexer>>
where
    R: RngCore + Clone,
{
    // Generate some deployments and blocks
    let deployments = gen_deployments();
    let blocks = gen_blocks();

    let number_of_indexers = rng.gen_range(0..=max_indexers);

    // Generate a random number of indexers
    repeat_with(move || {
        let id = rng
            .clone()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();

        let number_of_deployments = rng.gen_range(0..=deployments.len());

        let random_deployments = deployments
            .clone()
            .into_iter()
            .choose_multiple(&mut rng, number_of_deployments);

        let deployment_details = random_deployments
            .clone()
            .into_iter()
            .map(|deployment| DeploymentDetails {
                deployment,
                network: "mainnet".into(),
                latest_block: blocks.iter().choose(&mut rng).unwrap().clone(),
                canonical_pois: blocks
                    .clone()
                    .into_iter()
                    .map(|block| PartialProofOfIndexing {
                        block,
                        proof_of_indexing: gen_bytes32(&mut rng),
                    })
                    .collect(),
            })
            .collect();

        Arc::new(MockIndexer {
            id,
            urls: IndexerUrls {
                status: "http://some-url.com/".parse().unwrap(),
            },
            deployment_details,
            fail_indexing_statuses: rng.gen_bool(0.1),
            fail_proofs_of_indexing: rng.gen_bool(0.1),
        })
    })
    .take(number_of_indexers)
    .collect::<Vec<Arc<MockIndexer>>>()
}

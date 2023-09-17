use std::{iter::repeat_with, sync::Arc};

use crate::prelude::{BlockPointer, Bytes32, Indexer, SubgraphDeployment};
use rand::{distributions::Alphanumeric, seq::IteratorRandom, Rng};

use super::mocks::{DeploymentDetails, MockIndexer, PartialProofOfIndexing};

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
    let block_hash = |n: u64| -> Bytes32 {
        let mut buf = [0u8; 32];
        buf[24..32].clone_from_slice(&n.to_be_bytes());
        Bytes32(buf.into())
    };
    (0..10)
        .map(|number| BlockPointer {
            number,
            hash: Some(block_hash(number)),
        })
        .collect()
}

pub fn gen_bytes32<R>(rng: &mut R) -> Bytes32
where
    R: Rng,
{
    let mut bytes = [0; 32];
    rng.fill_bytes(&mut bytes);
    Bytes32::try_from(hex::encode(bytes).as_str()).unwrap()
}

pub fn gen_pois<R>(blocks: Vec<BlockPointer>, mut rng: &mut R) -> Vec<PartialProofOfIndexing>
where
    R: Rng,
{
    blocks
        .clone()
        .into_iter()
        .map(|block| PartialProofOfIndexing {
            block,
            proof_of_indexing: gen_bytes32(&mut rng),
        })
        .collect()
}

pub fn gen_indexers<R>(mut rng: &mut R, max_indexers: usize) -> Vec<Arc<dyn Indexer>>
where
    R: Rng,
{
    // Generate some deployments and blocks
    let deployments = gen_deployments();
    let blocks = gen_blocks();

    let number_of_indexers = rng.gen_range(0..=max_indexers);

    // Generate a random number of indexers
    repeat_with(move || {
        let id = rng
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
                canonical_pois: gen_pois(blocks.clone(), &mut rng),
                earliest_block_num: blocks[0].number,
            })
            .collect();

        Arc::new(MockIndexer {
            id,
            deployment_details,
            fail_indexing_statuses: rng.gen_bool(0.1),
        }) as Arc<dyn Indexer>
    })
    .take(number_of_indexers)
    .collect()
}

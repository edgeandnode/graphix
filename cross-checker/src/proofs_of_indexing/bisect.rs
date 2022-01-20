use futures::Future;
use graph_ixi_common::prelude::{Indexer, ProofOfIndexing};
use tracing::info;

pub struct DivergingBlock<I>
where
    I: Indexer,
{
    pub poi1: ProofOfIndexing<I>,
    pub poi2: ProofOfIndexing<I>,
}

pub enum BisectDecision<I>
where
    I: Indexer,
{
    Good,
    Bad {
        poi1: ProofOfIndexing<I>,
        poi2: ProofOfIndexing<I>,
    },
}

pub async fn bisect_blocks<C, F, Out, I>(
    bisection_id: String,
    context: C,
    mut bad: DivergingBlock<I>,
    test_fn: F,
) -> Result<DivergingBlock<I>, anyhow::Error>
where
    C: Clone,
    F: Fn(String, C, u64) -> Out,
    Out: Future<Output = Result<BisectDecision<I>, anyhow::Error>>,
    I: Indexer,
{
    info!(%bisection_id, bad = %bad.poi1.block.number, "Bisect start");

    // Special-casing for block #0; we could incorporate it into the bisecting
    // logic somehow but starting with good=-1 would complicate it a bunch
    {
        // Check the first block to find out if it's good or not
        let decision = test_fn(bisection_id.clone(), context.clone(), 0).await?;

        // If the first block is bad, we've found the bad block
        if let BisectDecision::Bad { poi1, poi2 } = decision {
            info!(%bisection_id, first_bad_block = %poi1.block, "Bisect end");
            return Ok(DivergingBlock { poi1, poi2 });
        }
    }

    // If the first block is good, we can start bisecting properly
    let mut good = 0;

    while bad.poi1.block.number - good > 1 {
        info!(%bisection_id, %good, bad = %bad.poi1.block.number, "Bisect step");

        // Calculate the block number in the middle between bad and good
        let current = good + (bad.poi1.block.number - good) / 2;

        // Test if this block is good or bad
        let decision = test_fn(bisection_id.clone(), context.clone(), current).await?;

        // Adjust the good/bad numbers according to the result
        match decision {
            BisectDecision::Good => {
                info!(%bisection_id, good_block = %current, "Bisect decision: block is good");
                good = current;
            }
            BisectDecision::Bad { poi1, poi2 } => {
                info!(%bisection_id, bad_block = %current, "Bisect decision: block is bad");
                bad = DivergingBlock { poi1, poi2 };
            }
        }
    }

    info!(%bisection_id, first_bad_block = %bad.poi1.block, "Bisect end");

    Ok(bad)
}

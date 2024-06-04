use graphix_indexer_client::IndexingStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum BlockChoicePolicy {
    // Use the earliest block that all indexers have in common
    #[default]
    Earliest,
    // Use the block that maximizes the total number of blocks synced across all indexers
    MaxSyncedBlocks,
}

impl BlockChoicePolicy {
    pub fn choose_block<'a>(
        &self,
        statuses: impl Iterator<Item = &'a IndexingStatus>,
    ) -> Option<u64> {
        match self {
            BlockChoicePolicy::Earliest => statuses
                .map(|status| &status.latest_block.number)
                .min()
                .copied(),
            BlockChoicePolicy::MaxSyncedBlocks => {
                // Assuming that all statuses have the same `deployment` and `earliest_block_num`,
                // this will return the block number that maximizes the total number of blocks
                // synced across all indexers.

                let mut indexers_ascending: Vec<&'a IndexingStatus> = statuses.collect();
                indexers_ascending.sort_by_key(|status| status.latest_block.number);

                let mut max_utility = 0;
                let mut best_block: Option<u64> = None;

                for (i, status) in indexers_ascending.iter().enumerate() {
                    let remaining_statuses = indexers_ascending.len() - i;
                    let block_number = status.latest_block.number;
                    if block_number < status.earliest_block_num {
                        // This status is inconsistent, ignore it, avoiding overflow.
                        continue;
                    }

                    let utility =
                        remaining_statuses as u64 * (block_number - status.earliest_block_num);

                    if utility > max_utility {
                        max_utility = utility;
                        best_block = Some(block_number);
                    }
                }

                best_block
            }
        }
    }
}

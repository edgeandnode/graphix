use crate::prelude::IndexingStatus;

#[derive(Copy, Clone, Debug)]
pub enum BlockChoicePolicy {
    // Use the earliest block that all indexers have in common
    Earliest,
    // Use the block that maximizes the total number of blocks synced across all indexers
    // MaxSyncedBlocks,
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
            // BlockChoicePolicy::MaxSyncedBlocks => {}
        }
    }
}

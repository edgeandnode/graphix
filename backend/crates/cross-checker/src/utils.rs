use std::{collections::HashSet, hash::Hash, sync::Arc};

use anyhow::Context;
use graphix_common::prelude::Indexer;
use graphix_common::store;

use crate::DivergenceInvestigationError;

/// Creates all combinations of elements in the iterator, without duplicates.
/// Elements are never paired with themselves.
pub fn unordered_pairs_combinations<T>(iter: impl Iterator<Item = T> + Clone) -> HashSet<(T, T)>
where
    T: Hash + Eq + Clone,
{
    let mut pairs = HashSet::new();
    for (i, x) in iter.clone().enumerate() {
        for y in iter.clone().skip(i + 1) {
            pairs.insert((x.clone(), y));
        }
    }
    pairs
}

/// Given a PoI, find any of the indexers that have been known to produce it.
pub fn find_any_indexer_for_poi(
    store: &store::Store,
    poi_s: &str,
    indexers: &[Arc<dyn Indexer>],
) -> anyhow::Result<Option<Arc<dyn Indexer>>> {
    let poi = if let Some(poi) = store.poi(&poi_s)? {
        poi
    } else {
        return Ok(None);
    };

    let indexer_opt = indexers
        .iter()
        .find(|indexer| indexer.address() == poi.indexer.address.as_deref())
        .cloned();

    Ok(indexer_opt)
}

pub fn find_indexer_pair(
    store: &store::Store,
    poi_1: &str,
    poi_2: &str,
    indexers: &[Arc<dyn Indexer>],
) -> Result<(Arc<dyn Indexer>, Arc<dyn Indexer>), DivergenceInvestigationError> {
    let indexer1 = find_any_indexer_for_poi(store, poi_1, indexers)
        .map_err(DivergenceInvestigationError::Database)
        .and_then(|opt| {
            if let Some(indexer) = opt {
                Ok(indexer)
            } else {
                Err(DivergenceInvestigationError::IndexerNotFound {
                    poi: poi_1.to_string(),
                })
            }
        })?;
    let indexer2 = find_any_indexer_for_poi(store, poi_2, indexers)
        .map_err(DivergenceInvestigationError::Database)
        .and_then(|opt| {
            if let Some(indexer) = opt {
                Ok(indexer)
            } else {
                Err(DivergenceInvestigationError::IndexerNotFound {
                    poi: poi_2.to_string(),
                })
            }
        })?;

    Ok((indexer1, indexer2))
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn test_unordered_pairs_combinations(original: Vec<u32>, combinations: Vec<(u32, u32)>) {
        assert_eq!(
            unordered_pairs_combinations(original.into_iter()),
            HashSet::from_iter(combinations.into_iter())
        );
    }

    #[test]
    fn unordered_pairs_combinations_test_cases() {
        test_unordered_pairs_combinations(vec![], vec![]);
        test_unordered_pairs_combinations(vec![1], vec![]);
        test_unordered_pairs_combinations(vec![1, 2], vec![(1, 2)]);
        test_unordered_pairs_combinations(vec![1, 2, 3], vec![(1, 2), (2, 3), (1, 3)]);
    }
}

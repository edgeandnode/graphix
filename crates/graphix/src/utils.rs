#![allow(dead_code)]

use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Arc;

use graphix_indexer_client::Indexer;
use graphix_store::Store;

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

/// Given a Poi, find any of the indexers that have been known to produce it.
pub async fn find_any_indexer_for_poi(
    store: &Store,
    poi_s: &str,
    indexers: &[Arc<dyn Indexer>],
) -> anyhow::Result<Option<Arc<dyn Indexer>>> {
    let Some(poi) = store.poi(poi_s).await? else {
        return Ok(None);
    };

    let indexer_opt = indexers
        .iter()
        .find(|indexer| indexer.address() == poi.indexer.address)
        .cloned();

    Ok(indexer_opt)
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

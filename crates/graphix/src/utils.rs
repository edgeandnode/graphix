#![allow(dead_code)]

use std::collections::HashSet;
use std::hash::Hash;

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

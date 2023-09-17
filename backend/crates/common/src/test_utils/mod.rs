pub mod gen;
pub mod mocks;

use once_cell::sync::Lazy;
use rand::{rngs::OsRng, rngs::SmallRng, RngCore, SeedableRng};
use std::env;

pub static TEST_SEED: Lazy<u64> = Lazy::new(|| {
    let seed = env::var("TEST_SEED")
        .map(|seed| seed.parse().expect("Invalid TEST_SEED value"))
        .unwrap_or(OsRng.next_u64());

    println!("------------------------------------------------------------------------");
    println!("TEST_SEED={}", seed);
    println!("  This value can be changed via the environment variable TEST_SEED.");
    println!("------------------------------------------------------------------------");

    seed
});

pub fn fast_rng(seed_extra: u64) -> SmallRng {
    SmallRng::seed_from_u64(*TEST_SEED + seed_extra)
}

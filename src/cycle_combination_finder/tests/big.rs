#![warn(clippy::pedantic)]

use std::num::NonZeroU16;

use cycle_combination_finder::{
    finder::{CycleCombinationFinder, Optimality, RegisterCount},
    puzzle::misc::BIG1,
};

use crate::common::cycles;

mod common;

#[test_log::test]
fn optimal_2() {
    let big = BIG1.clone();
    let ccf = CycleCombinationFinder::from(big);
    let cycle_combinations = ccf.find(
        Optimality::Optimal,
        RegisterCount::Exactly(NonZeroU16::new(2).unwrap()),
    );
    assert_eq!(
        cycles(cycle_combinations),
        vec![
            vec![1_396_755_360, 3],
            vec![944_863_920, 9],
            vec![845_404_560, 18],
            vec![698_377_680, 90],
            vec![349_188_840, 360],
            vec![232_792_560, 630],
            vec![116_396_280, 2520],
            vec![41_081_040, 7560],
            vec![36_756_720, 13860],
            vec![20_540_520, 27720],
            vec![18_378_360, 32760],
            vec![12_252_240, 55440],
            vec![6_846_840, 83160],
            vec![6_126_120, 98280],
            vec![3_063_060, 166_320],
            vec![2_827_440, 180_180],
            vec![2_162_160, 360_360],
            vec![1_081_080, 1_081_080],
        ]
    );
}

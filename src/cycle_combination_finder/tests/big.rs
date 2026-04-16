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
            vec![1396755360, 3],
            vec![944863920, 9],
            vec![845404560, 18],
            vec![698377680, 90],
            vec![349188840, 360],
            vec![232792560, 630],
            vec![116396280, 2520],
            vec![41081040, 7560],
            vec![36756720, 13860],
            vec![20540520, 27720],
            vec![18378360, 32760],
            vec![12252240, 55440],
            vec![6846840, 83160],
            vec![6126120, 98280],
            vec![3063060, 166320],
            vec![2827440, 180180],
            vec![2162160, 360360],
            vec![1081080, 1081080],
        ]
    );
}

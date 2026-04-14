use std::num::NonZeroU16;

use cycle_combination_finder::{
    finder::{CycleCombinationFinder, Optimality, RegisterCount},
    puzzle::minxN::MINX3,
};
use puzzle_theory::numbers::{Int, U};

use crate::common::cycles;

mod common;

#[test_log::test]
fn optimal_2() {
    let megaminx = MINX3.clone();
    let ccf = CycleCombinationFinder::from(megaminx);
    let cycle_combinations = ccf.find(
        Optimality::Optimal,
        RegisterCount::Exactly(NonZeroU16::new(2).unwrap()),
    );
    assert_eq!(
        cycles(cycle_combinations),
        vec![
            vec![720720, 1],
            vec![540540, 2],
            vec![360360, 18],
            vec![196560, 36],
            vec![166320, 72],
            vec![98280, 120],
            vec![83160, 180],
            vec![65520, 360],
            vec![55440, 504],
            vec![32760, 840],
            vec![27720, 1260],
            vec![13860, 2520],
            vec![7560, 3780],
            vec![5544, 5040]
        ]
    );
}

#[test_log::test]
fn optimal_3() {
    let megaminx = MINX3.clone();
    let ccf = CycleCombinationFinder::from(megaminx);
    let cycle_combinations = ccf.find(
        Optimality::Optimal,
        RegisterCount::Exactly(NonZeroU16::new(3).unwrap()),
    );
    assert_eq!(
        cycles(cycle_combinations),
        vec![
            vec![720720, 1, 1],
            vec![360360, 12, 3],
            vec![360360, 6, 6],
            vec![180180, 12, 12],
            vec![98280, 24, 18],
            vec![83160, 36, 30],
            vec![55440, 60, 36],
            vec![32760, 90, 60],
            vec![27720, 180, 72],
            vec![27720, 120, 90],
            vec![15120, 120, 120],
            vec![9240, 180, 180],
            vec![5040, 360, 360],
            vec![2520, 630, 420],
            vec![1260, 840, 630]
        ]
    );
}

#[test_log::test]
fn equivalent_2() {
    let megaminx = MINX3.clone();
    let ccf = CycleCombinationFinder::from(megaminx);
    let cycle_combinations = ccf.find(
        Optimality::Equivalent,
        RegisterCount::Exactly(NonZeroU16::new(2).unwrap()),
    );
    assert_eq!(
        cycle_combinations[0].cycles()[0].order(),
        Int::<U>::from(5040_u16),
    );
}

#[test_log::test]
fn equivalent_3() {
    let megaminx = MINX3.clone();
    let ccf = CycleCombinationFinder::from(megaminx);
    let cycle_combinations = ccf.find(
        Optimality::Equivalent,
        RegisterCount::Exactly(NonZeroU16::new(3).unwrap()),
    );
    assert_eq!(
        cycle_combinations[0].cycles()[0].order(),
        Int::<U>::from(630_u16),
    );
}

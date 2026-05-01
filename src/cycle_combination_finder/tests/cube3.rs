use std::num::NonZeroU16;

use cycle_combination_finder::{
    finder::{CycleCombinationFinder, Optimality, RegisterCount},
    puzzle::cubeN::CUBE3,
};
use puzzle_theory::numbers::{Int, U};

use crate::common::cycles;

mod common;

#[test_log::test]
fn optimal_2() {
    let cube3 = CUBE3.clone();
    let ccf = CycleCombinationFinder::from(cube3);
    let cycle_combinations = ccf.find(
        Optimality::Optimal,
        RegisterCount::Exactly(NonZeroU16::new(2).unwrap()),
    );
    assert_eq!(
        cycles(cycle_combinations),
        vec![
            vec![1260, 2],
            vec![840, 3],
            vec![720, 4],
            vec![630, 9],
            vec![420, 12],
            vec![360, 36],
            vec![180, 72],
            vec![90, 90],
        ],
    );
}

#[test_log::test]
fn optimal_3() {
    let cube3 = CUBE3.clone();
    let ccf = CycleCombinationFinder::from(cube3);
    let cycle_combinations = ccf.find(
        Optimality::Optimal,
        RegisterCount::Exactly(NonZeroU16::new(3).unwrap()),
    );
    assert_eq!(
        cycles(cycle_combinations),
        vec![
            vec![1260, 2, 1],
            vec![720, 2, 2],
            vec![630, 9, 3],
            vec![360, 9, 6],
            vec![210, 9, 9],
            vec![180, 18, 12],
            vec![90, 30, 18],
            vec![72, 36, 24],
            vec![36, 36, 30],
        ]
    );
}

#[test_log::test]
fn optimal_4() {
    let cube3 = CUBE3.clone();
    let ccf = CycleCombinationFinder::from(cube3);
    let cycle_combinations = ccf.find(
        Optimality::Optimal,
        RegisterCount::Exactly(NonZeroU16::new(4).unwrap()),
    );
    assert_eq!(
        cycles(cycle_combinations),
        vec![
            vec![1260, 2, 1, 1],
            vec![630, 3, 3, 3],
            vec![360, 4, 4, 4],
            vec![180, 12, 6, 6],
            vec![90, 12, 12, 12]
        ],
    );
}

#[test_log::test]
fn optimal_5() {
    let cube3 = CUBE3.clone();
    let ccf = CycleCombinationFinder::from(cube3);
    let cycle_combinations = ccf.find(
        Optimality::Optimal,
        RegisterCount::Exactly(NonZeroU16::new(5).unwrap()),
    );
    assert_eq!(
        cycles(cycle_combinations),
        vec![
            vec![1260, 2, 1, 1, 1],
            vec![630, 3, 3, 3, 3],
            vec![180, 4, 4, 4, 4],
            vec![126, 6, 6, 6, 6],
            vec![36, 12, 12, 12, 12]
        ],
    );
}

#[test_log::test]
fn equivalent_2() {
    let cube3 = CUBE3.clone();
    let ccf = CycleCombinationFinder::from(cube3);
    let cycle_combinations = ccf.find(
        Optimality::Equivalent,
        RegisterCount::Exactly(NonZeroU16::new(2).unwrap()),
    );
    assert_eq!(
        cycle_combinations[0].cycles()[0].order(),
        Int::<U>::from(90_u16),
    );
}

#[test_log::test]
fn equivalent_3() {
    let cube3 = CUBE3.clone();
    let ccf = CycleCombinationFinder::from(cube3);
    let cycle_combinations = ccf.find(
        Optimality::Equivalent,
        RegisterCount::Exactly(NonZeroU16::new(3).unwrap()),
    );
    assert_eq!(
        cycle_combinations[0].cycles()[0].order(),
        Int::<U>::from(30_u16),
    );
}

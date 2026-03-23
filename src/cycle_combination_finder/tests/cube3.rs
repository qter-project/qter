use std::num::NonZeroU16;

use cycle_combination_finder::{
    finder::{CycleCombinationFinder, MaxPrimePower, Optimality, RegisterCount},
    puzzle::{EvenParityConstraints, OrientationSumConstraint, ParityConstraint, PuzzleDef},
};
use puzzle_theory::{
    numbers::{Int, U},
    puzzle_geometry::parsing::puzzle,
};

use crate::common::cycles;

mod common;

pub fn cube3() -> PuzzleDef {
    PuzzleDef::from_ksolve_naive(
        &puzzle("3x3").ksolve(),
        OrientationSumConstraint::Zero,
        EvenParityConstraints(vec![vec![0, 1]]),
        vec![
            (OrientationSumConstraint::Zero, ParityConstraint::None),
            (OrientationSumConstraint::Zero, ParityConstraint::None),
        ],
    )
    .unwrap()
}

#[test_log::test]
fn test_max_prime_powers_below_edge_cases() {
    let cube3 = cube3();
    let ccf = CycleCombinationFinder::from(cube3);
    assert!(ccf.max_prime_powers_below(0).is_empty());
    assert!(ccf.max_prime_powers_below(1).is_empty());
}

#[test_log::test]
fn test_cube3_max_prime_powers_below() {
    let cube3 = cube3();
    let ccf = CycleCombinationFinder::from(cube3);
    let max_prime_powers = ccf.max_prime_powers_below(12);
    assert_eq!(
        max_prime_powers,
        vec![
            MaxPrimePower {
                prime: 2,
                exponent: 4,
            },
            MaxPrimePower {
                prime: 3,
                exponent: 2,
            },
            MaxPrimePower {
                prime: 5,
                exponent: 1,
            },
            MaxPrimePower {
                prime: 7,
                exponent: 1,
            },
            MaxPrimePower {
                prime: 11,
                exponent: 1,
            },
        ]
    );
}

#[test_log::test]
fn test_cube3_2_optimal() {
    let cube3 = cube3();
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
fn test_cube3_3_optimal() {
    let cube3 = cube3();
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
fn test_cube3_2_equivalent() {
    let cube3 = cube3();
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
fn test_cube3_3_equivalent() {
    let cube3 = cube3();
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

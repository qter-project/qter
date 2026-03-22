use std::num::NonZeroU16;

use cycle_combination_finder::{
    finder::{CycleCombinationFinder, MaxPrimePower, Optimality, RegisterCount},
    puzzle::{OrientationSumConstraint, ParityConstraint, PuzzleDef},
};
use puzzle_theory::{
    numbers::{Int, U},
    puzzle_geometry::parsing::puzzle,
};

use crate::common::cycles;

mod common;

pub fn megaminx() -> PuzzleDef {
    PuzzleDef::from_ksolve_naive(
        &puzzle("megaminx").ksolve(),
        OrientationSumConstraint::Zero,
        ParityConstraint::Even,
        vec![
            (OrientationSumConstraint::Zero, ParityConstraint::None),
            (OrientationSumConstraint::Zero, ParityConstraint::None),
        ],
    )
    .unwrap()
}

#[test_log::test]
fn test_megaminx_max_prime_powers_below() {
    let megaminx = megaminx();
    let ccf = CycleCombinationFinder::from(megaminx);
    let max_prime_powers = ccf.max_prime_powers_below(30);
    assert_eq!(
        max_prime_powers,
        vec![
            MaxPrimePower {
                prime: 2,
                exponent: 5
            },
            MaxPrimePower {
                prime: 3,
                exponent: 3
            },
            MaxPrimePower {
                prime: 5,
                exponent: 2
            },
            MaxPrimePower {
                prime: 7,
                exponent: 1
            },
            MaxPrimePower {
                prime: 11,
                exponent: 1
            },
            MaxPrimePower {
                prime: 13,
                exponent: 1
            },
            MaxPrimePower {
                prime: 17,
                exponent: 1
            },
            MaxPrimePower {
                prime: 19,
                exponent: 1
            },
            MaxPrimePower {
                prime: 23,
                exponent: 1
            },
            MaxPrimePower {
                prime: 29,
                exponent: 1
            }
        ]
    );
}

#[test_log::test]
fn test_megaminx_2_optimal() {
    let megaminx = megaminx();
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
fn test_megaminx_3_optimal() {
    let megaminx = megaminx();
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
fn test_megaminx_2_equivalent() {
    let megaminx = megaminx();
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
fn test_megaminx_3_equivalent() {
    let megaminx = megaminx();
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

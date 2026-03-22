use std::num::NonZeroU16;

use cycle_combination_finder::{
    finder::{CycleCombinationFinder, MaxPrimePower, Optimality, RegisterCount},
    puzzle::{OrbitDef, OrientationSumConstraint, ParityConstraint, PuzzleDef},
};

use crate::common::cycles;

mod common;

fn slow() -> PuzzleDef {
    PuzzleDef::from_orbit_defs_naive(
        vec![
            OrbitDef {
                piece_count: 60.try_into().unwrap(),
                orientation_count: 2.try_into().unwrap(),
                orientation_sum_constraint: OrientationSumConstraint::Zero,
                parity_constraint: ParityConstraint::None,
            },
            OrbitDef {
                piece_count: 40.try_into().unwrap(),
                orientation_count: 3.try_into().unwrap(),
                orientation_sum_constraint: OrientationSumConstraint::Zero,
                parity_constraint: ParityConstraint::None,
            },
        ],
        OrientationSumConstraint::Zero,
        ParityConstraint::Even,
    )
    .unwrap()
}

#[test_log::test]
fn test_slow_max_prime_powers_below() {
    let slow = slow();
    let ccf = CycleCombinationFinder::from(slow);
    let max_prime_powers = ccf.max_prime_powers_below(60);
    assert_eq!(
        max_prime_powers,
        vec![
            MaxPrimePower {
                prime: 2,
                exponent: 6,
            },
            MaxPrimePower {
                prime: 3,
                exponent: 4,
            },
            MaxPrimePower {
                prime: 5,
                exponent: 2,
            },
            MaxPrimePower {
                prime: 7,
                exponent: 2,
            },
            MaxPrimePower {
                prime: 11,
                exponent: 1,
            },
            MaxPrimePower {
                prime: 13,
                exponent: 1,
            },
            MaxPrimePower {
                prime: 17,
                exponent: 1,
            },
            MaxPrimePower {
                prime: 19,
                exponent: 1,
            },
            MaxPrimePower {
                prime: 23,
                exponent: 1,
            },
            MaxPrimePower {
                prime: 29,
                exponent: 1,
            },
            MaxPrimePower {
                prime: 31,
                exponent: 1,
            },
            MaxPrimePower {
                prime: 37,
                exponent: 1,
            },
            MaxPrimePower {
                prime: 41,
                exponent: 1,
            },
            MaxPrimePower {
                prime: 43,
                exponent: 1,
            },
            MaxPrimePower {
                prime: 47,
                exponent: 1,
            },
            MaxPrimePower {
                prime: 53,
                exponent: 1,
            },
            MaxPrimePower {
                prime: 59,
                exponent: 1,
            },
        ]
    );
}

#[test_log::test]
fn test_slow_2_optimal() {
    let slow = slow();
    let ccf = CycleCombinationFinder::from(slow);
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

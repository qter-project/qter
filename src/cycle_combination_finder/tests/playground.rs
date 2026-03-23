use std::num::NonZeroU16;

use cycle_combination_finder::{
    finder::{CycleCombinationFinder, Optimality, RegisterCount},
    puzzle::{
        EvenParityConstraints, OrbitDef, OrientationStatus, OrientationSumConstraint,
        ParityConstraint, PuzzleDef,
    },
};

#[test_log::test]
fn playground() {
    let puzzle = PuzzleDef::from_orbit_defs_naive(
        vec![
            OrbitDef {
                piece_count: 150.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 2,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
                parity_constraint: ParityConstraint::None,
            },
            OrbitDef {
                piece_count: 100.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 3,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
                parity_constraint: ParityConstraint::None,
            },
        ],
        OrientationSumConstraint::Zero,
        EvenParityConstraints(vec![vec![0, 1]]),
    )
    .unwrap();
    let ccf = CycleCombinationFinder::from(puzzle);
    let cycle_combinations = ccf.find(
        Optimality::Optimal,
        RegisterCount::Exactly(NonZeroU16::new(2).unwrap()),
    );
    println!("{:?}", cycle_combinations);
}

use std::num::NonZeroU16;

use cycle_combination_finder::{
    finder::{CycleCombinationFinder, Optimality, RegisterCount},
    puzzle::{OrbitDef, OrientationSumConstraint, ParityConstraint, PuzzleDef},
};

#[test_log::test]
fn playground() {
    let puzzle = PuzzleDef::from_orbit_defs_naive(
        vec![
            OrbitDef {
                piece_count: 150.try_into().unwrap(),
                orientation_count: 2.try_into().unwrap(),
                orientation_sum_constraint: OrientationSumConstraint::Zero,
                parity_constraint: ParityConstraint::None,
            },
            OrbitDef {
                piece_count: 100.try_into().unwrap(),
                orientation_count: 3.try_into().unwrap(),
                orientation_sum_constraint: OrientationSumConstraint::Zero,
                parity_constraint: ParityConstraint::None,
            },
        ],
        OrientationSumConstraint::Zero,
        ParityConstraint::Even,
    )
    .unwrap();
    let ccf = CycleCombinationFinder::from(puzzle);
    let cycle_combinations = ccf.find(
        Optimality::Optimal,
        RegisterCount::Exactly(NonZeroU16::new(2).unwrap()),
    );
    println!("{:?}", cycle_combinations);
}

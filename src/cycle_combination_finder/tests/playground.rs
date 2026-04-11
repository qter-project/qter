#![allow(unused, clippy::useless_vec)]

use std::{
    cmp::{Ordering, Reverse},
    num::NonZeroU16,
};

use bitgauss::BitMatrix;
use cycle_combination_finder::{
    finder::{CycleCombinationFinder, Optimality, RegisterCount},
    puzzle::{
        EvenParityConstraints, OrbitDef, OrientationStatus, OrientationSumConstraint,
        ParityConstraint, PuzzleDef,
    },
};
use fxhash::{FxHashMap, FxHashSet};
use union_find::{QuickUnionUf, UnionByRankSize, UnionBySize, UnionFind};

#[test_log::test]
fn playground() {
    // let even_parity_constraints = vec![vec![0, 1], vec![0, 1], vec![0, 2, 3],
    // vec![0, 4]];
    // let even_parity_constraints = vec![vec![0, 5], vec![1, 5], vec![2, 3]];
    // let even_parity_constraints = vec![vec![0, 1], vec![0, 2, 3], vec![0, 4]];
    // let puzzle = PuzzleDef::new(
    //     vec![
    //         OrbitDef {
    //             piece_count: 150.try_into().unwrap(),
    //             orientation: OrientationStatus::CanOrient {
    //                 count: 2,
    //                 sum_constraint: OrientationSumConstraint::Zero,
    //             },
    //             parity_constraint: ParityConstraint::None,
    //         },
    //         OrbitDef {
    //             piece_count: 100.try_into().unwrap(),
    //             orientation: OrientationStatus::CanOrient {
    //                 count: 3,
    //                 sum_constraint: OrientationSumConstraint::Zero,
    //             },
    //             parity_constraint: ParityConstraint::None,
    //         },
    //     ],
    //     EvenParityConstraints(vec![vec![0, 1]]),
    // )
    // .unwrap();
    // let ccf = CycleCombinationFinder::from(puzzle);
    // let cycle_combinations = ccf.find(
    //     Optimality::Optimal,
    //     RegisterCount::Exactly(NonZeroU16::new(2).unwrap()),
    // );
    // println!("{:?}", cycle_combinations);
}

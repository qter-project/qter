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
    let even_parity_constraints = vec![vec![0, 1], vec![0, 2, 3], vec![0, 4]];
    if let Some(cols) = even_parity_constraints
        .iter()
        .flatten()
        .copied()
        .max()
        .map(|orbit_index| orbit_index + 1)
    {
        let mut rows = even_parity_constraints.len();
        let mut constraints =
            BitMatrix::build(rows, cols, |i, j| even_parity_constraints[i].contains(&j));
        let pivot_cols = constraints.gauss(true);
        let rank = pivot_cols.len();
        if rank != rows {
            rows = rank;
            constraints = BitMatrix::build(rows, cols, |i, j| constraints[(i, j)]);
        }
        let mut uf = QuickUnionUf::<UnionBySize>::new(cols);
        for free_col in (0..cols).filter(|col| !pivot_cols.contains(col)) {
            for row in (0..rows).filter_map(|row| {
                let constraints_row = constraints.row(row);
                // there must be exactly two parity constraints
                if constraints_row.bit(free_col) && constraints_row.count_ones() == 2 {
                    Some(constraints_row)
                } else {
                    None
                }
            }) {
                for equal_orbit_index in row
                    .iter()
                    .enumerate()
                    .filter_map(|(i, bit)| if bit { Some(i) } else { None })
                {
                    uf.union(free_col, equal_orbit_index);
                }
            }
        }
        let mut sizes = FxHashMap::<usize, Vec<usize>>::default();
        for (orbit_index, &root) in uf.link_parent().iter().enumerate() {
            sizes.entry(root).or_default().push(orbit_index);
        }
        let mut sizes = sizes.into_values().collect::<Vec<_>>();
        sizes.sort_by_key(|size| Reverse(size.len()));
        println!("{:?}", sizes);
        println!("{:?}", uf);
        println!("{}", constraints);
    }
    panic!();
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

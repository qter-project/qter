use std::sync::LazyLock;

use puzzle_theory::puzzle_geometry::parsing::puzzle;

use crate::puzzle::{
    EvenParityConstraints, OrbitDef, OrientationStatus, OrientationSumConstraint, ParityConstraint,
    PuzzleDef,
};

pub static CUBE2: LazyLock<PuzzleDef> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![
            OrbitDef {
                piece_count: 8.try_into().unwrap(),
                parity_constraint: ParityConstraint::None,
                orientation: OrientationStatus::CanOrient {
                    count: 3,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
        ],
        EvenParityConstraints(vec![]),
    )
    .unwrap()
});

pub static CUBE3: LazyLock<PuzzleDef> = LazyLock::new(|| {
    PuzzleDef::from_ksolve_naive(
        &puzzle("3x3").ksolve(),
        vec![
            (OrientationSumConstraint::Zero, ParityConstraint::None),
            (OrientationSumConstraint::Zero, ParityConstraint::None),
        ],
        EvenParityConstraints(vec![vec![0, 1]]),
    )
    .unwrap()
});

pub static CUBE4: LazyLock<PuzzleDef> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![
            OrbitDef {
                piece_count: 8.try_into().unwrap(),
                parity_constraint: ParityConstraint::None,
                orientation: OrientationStatus::CanOrient {
                    count: 3,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            OrbitDef {
                piece_count: 24.try_into().unwrap(),
                parity_constraint: ParityConstraint::None,
                orientation: OrientationStatus::CannotOrient,
            },
            OrbitDef {
                piece_count: 24.try_into().unwrap(),
                parity_constraint: ParityConstraint::None,
                orientation: OrientationStatus::CannotOrient,
            },
        ],
        EvenParityConstraints(vec![vec![0, 1]]),
    )
    .unwrap()
});

pub static CUBE5: LazyLock<PuzzleDef> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![
            OrbitDef {
                piece_count: 8.try_into().unwrap(),
                parity_constraint: ParityConstraint::None,
                orientation: OrientationStatus::CanOrient {
                    count: 3,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            OrbitDef {
                piece_count: 12.try_into().unwrap(),
                parity_constraint: ParityConstraint::None,
                orientation: OrientationStatus::CanOrient {
                    count: 2,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            OrbitDef {
                piece_count: 24.try_into().unwrap(),
                parity_constraint: ParityConstraint::None,
                orientation: OrientationStatus::CannotOrient,
            },
            OrbitDef {
                piece_count: 24.try_into().unwrap(),
                parity_constraint: ParityConstraint::None,
                orientation: OrientationStatus::CannotOrient,
            },
            OrbitDef {
                piece_count: 24.try_into().unwrap(),
                parity_constraint: ParityConstraint::None,
                orientation: OrientationStatus::CannotOrient,
            },
        ],
        EvenParityConstraints(vec![vec![0, 1], vec![0, 2, 3], vec![0, 4]]),
    )
    .unwrap()
});

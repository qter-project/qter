use std::sync::LazyLock;

use crate::puzzle::{
    EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef, PuzzleDef,
};

pub static MINX3: LazyLock<PuzzleDef<16>> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![
            PartialOrbitDef {
                piece_count: 20.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 3,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            PartialOrbitDef {
                piece_count: 30.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 2,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
        ],
        EvenParityConstraints(vec![vec![0], vec![1]]),
    )
    .unwrap()
});

pub static MINX4: LazyLock<PuzzleDef<32>> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![
            PartialOrbitDef {
                piece_count: 20.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 3,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            PartialOrbitDef {
                piece_count: 60.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 60.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
        ],
        EvenParityConstraints(vec![vec![0], vec![1], vec![2]]),
    )
    .unwrap()
});

pub static MINX5: LazyLock<PuzzleDef<32>> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![
            PartialOrbitDef {
                piece_count: 20.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 3,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            PartialOrbitDef {
                piece_count: 30.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 2,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            PartialOrbitDef {
                piece_count: 60.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 60.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 60.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
        ],
        EvenParityConstraints(vec![vec![0], vec![1], vec![2], vec![3], vec![4]]),
    )
    .unwrap()
});

pub static MINX6: LazyLock<PuzzleDef<32>> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![
            PartialOrbitDef {
                piece_count: 20.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 3,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            PartialOrbitDef {
                piece_count: 60.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 60.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 60.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 60.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 60.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 60.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
        ],
        EvenParityConstraints(vec![
            vec![0],
            vec![1],
            vec![2],
            vec![3],
            vec![4],
            vec![5],
            vec![6],
        ]),
    )
    .unwrap()
});

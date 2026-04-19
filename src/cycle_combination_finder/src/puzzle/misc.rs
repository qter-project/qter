use std::sync::LazyLock;

use crate::puzzle::{
    EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef, PuzzleDef,
};

pub static BIG1: LazyLock<PuzzleDef<32>> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![
            PartialOrbitDef {
                piece_count: 60.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 2,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            PartialOrbitDef {
                piece_count: 40.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 3,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
        ],
        EvenParityConstraints(vec![vec![0, 1]]),
    )
    .unwrap()
});

pub static BIG2: LazyLock<PuzzleDef<32>> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![
            PartialOrbitDef {
                piece_count: 60.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 20,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            PartialOrbitDef {
                piece_count: 40.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 30,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
        ],
        EvenParityConstraints(vec![vec![0, 1]]),
    )
    .unwrap()
});

pub static BIG3: LazyLock<PuzzleDef<32>> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![
            PartialOrbitDef {
                piece_count: 40.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 3,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            PartialOrbitDef {
                piece_count: 60.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 2,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            PartialOrbitDef {
                piece_count: 20.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 6,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
        ],
        EvenParityConstraints(vec![vec![0, 1, 2]]),
    )
    .unwrap()
});

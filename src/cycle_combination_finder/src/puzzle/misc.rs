use std::sync::LazyLock;

use crate::puzzle::{EvenParityConstraints, OrbitDef, OrientationStatus, OrientationSumConstraint, ParityConstraint, PuzzleDef};

pub static SLOW1: LazyLock<PuzzleDef> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![
            OrbitDef {
                piece_count: 60.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 2,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
                parity_constraint: ParityConstraint::None,
            },
            OrbitDef {
                piece_count: 40.try_into().unwrap(),
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
    .unwrap()
});

pub static SLOW2: LazyLock<PuzzleDef> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![
            OrbitDef {
                piece_count: 120.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 2,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
                parity_constraint: ParityConstraint::None,
            },
            OrbitDef {
                piece_count: 80.try_into().unwrap(),
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
    .unwrap()
});

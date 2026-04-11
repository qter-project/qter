use std::sync::LazyLock;

use crate::puzzle::{
    EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef, PuzzleDef,
};

pub static SLOW: LazyLock<PuzzleDef> = LazyLock::new(|| {
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

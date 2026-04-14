use std::sync::LazyLock;

use puzzle_theory::puzzle_geometry::parsing::puzzle;

use crate::puzzle::{
    EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef, PuzzleDef,
};

pub static CUBE2: LazyLock<PuzzleDef> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![PartialOrbitDef {
            piece_count: 8.try_into().unwrap(),
            orientation: OrientationStatus::CanOrient {
                count: 3,
                sum_constraint: OrientationSumConstraint::Zero,
            },
        }],
        EvenParityConstraints(vec![]),
    )
    .unwrap()
});

pub static CUBE3: LazyLock<PuzzleDef> = LazyLock::new(|| {
    PuzzleDef::from_ksolve_naive(
        &puzzle("3x3").ksolve(),
        vec![
            OrientationSumConstraint::Zero,
            OrientationSumConstraint::Zero,
        ],
        EvenParityConstraints(vec![vec![0, 1]]),
    )
    .unwrap()
});

pub static CUBE4: LazyLock<PuzzleDef> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![
            PartialOrbitDef {
                piece_count: 8.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 3,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
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
            PartialOrbitDef {
                piece_count: 8.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 3,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            PartialOrbitDef {
                piece_count: 12.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 2,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
        ],
        EvenParityConstraints(vec![vec![0, 1], vec![0, 2, 3], vec![0, 4]]),
    )
    .unwrap()
});

pub static CUBE6: LazyLock<PuzzleDef> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![
            PartialOrbitDef {
                piece_count: 8.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 3,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
        ],
        EvenParityConstraints(vec![
            vec![0, 3],
            vec![0, 1, 2, 4],
            vec![0, 1, 2, 5],
            vec![0, 6],
        ]),
    )
    .unwrap()
});

pub static CUBE7: LazyLock<PuzzleDef> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![
            PartialOrbitDef {
                piece_count: 8.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 3,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            PartialOrbitDef {
                piece_count: 12.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 2,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
        ],
        EvenParityConstraints(vec![
            vec![0, 1],
            vec![0, 2, 4],
            vec![0, 3, 5],
            vec![0, 6],
            vec![0, 4, 5, 7],
            vec![0, 4, 5, 8],
            vec![0, 9],
        ]),
    )
    .unwrap()
});

pub static CUBE8: LazyLock<PuzzleDef> = LazyLock::new(|| {
    PuzzleDef::new(
        vec![
            PartialOrbitDef {
                piece_count: 8.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 3,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
            PartialOrbitDef {
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            },
        ],
        EvenParityConstraints(vec![
            vec![0, 4],
            vec![0, 1, 2, 5],
            vec![0, 1, 3, 6],
            vec![0, 2, 1, 7],
            vec![0, 8],
            vec![0, 2, 3, 9],
            vec![0, 3, 1, 10],
            vec![0, 3, 2, 11],
            vec![0, 12],
        ]),
    )
    .unwrap()
});

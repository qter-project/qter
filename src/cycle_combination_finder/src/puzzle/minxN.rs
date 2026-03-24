use std::sync::LazyLock;

use puzzle_theory::puzzle_geometry::parsing::puzzle;

use crate::puzzle::{EvenParityConstraints, OrientationSumConstraint, ParityConstraint, PuzzleDef};

pub static MEGAMINX: LazyLock<PuzzleDef> = LazyLock::new(|| {
    PuzzleDef::from_ksolve_naive(
        &puzzle("megaminx").ksolve(),
        OrientationSumConstraint::Zero,
        EvenParityConstraints(vec![vec![0, 1]]),
        vec![
            (OrientationSumConstraint::Zero, ParityConstraint::Even),
            (OrientationSumConstraint::Zero, ParityConstraint::Even),
        ],
    )
    .unwrap()
});

use std::sync::LazyLock;

use puzzle_theory::puzzle_geometry::parsing::puzzle;

use crate::puzzle::{EvenParityConstraints, OrientationSumConstraint, PuzzleDef};

pub static MEGAMINX: LazyLock<PuzzleDef> = LazyLock::new(|| {
    PuzzleDef::from_ksolve_naive(
        &puzzle("megaminx").ksolve(),
        vec![
            OrientationSumConstraint::Zero,
            OrientationSumConstraint::Zero,
        ],
        EvenParityConstraints(vec![vec![0], vec![1]]),
    )
    .unwrap()
});

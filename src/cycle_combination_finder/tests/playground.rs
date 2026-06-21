#![allow(unused, clippy::useless_vec)]

use std::{
    cmp::{Ordering, Reverse},
    num::NonZeroU16,
};

use bitgauss::BitMatrix;
use cycle_combination_finder::{
    finder::{CycleCombinationFinder, Optimality},
    puzzle::{
        EvenParityConstraints, OrbitDef, OrientationStatus, OrientationSumConstraint,
        ParityConstraint, PuzzleDef,
    },
};
use fxhash::{FxHashMap, FxHashSet};
use union_find::{QuickUnionUf, UnionByRankSize, UnionBySize, UnionFind};

#[test_log::test]
fn playground() {}

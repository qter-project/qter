#![allow(unused_imports, unused_variables)]

use cycle_combination_solver::{
    make_guard,
    pruning::{
        OrbitPruningTables, OrbitPruningTablesGenerateMeta, PruningTables, StorageBackendTy,
        TableTy,
    },
    puzzle::{
        PuzzleDef, PuzzleState, SortedCycleStructure, apply_moves, cube3::Cube3,
        slice_puzzle::HeapPuzzle,
    },
    solver::{CycleStructureSolver, SearchStrategy},
};
use itertools::Itertools;
use log::info;
use puzzle_geometry::ksolve::{KPUZZLE_3X3, KPUZZLE_MEGAMINX, KSolve};

#[test_log::test]
fn playground() {
    
}

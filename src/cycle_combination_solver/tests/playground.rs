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
    make_guard!(guard);
    let megaminx_def = PuzzleDef::<HeapPuzzle>::new(&KPUZZLE_MEGAMINX, guard).unwrap();
    let solved = megaminx_def.new_solved_state();
    let a = apply_moves(&megaminx_def, &solved, "R L D F", 1);
    println!(
        "{:?}",
        a.sorted_cycle_structure(
            megaminx_def.sorted_orbit_defs_ref(),
            &mut HeapPuzzle::new_aux_mem(megaminx_def.sorted_orbit_defs_ref())
        )
    );
    let a = apply_moves(&megaminx_def, &solved, "R L F D", 1);
    println!(
        "{:?}",
        a.sorted_cycle_structure(
            megaminx_def.sorted_orbit_defs_ref(),
            &mut HeapPuzzle::new_aux_mem(megaminx_def.sorted_orbit_defs_ref())
        )
    );
    let a = apply_moves(&megaminx_def, &solved, "R F L D", 1);
    println!(
        "{:?}",
        a.sorted_cycle_structure(
            megaminx_def.sorted_orbit_defs_ref(),
            &mut HeapPuzzle::new_aux_mem(megaminx_def.sorted_orbit_defs_ref())
        )
    );
    let a = apply_moves(&megaminx_def, &solved, "F R L D", 1);
    println!(
        "{:?}",
        a.sorted_cycle_structure(
            megaminx_def.sorted_orbit_defs_ref(),
            &mut HeapPuzzle::new_aux_mem(megaminx_def.sorted_orbit_defs_ref())
        )
    );
    panic!();
}

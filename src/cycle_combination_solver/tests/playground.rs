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
use puzzle_theory::puzzle_geometry::parsing::puzzle;

#[test_log::test]
fn playground() {
    make_guard!(guard);
    let megaminx_def = PuzzleDef::<HeapPuzzle>::new(&puzzle("megaminx").ksolve(), guard).unwrap();
    // info!(
    //     "{}",
    //     megaminx_def.moves.iter().map(|m| m.name()).format(" ")
    // );
    // let a = apply_moves(&megaminx_def, &solved, "F R L D", 1);
    let d_move = megaminx_def.find_move("D").unwrap();
    let r_move = megaminx_def.find_move("R").unwrap();
    let f_move = megaminx_def.find_move("F").unwrap();
    let u_move = megaminx_def.find_move("L").unwrap();
    let mut aux_mem = HeapPuzzle::new_aux_mem(megaminx_def.sorted_orbit_defs_ref());

    let moves = [0, 1, 2, 3].iter().copied().permutations(4).collect_vec();
    let moves2 = [d_move, r_move, f_move, u_move];

    for v in moves {
        // let v: [usize; 4] = v.try_into().unwrap();
        // let [i, j, k, l] = v;
        let i = v[0];
        let j = v[1];
        let k = v[2];
        let l = v[3];
        let mut result1 = megaminx_def.new_solved_state();
        let mut result = megaminx_def.new_solved_state();
        result.replace_compose(
            &result1,
            moves2[i].puzzle_state(),
            megaminx_def.sorted_orbit_defs_ref(),
        );
        result1.replace_compose(
            &result,
            moves2[j].puzzle_state(),
            megaminx_def.sorted_orbit_defs_ref(),
        );
        result.replace_compose(
            &result1,
            moves2[k].puzzle_state(),
            megaminx_def.sorted_orbit_defs_ref(),
        );
        result1.replace_compose(
            &result,
            moves2[l].puzzle_state(),
            megaminx_def.sorted_orbit_defs_ref(),
        );
        println!(
            "{:?} {} {} {} {}",
            result1.sorted_cycle_structure(megaminx_def.sorted_orbit_defs_ref(), &mut aux_mem),
            moves2[i].name(),
            moves2[j].name(),
            moves2[k].name(),
            moves2[l].name(),
        );
    }
    panic!();
}

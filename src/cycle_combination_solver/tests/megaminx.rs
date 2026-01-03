use cycle_combination_solver::{
    pruning::{PruningTables, ZeroTable},
    puzzle::{PuzzleDef, PuzzleState, SortedCycleStructure, apply_moves, slice_puzzle::HeapPuzzle},
    solver::{CycleStructureSolver, SearchStrategy},
};
use generativity::make_guard;
use puzzle_theory::puzzle_geometry::parsing::puzzle;

#[test_log::test]
fn test_move_powers() {
    make_guard!(guard);
    let megaminx_def = PuzzleDef::<HeapPuzzle>::new(&puzzle("megaminx").ksolve(), guard).unwrap();
    let sorted_orbit_defs = megaminx_def.sorted_orbit_defs_ref();
    let mut aux_mem = HeapPuzzle::new_aux_mem(sorted_orbit_defs);
    let solved = megaminx_def.new_solved_state();

    for (moves_str, expected_sorted_cycle_structure) in [
        ("U F", &[vec![(1, true), (7, true)], vec![(9, false)]]),
        ("U F2", &[vec![(3, true), (5, true)], vec![(9, false)]]),
        ("U F2'", &[vec![(2, true), (6, true)], vec![(9, false)]]),
        ("U F'", &[vec![(4, true), (4, true)], vec![(9, false)]]),
    ] {
        let test = apply_moves(&megaminx_def, &solved, moves_str, 1);
        assert!(
            test.induces_sorted_cycle_structure(
                SortedCycleStructure::new(expected_sorted_cycle_structure, sorted_orbit_defs)
                    .unwrap()
                    .as_ref(),
                sorted_orbit_defs,
                aux_mem.as_ref_mut(),
            )
        );
    }
}

#[test_log::test]
fn test_random1() {
    make_guard!(guard);
    let megaminx_def = PuzzleDef::<HeapPuzzle>::new(&puzzle("megaminx").ksolve(), guard).unwrap();
    let sorted_cycle_structure = SortedCycleStructure::new(
        &[
            vec![(2, true), (14, true)],
            vec![(5, true), (6, false), (10, true)],
        ],
        megaminx_def.sorted_orbit_defs_ref(),
    )
    .unwrap();
    let solver: CycleStructureSolver<HeapPuzzle, _> = CycleStructureSolver::new(
        megaminx_def,
        ZeroTable::try_generate_all(sorted_cycle_structure, ()).unwrap(),
        SearchStrategy::AllSolutions,
    );

    let mut solutions = solver.solve::<Vec<_>>().unwrap();
    assert_eq!(solutions.solution_length(), 6);
    while solutions.next().is_some() {}
    assert_eq!(solutions.expanded_count(), 165600);
}

#[test_log::test]
fn test_random2() {
    make_guard!(guard);
    let megaminx_def = PuzzleDef::<HeapPuzzle>::new(&puzzle("megaminx").ksolve(), guard).unwrap();
    let sorted_cycle_structure = SortedCycleStructure::new(
        &[
            vec![(1, true), (1, true), (5, false), (9, true)],
            vec![(5, false), (13, false)],
        ],
        megaminx_def.sorted_orbit_defs_ref(),
    )
    .unwrap();
    let solver: CycleStructureSolver<HeapPuzzle, _> = CycleStructureSolver::new(
        megaminx_def,
        ZeroTable::try_generate_all(sorted_cycle_structure, ()).unwrap(),
        SearchStrategy::AllSolutions,
    );

    let mut solutions = solver.solve::<Vec<_>>().unwrap();
    assert_eq!(solutions.solution_length(), 4);
    while solutions.next().is_some() {}
    assert_eq!(solutions.expanded_count(), 23040);
}

#[test_log::test]
fn test_random3() {
    make_guard!(guard);
    let megaminx_def = PuzzleDef::<HeapPuzzle>::new(&puzzle("megaminx").ksolve(), guard).unwrap();
    let sorted_cycle_structure = SortedCycleStructure::new(
        &[vec![(1, true), (1, true), (9, true)], vec![(13, false)]],
        megaminx_def.sorted_orbit_defs_ref(),
    )
    .unwrap();
    let solver: CycleStructureSolver<HeapPuzzle, _> = CycleStructureSolver::new(
        megaminx_def,
        ZeroTable::try_generate_all(sorted_cycle_structure, ()).unwrap(),
        SearchStrategy::AllSolutions,
    );

    let mut solutions = solver.solve::<Vec<_>>().unwrap();
    assert_eq!(solutions.solution_length(), 3);
    while solutions.next().is_some() {}
    assert_eq!(solutions.expanded_count(), 720);
}

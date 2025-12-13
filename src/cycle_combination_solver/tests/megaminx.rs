use cycle_combination_solver::{pruning::{PruningTables, ZeroTable}, puzzle::{PuzzleDef, SortedCycleStructure, slice_puzzle::HeapPuzzle}, solver::{CycleStructureSolver, SearchStrategy}};
use generativity::make_guard;
use itertools::Itertools;
use log::trace;
use puzzle_geometry::ksolve::KPUZZLE_MEGAMINX;

#[test_log::test]
fn test_random_order1() {
    make_guard!(guard);
    let megaminx_def = PuzzleDef::<HeapPuzzle>::new(&KPUZZLE_MEGAMINX, guard).unwrap();
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
    while solutions.next().is_some() {
        trace!(
            "{:<2}",
            solutions
                .expanded_solution()
                .iter()
                .map(|move_| move_.name())
                .format(" ")
        );
    }
    assert_eq!(solutions.expanded_count(), 66444);
    panic!();
}

#[test_log::test]
fn test_random_order2() {
    make_guard!(guard);
    let megaminx_def = PuzzleDef::<HeapPuzzle>::new(&KPUZZLE_MEGAMINX, guard).unwrap();
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
    while solutions.next().is_some() {
        trace!(
            "{:<2}",
            solutions
                .expanded_solution()
                .iter()
                .map(|move_| move_.name())
                .format(" ")
        );
    }
    assert_eq!(solutions.expanded_count(), 80856);
}

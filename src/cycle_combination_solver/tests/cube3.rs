use crate::common::OptimalCycleStructureTest;
use cycle_combination_solver::{
    make_guard,
    pruning::{
        OrbitPruningTables, OrbitPruningTablesGenerateMeta, PruningTables, StorageBackendTy,
        TableTy, ZeroTable,
    },
    puzzle::{
        PuzzleDef, PuzzleState, SortedCycleStructure, cube3::Cube3, slice_puzzle::HeapPuzzle,
    },
    solver::{CycleStructureSolver, CycleStructureSolverError, SearchStrategy},
};
use itertools::Itertools;
use log::trace;
use puzzle_geometry::ksolve::KPUZZLE_3X3;

mod common;

#[test_log::test]
fn test_identity_cycle_structure() {
    make_guard!(guard);
    let mut cube3_def = PuzzleDef::<Cube3>::new(&KPUZZLE_3X3, guard).unwrap();
    let identity_cycle_structure =
        SortedCycleStructure::new(&[vec![], vec![]], cube3_def.sorted_orbit_defs_ref()).unwrap();

    let solver: CycleStructureSolver<Cube3, _> = CycleStructureSolver::new(
        cube3_def,
        ZeroTable::try_generate_all(identity_cycle_structure.clone(), ()).unwrap(),
        SearchStrategy::AllSolutions,
    );
    let mut solutions = solver.solve::<[Cube3; 21]>().unwrap();
    assert_eq!(solutions.solution_length(), 0);
    solutions.next().unwrap();
    assert!(solutions.next().is_none());

    cube3_def = solver.into_puzzle_def_and_pruning_tables().0;

    let pruning_tables = OrbitPruningTables::try_generate_all(
        identity_cycle_structure,
        OrbitPruningTablesGenerateMeta::new_with_table_types(
            &cube3_def,
            vec![TableTy::Zero, TableTy::Zero],
            0,
            cube3_def.id(),
        )
        .unwrap(),
    )
    .unwrap();
    let solver: CycleStructureSolver<Cube3, _> =
        CycleStructureSolver::new(cube3_def, pruning_tables, SearchStrategy::AllSolutions);
    let mut solutions = solver.solve::<[Cube3; 21]>().unwrap();
    assert_eq!(solutions.solution_length(), 0);
    solutions.next().unwrap();
    assert!(solutions.next().is_none());
}

#[test_log::test]
fn test_single_quarter_turn() {
    make_guard!(guard);
    let cube3_def = PuzzleDef::<Cube3>::new(&KPUZZLE_3X3, guard).unwrap();
    let sorted_cycle_structure = SortedCycleStructure::new(
        &[vec![(4, false)], vec![(4, false)]],
        cube3_def.sorted_orbit_defs_ref(),
    )
    .unwrap();
    let solver: CycleStructureSolver<Cube3, _> = CycleStructureSolver::new(
        cube3_def,
        ZeroTable::try_generate_all(sorted_cycle_structure, ()).unwrap(),
        SearchStrategy::AllSolutions,
    );
    let mut solutions = solver.solve::<[Cube3; 21]>().unwrap();
    assert_eq!(solutions.solution_length(), 1);
    while solutions.next().is_some() {
        trace!(
            "{:<2}",
            solutions
                .expanded_solution()
                .iter()
                .map(|move_| move_.name())
                .format(" ")
        );
        if solutions.expanded_count() == 1 {
            // assert_eq!(
            //     format!(
            //         "{}",
            //         solutions
            //             .expanded_solution()
            //             .iter()
            //             .map(|move_| move_.name())
            //             .format(" ")
            //     ),
            //     "F"
            // );
        }
    }
    assert_eq!(solutions.expanded_count(), 12);
}

#[test_log::test]
fn test_single_half_turn() {
    make_guard!(guard);
    let cube3_def = PuzzleDef::<Cube3>::new(&KPUZZLE_3X3, guard).unwrap();
    let sorted_cycle_structure = SortedCycleStructure::new(
        &[vec![(2, false), (2, false)], vec![(2, false), (2, false)]],
        cube3_def.sorted_orbit_defs_ref(),
    )
    .unwrap();
    let solver: CycleStructureSolver<Cube3, _> = CycleStructureSolver::new(
        cube3_def,
        ZeroTable::try_generate_all(sorted_cycle_structure, ()).unwrap(),
        SearchStrategy::AllSolutions,
    );
    let mut solutions = solver.solve::<[Cube3; 21]>().unwrap();
    assert_eq!(solutions.solution_length(), 1);
    while solutions.next().is_some() {
        trace!(
            "{:<2}",
            solutions
                .expanded_solution()
                .iter()
                .map(|move_| move_.name())
                .format(" ")
        );

        if solutions.expanded_count() == 1 {
            // assert_eq!(
            //     format!(
            //         "{}",
            //         solutions
            //             .expanded_solution()
            //             .iter()
            //             .map(|move_| move_.name())
            //             .format(" ")
            //     ),
            //     "F2"
            // );
        }
    }
    assert_eq!(solutions.expanded_count(), 6);
}

#[test_log::test]
fn test_optimal_subgroup_cycle() {
    make_guard!(guard);
    let cube3_def = PuzzleDef::<Cube3>::new(
        &KPUZZLE_3X3
            .clone()
            .with_moves(&["F", "F2", "F'", "R", "R2", "R'", "U", "U2", "U'"]),
        guard,
    )
    .unwrap();
    let sorted_cycle_structure = SortedCycleStructure::new(
        &[vec![(3, false), (4, false)], vec![(1, true), (8, true)]],
        cube3_def.sorted_orbit_defs_ref(),
    )
    .unwrap();
    let solver: CycleStructureSolver<Cube3, _> = CycleStructureSolver::new(
        cube3_def,
        ZeroTable::try_generate_all(sorted_cycle_structure, ()).unwrap(),
        SearchStrategy::AllSolutions,
    );
    let mut solutions = solver.solve::<[Cube3; 21]>().unwrap();
    assert_eq!(solutions.solution_length(), 4);
    while solutions.next().is_some() {
        trace!(
            "{:<2}",
            solutions
                .expanded_solution()
                .iter()
                .map(|move_| move_.name())
                .format(" ")
        );
        if solutions.expanded_count() == 1 {
            // assert_eq!(
            //     format!(
            //         "{}",
            //         solutions
            //             .expanded_solution()
            //             .iter()
            //             .map(|move_| move_.name())
            //             .format(" ")
            //     ),
            //     "F2"
            // );
        }
    }
    assert_eq!(solutions.expanded_count(), 24);
}

#[test_log::test]
fn test_210_optimal_cycle() {
    make_guard!(guard);
    let cube3_def = PuzzleDef::<Cube3>::new(&KPUZZLE_3X3, guard).unwrap();
    let sorted_cycle_structure = SortedCycleStructure::new(
        &[vec![(1, true), (5, true)], vec![(1, true), (7, true)]],
        cube3_def.sorted_orbit_defs_ref(),
    )
    .unwrap();
    let solver: CycleStructureSolver<Cube3, _> = CycleStructureSolver::new(
        cube3_def,
        ZeroTable::try_generate_all(sorted_cycle_structure, ()).unwrap(),
        SearchStrategy::AllSolutions,
    );

    let mut solutions = solver.solve::<[Cube3; 21]>().unwrap();
    assert_eq!(solutions.solution_length(), 5);
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
    assert_eq!(solutions.expanded_count(), 480);
}

#[test_log::test]
fn test_easy_30x30x30_optimal_cycle() {
    make_guard!(guard);
    let cube3_def = PuzzleDef::<Cube3>::new(&KPUZZLE_3X3, guard).unwrap();
    let sorted_cycle_structure = SortedCycleStructure::new(
        &[vec![(1, true), (1, true)], vec![(1, true), (5, true)]],
        cube3_def.sorted_orbit_defs_ref(),
    )
    .unwrap();
    let generate_meta = OrbitPruningTablesGenerateMeta::new_with_table_types(
        &cube3_def,
        vec![
            TableTy::Exact(StorageBackendTy::Uncompressed),
            TableTy::Zero,
        ],
        88_179_840,
        cube3_def.id(),
    )
    .unwrap();
    let pruning_tables =
        OrbitPruningTables::try_generate_all(sorted_cycle_structure, generate_meta).unwrap();
    let solver: CycleStructureSolver<Cube3, _> =
        CycleStructureSolver::new(cube3_def, pruning_tables, SearchStrategy::AllSolutions);

    let mut solutions = solver.solve::<[Cube3; 21]>().unwrap();
    assert_eq!(solutions.solution_length(), 11);
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
    assert_eq!(solutions.expanded_count(), 2112);
}

#[test_log::test]
fn test_hard_30x30x30_optimal_cycle() {
    make_guard!(guard);
    let cube3_def = PuzzleDef::<Cube3>::new(&KPUZZLE_3X3, guard).unwrap();
    let sorted_cycle_structure = SortedCycleStructure::new(
        &[vec![(1, true), (5, true)], vec![(1, true), (1, true)]],
        cube3_def.sorted_orbit_defs_ref(),
    )
    .unwrap();
    let generate_meta = OrbitPruningTablesGenerateMeta::new_with_table_types(
        &cube3_def,
        vec![
            TableTy::Exact(StorageBackendTy::Uncompressed),
            TableTy::Zero,
        ],
        88_179_840,
        cube3_def.id(),
    )
    .unwrap();
    let pruning_tables =
        OrbitPruningTables::try_generate_all(sorted_cycle_structure, generate_meta).unwrap();

    let solver: CycleStructureSolver<Cube3, _> =
        CycleStructureSolver::new(cube3_def, pruning_tables, SearchStrategy::AllSolutions)
            .with_max_solution_length(10);

    let failed = solver.solve::<[Cube3; 21]>().unwrap_err();
    assert!(matches!(
        failed,
        CycleStructureSolverError::MaxSolutionLengthExceeded
    ));

    panic!();
}

#[test_log::test]
fn test_3c_optimal_cycle() {
    make_guard!(guard);
    let cube3_def = PuzzleDef::<Cube3>::new(&KPUZZLE_3X3, guard).unwrap();
    let sorted_cycle_structure = SortedCycleStructure::new(
        &[vec![(3, false)], vec![]],
        cube3_def.sorted_orbit_defs_ref(),
    )
    .unwrap();
    let solver: CycleStructureSolver<Cube3, _> = CycleStructureSolver::new(
        cube3_def,
        ZeroTable::try_generate_all(sorted_cycle_structure, ()).unwrap(),
        SearchStrategy::AllSolutions,
    );

    let mut solutions = solver.solve::<[Cube3; 21]>().unwrap();
    assert_eq!(solutions.solution_length(), 8);
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
    assert_eq!(solutions.expanded_count(), 864);
}

#[test_log::test]
fn test_8c8e_optimal_cycle() {
    make_guard!(guard);
    let cube3_def = PuzzleDef::<Cube3>::new(&KPUZZLE_3X3, guard).unwrap();
    let sorted_cycle_structure = SortedCycleStructure::new(
        &[vec![(8, false)], vec![(8, false)]],
        cube3_def.sorted_orbit_defs_ref(),
    )
    .unwrap();
    let solver: CycleStructureSolver<Cube3, _> = CycleStructureSolver::new(
        cube3_def,
        ZeroTable::try_generate_all(sorted_cycle_structure, ()).unwrap(),
        SearchStrategy::AllSolutions,
    );

    let mut solutions = solver.solve::<[Cube3; 21]>().unwrap();
    assert_eq!(solutions.solution_length(), 5);
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
    assert_eq!(solutions.expanded_count(), 960);
}

#[test_log::test]
fn test_consecutive_commutative_moves() {
    make_guard!(guard);
    let cube3_def = PuzzleDef::<Cube3>::new(&KPUZZLE_3X3, guard).unwrap();
    let sorted_cycle_structure = SortedCycleStructure::new(
        &[
            vec![
                (1, true),
                (1, true),
                (1, true),
                (1, true),
                (2, true),
                (2, true),
            ],
            vec![(6, false), (6, false)],
        ],
        cube3_def.sorted_orbit_defs_ref(),
    )
    .unwrap();
    let solver: CycleStructureSolver<Cube3, _> = CycleStructureSolver::new(
        cube3_def,
        ZeroTable::try_generate_all(sorted_cycle_structure, ()).unwrap(),
        SearchStrategy::AllSolutions,
    );

    let mut solutions = solver.solve::<[Cube3; 21]>().unwrap();
    assert_eq!(solutions.solution_length(), 4);
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
    assert_eq!(solutions.expanded_count(), 96);
}

#[test_log::test]
fn test_commutative_moves_only() {
    make_guard!(guard);
    let cube3_def = PuzzleDef::<Cube3>::new(&KPUZZLE_3X3, guard).unwrap();
    let sorted_cycle_structure = SortedCycleStructure::new(
        &[vec![(4, false), (4, false)], vec![(4, false), (4, false)]],
        cube3_def.sorted_orbit_defs_ref(),
    )
    .unwrap();
    let solver: CycleStructureSolver<Cube3, _> = CycleStructureSolver::new(
        cube3_def,
        ZeroTable::try_generate_all(sorted_cycle_structure, ()).unwrap(),
        SearchStrategy::AllSolutions,
    );

    let mut solutions = solver.solve::<[Cube3; 21]>().unwrap();
    assert_eq!(solutions.solution_length(), 2);
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
    assert_eq!(solutions.expanded_count(), 12);
}

#[test_log::test]
fn test_sequence_symmetry_wrapping_sequence_edge_case() {
    make_guard!(guard);
    let cube3_def = PuzzleDef::<Cube3>::new(&KPUZZLE_3X3, guard).unwrap();
    let sorted_cycle_structure = SortedCycleStructure::new(
        &[vec![(2, false), (2, false), (3, false)], vec![(9, false)]],
        cube3_def.sorted_orbit_defs_ref(),
    )
    .unwrap();
    let solver: CycleStructureSolver<Cube3, _> = CycleStructureSolver::new(
        cube3_def,
        ZeroTable::try_generate_all(sorted_cycle_structure, ()).unwrap(),
        SearchStrategy::AllSolutions,
    );

    let mut solutions = solver.solve::<[Cube3; 21]>().unwrap();
    assert_eq!(solutions.solution_length(), 7);
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
    assert_eq!(solutions.expanded_count(), 6048);
}

#[test_log::test]
fn test_many_optimal_cycles() {
    make_guard!(guard);
    let mut cube3_def = PuzzleDef::<HeapPuzzle>::new(&KPUZZLE_3X3, guard).unwrap();

    // Test cases taken from Michael Gottlieb's order table
    // https://mzrg.com/rubik/orders.shtml
    let optimal_cycle_structure_tests = [
        OptimalCycleStructureTest {
            moves_str: "U2 R2 U2 R2",
            expected_count: 24,
        },
        OptimalCycleStructureTest {
            moves_str: "U R U R'",
            expected_count: 192,
        },
        OptimalCycleStructureTest {
            moves_str: "U2 R2",
            expected_count: 24,
        },
        OptimalCycleStructureTest {
            moves_str: "U R U' F",
            expected_count: 384,
        },
        OptimalCycleStructureTest {
            moves_str: "U R2 D",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "U R F2",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "U R U' F2",
            expected_count: 384,
        },
        OptimalCycleStructureTest {
            moves_str: "U R2 B2",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "U R2 U R2",
            expected_count: 96,
        },
        OptimalCycleStructureTest {
            moves_str: "U R U' L2",
            expected_count: 384,
        },
        OptimalCycleStructureTest {
            moves_str: "U R2 F R2",
            expected_count: 480,
        },
        OptimalCycleStructureTest {
            moves_str: "U R2 D2",
            expected_count: 432,
        },
        OptimalCycleStructureTest {
            moves_str: "U R U' L",
            expected_count: 384,
        },
        OptimalCycleStructureTest {
            moves_str: "U R2",
            expected_count: 96,
        },
        OptimalCycleStructureTest {
            moves_str: "U R F' D'",
            expected_count: 384,
        },
        OptimalCycleStructureTest {
            moves_str: "U R2 D' R2",
            expected_count: 96,
        },
        OptimalCycleStructureTest {
            moves_str: "U R B2",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "U R U2 L",
            expected_count: 768,
        },
        OptimalCycleStructureTest {
            moves_str: "U R2 U2 R'",
            expected_count: 192,
        },
        OptimalCycleStructureTest {
            moves_str: "U R B' D'",
            expected_count: 384,
        },
        OptimalCycleStructureTest {
            moves_str: "U R U L",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "U R U2 B",
            expected_count: 192,
        },
        OptimalCycleStructureTest {
            moves_str: "U R B' L2",
            expected_count: 1152,
        },
        OptimalCycleStructureTest {
            moves_str: "U R B'",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "U R'",
            expected_count: 48,
        },
        OptimalCycleStructureTest {
            moves_str: "U R U F'",
            expected_count: 384,
        },
        OptimalCycleStructureTest {
            moves_str: "U R B' L",
            expected_count: 192,
        },
        OptimalCycleStructureTest {
            moves_str: "U R B",
            expected_count: 48,
        },
        OptimalCycleStructureTest {
            moves_str: "U R F",
            expected_count: 48,
        },
        OptimalCycleStructureTest {
            moves_str: "U R D",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "U R2 F L2",
            expected_count: 192,
        },
        OptimalCycleStructureTest {
            moves_str: "U R",
            expected_count: 48,
        },
        OptimalCycleStructureTest {
            moves_str: "U R U L2",
            expected_count: 384,
        },
        OptimalCycleStructureTest {
            moves_str: "U R U F2",
            expected_count: 384,
        },
        OptimalCycleStructureTest {
            moves_str: "U R F' L",
            expected_count: 192,
        },
        OptimalCycleStructureTest {
            moves_str: "U R' U F'",
            expected_count: 192,
        },
        OptimalCycleStructureTest {
            moves_str: "U R D2",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "U R D'",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "U R B2 F2",
            expected_count: 384,
        },
        OptimalCycleStructureTest {
            moves_str: "U R2 U F'",
            expected_count: 384,
        },
        OptimalCycleStructureTest {
            moves_str: "U R F2 L2",
            expected_count: 3456,
        },
        OptimalCycleStructureTest {
            moves_str: "U R2 B L'",
            expected_count: 192,
        },
        OptimalCycleStructureTest {
            moves_str: "U R F L",
            expected_count: 96,
        },
        OptimalCycleStructureTest {
            moves_str: "U R D L",
            expected_count: 48,
        },
        OptimalCycleStructureTest {
            moves_str: "U R F'",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "U R U2 F",
            expected_count: 192,
        },
        OptimalCycleStructureTest {
            moves_str: "U R2 F L'",
            expected_count: 192,
        },
        OptimalCycleStructureTest {
            moves_str: "U R B2 F'",
            expected_count: 384,
        },
    ];

    let solved = cube3_def.new_solved_state();
    let mut aux_mem = HeapPuzzle::new_aux_mem(cube3_def.sorted_orbit_defs_ref());

    for optimal_cycle_test in optimal_cycle_structure_tests.into_iter() {
        let mut result_1 = solved.clone();
        let mut result_2 = solved.clone();
        let mut move_count = 0;
        for name in optimal_cycle_test.moves_str.split_whitespace() {
            let move_ = cube3_def.find_move(name).unwrap();
            result_2.replace_compose(
                &result_1,
                move_.puzzle_state(),
                cube3_def.sorted_orbit_defs_ref(),
            );
            std::mem::swap(&mut result_1, &mut result_2);
            move_count += 1;
        }

        let sorted_cycle_structure =
            result_1.sorted_cycle_structure(cube3_def.sorted_orbit_defs_ref(), &mut aux_mem);

        let zero_table = ZeroTable::try_generate_all(sorted_cycle_structure, ()).unwrap();

        let solver: CycleStructureSolver<HeapPuzzle, _> =
            CycleStructureSolver::new(cube3_def, zero_table, SearchStrategy::AllSolutions);

        let mut solutions = solver.solve::<Vec<_>>().unwrap();
        assert_eq!(solutions.solution_length(), move_count);
        while solutions.next().is_some() {}
        assert_eq!(
            solutions.expanded_count(),
            optimal_cycle_test.expected_count,
        );

        cube3_def = solver.into_puzzle_def_and_pruning_tables().0;
    }
}

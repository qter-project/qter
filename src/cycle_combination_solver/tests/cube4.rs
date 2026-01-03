use crate::common::OptimalCycleStructureTest;
use cycle_combination_solver::{
    pruning::{PruningTables, ZeroTable},
    puzzle::{PuzzleDef, PuzzleState, slice_puzzle::HeapPuzzle},
    solver::{CycleStructureSolver, SearchStrategy},
};
use generativity::make_guard;
use puzzle_theory::puzzle_geometry::parsing::puzzle;

mod common;

#[test_log::test]
#[ignore = "big cube stuff isnt working without puzzle working"]
fn test_big_cube_optimal_cycle() {
    make_guard!(guard);
    let mut cube4_def = PuzzleDef::<HeapPuzzle>::new(&puzzle("4x4").ksolve(), guard).unwrap();

    // Test cases taken from Michael Gottlieb's order table
    // https://mzrg.com/rubik/orders.shtml
    let mut optimal_cycle_structure_tests = [
        OptimalCycleStructureTest {
            moves_str: "R2",
            expected_count: 6,
        },
        OptimalCycleStructureTest {
            moves_str: "r2 u2",
            expected_count: 24,
        },
        OptimalCycleStructureTest {
            moves_str: "R",
            expected_count: 12,
        },
        OptimalCycleStructureTest {
            moves_str: "R2 U2",
            expected_count: 24,
        },
        OptimalCycleStructureTest {
            moves_str: "r u' f2",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "r u'",
            expected_count: 48,
        },
        OptimalCycleStructureTest {
            moves_str: "r u",
            expected_count: 48,
        },
        OptimalCycleStructureTest {
            moves_str: "R L' 2U",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R 2U",
            expected_count: 192,
        },
        OptimalCycleStructureTest {
            moves_str: "r l2 u",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "2R 2U",
            expected_count: 96,
        },
        OptimalCycleStructureTest {
            moves_str: "R U2",
            expected_count: 96,
        },
        OptimalCycleStructureTest {
            moves_str: "R L 2U",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R U'",
            expected_count: 48,
        },
        OptimalCycleStructureTest {
            moves_str: "r 2U",
            expected_count: 192,
        },
        OptimalCycleStructureTest {
            moves_str: "F U R",
            expected_count: 48,
        },
        OptimalCycleStructureTest {
            moves_str: "R' 2U 2F'",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R L U",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "R U",
            expected_count: 48,
        },
        OptimalCycleStructureTest {
            moves_str: "R l' 2U",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R u' 2F'",
            expected_count: 576,
        },
        OptimalCycleStructureTest {
            moves_str: "r' 2U 2F",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "R L2 U",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R L' U",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R u",
            expected_count: 96,
        },
        OptimalCycleStructureTest {
            moves_str: "R u 2F'",
            expected_count: 576,
        },
        OptimalCycleStructureTest {
            moves_str: "r 2U' 2F'",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "R u f",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "r' 2U 2F'",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R u' 2F",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "F U R'",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "R U f'",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "R u' 2L",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R u' 2L'",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R u'",
            expected_count: 96,
        },
        OptimalCycleStructureTest {
            moves_str: "R' U' f",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "R2 u f'",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R U' f'",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R U l",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "r U' 2L'",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R2 u 2F",
            expected_count: 576,
        },
        OptimalCycleStructureTest {
            moves_str: "R u 2L",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R l u'",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R2 u' f'",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "R l' u'",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R' U2 f",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R U l'",
            expected_count: 576,
        },
        OptimalCycleStructureTest {
            moves_str: "r' u' 2F2",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "r u' 2F2",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R u' f'",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "R u 2L'",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R l u",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "r' u' 2F",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "R2 u f",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "r u 2L2",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R u 2F2",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "r u 2L",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R2 l u'",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R2 l u",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R l2 u'",
            expected_count: 288,
        },
        OptimalCycleStructureTest {
            moves_str: "R' u f",
            expected_count: 144,
        },
        OptimalCycleStructureTest {
            moves_str: "R2 r u'",
            expected_count: 864,
        },
        OptimalCycleStructureTest {
            moves_str: "R2 r u",
            expected_count: 864,
        },
    ];

    fastrand::shuffle(&mut optimal_cycle_structure_tests);
    // only do 5 because this is slow
    let optimal_cycle_structure_tests = &optimal_cycle_structure_tests[0..5];

    let solved = cube4_def.new_solved_state();
    let mut aux_mem = HeapPuzzle::new_aux_mem(cube4_def.sorted_orbit_defs_ref());

    for optimal_cycle_test in optimal_cycle_structure_tests {
        let mut result_1 = solved.clone();
        let mut result_2 = solved.clone();
        let mut move_count = 0;
        for name in optimal_cycle_test.moves_str.split_whitespace() {
            let move_ = cube4_def.find_move(name).unwrap();
            result_2.replace_compose(
                &result_1,
                move_.puzzle_state(),
                cube4_def.sorted_orbit_defs_ref(),
            );
            std::mem::swap(&mut result_1, &mut result_2);
            move_count += 1;
        }

        let sorted_cycle_structure =
            result_1.sorted_cycle_structure(cube4_def.sorted_orbit_defs_ref(), &mut aux_mem);

        let zero_table = ZeroTable::try_generate_all(sorted_cycle_structure, ()).unwrap();

        let solver: CycleStructureSolver<HeapPuzzle, _> =
            CycleStructureSolver::new(cube4_def, zero_table, SearchStrategy::AllSolutions);

        let mut solutions = solver.solve::<Vec<_>>().unwrap();
        assert_eq!(solutions.solution_length(), move_count);
        while solutions.next().is_some() {}
        assert_eq!(
            solutions.expanded_count(),
            optimal_cycle_test.expected_count,
        );

        cube4_def = solver.into_puzzle_def_and_pruning_tables().0;
    }
}

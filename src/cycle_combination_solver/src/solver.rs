use super::{
    canonical_fsm::{CanonicalFSMState, PuzzleCanonicalFSM},
    pruning::PruningTables,
    puzzle::{Move, PuzzleDef, PuzzleState},
    puzzle_state_history::{PuzzleStateHistory, StackedPuzzleStateHistory},
};
use crate::{puzzle::AuxMem, start, success, working};
use humanize_duration::{Truncate, prelude::DurationExt};
use itertools::Itertools;
use log::{Level, debug, info, log_enabled, trace};
use std::{borrow::Cow, cmp::Ordering, time::Instant, vec::IntoIter};
use thiserror::Error;

pub struct CycleStructureSolver<'id, P: PuzzleState<'id>, T: PruningTables<'id, P>> {
    puzzle_def: PuzzleDef<'id, P>,
    pruning_tables: T,
    canonical_fsm: PuzzleCanonicalFSM<'id, P>,
    max_solution_length: Option<usize>,
    search_strategy: SearchStrategy,
}

struct CycleStructureSolverMutable<'id, P: PuzzleState<'id>, H: PuzzleStateHistory<'id, P>> {
    puzzle_state_history: StackedPuzzleStateHistory<'id, P, H>,
    aux_mem: AuxMem<'id>,
    solutions: Vec<Vec<usize>>,
    root_canonical_fsm_reversed_state: usize,
    traversed: u64,
    tmp: u64,
}

#[derive(Error, Debug)]
pub enum CycleStructureSolverError {
    #[error("A deep search still did not find a solution. It is unlikely that one exists")]
    SolutionDoesNotExist,
    #[error("Max solution length exceeded")]
    MaxSolutionLengthExceeded,
    #[error("Time limit exceeded")]
    TimeLimitExceeded,
}

/// The return type of the IDA* recursion function. It maintains the
/// soft-invariant that zero means a solution has been found, hence
/// `AdmissibleGoalHeuristic::SOLVED`.
#[derive(PartialEq, Copy, Clone, Debug)]
struct AdmissibleGoalHeuristic(u8);

impl AdmissibleGoalHeuristic {
    const SOLVED: Self = Self(0);
}

// TODO: is this worth making a const generic?
#[derive(Debug, PartialEq)]
pub enum SearchStrategy {
    FirstSolution,
    AllSolutions,
}

impl<'id, P: PuzzleState<'id>, H: PuzzleStateHistory<'id, P>>
    CycleStructureSolverMutable<'id, P, H>
{
    fn found_solution(&self) -> bool {
        !self.solutions.is_empty()
    }
}

#[derive(Debug)]
pub struct SolutionsIntoIter<'id, 'a, P: PuzzleState<'id>> {
    puzzle_def: &'a PuzzleDef<'id, P>,
    result_1: P,
    result_2: P,
    solutions: IntoIter<Vec<usize>>,
    expanded_count: usize,
    solution_length: usize,
    /// The buffer reused
    expanded_solution: Option<Box<[&'a Move<'id, P>]>>,
    /// The current solution from `solutions` being expanded upon
    currently_expanding_solution: Option<Vec<usize>>,
    /// The state of the canonical sequence expansion
    canonical_sequence_expansion: Option<CanonicalSequenceExpansion>,
    /// A disjoint mapping of multiple canonical sequence expansions. These are
    /// indicies of a permutation.
    canonical_sequence_expansion_transformation: Vec<usize>,
    /// The state of the sequence symmetry expansion
    sequence_symmetry_expansion: Option<SequenceSymmetryExpansion>,
}

#[derive(Debug)]
struct SequenceSymmetryExpansion {
    rotation_index: usize,
    rotation_class_size: usize,
}

#[derive(Debug)]
struct CanonicalSequenceExpansion {
    expansion_intervals: Vec<(usize, usize)>,
}

impl<'id, P: PuzzleState<'id>, T: PruningTables<'id, P>> CycleStructureSolver<'id, P, T> {
    pub fn new(
        puzzle_def: PuzzleDef<'id, P>,
        pruning_tables: T,
        search_strategy: SearchStrategy,
    ) -> Self {
        let canonical_fsm = (&puzzle_def).into();
        Self {
            puzzle_def,
            pruning_tables,
            canonical_fsm,
            max_solution_length: None,
            search_strategy,
        }
    }

    #[must_use]
    pub fn with_max_solution_length(mut self, max_solution_length: usize) -> Self {
        self.max_solution_length = Some(max_solution_length);
        self
    }

    pub fn into_puzzle_def_and_pruning_tables(self) -> (PuzzleDef<'id, P>, T) {
        (self.puzzle_def, self.pruning_tables)
    }

    /// A highly optimized [iterative deepening A*][IDA] search algorithm. We
    /// employ a number of techniques, some specific to a cycle structure solver
    /// only:
    ///
    /// - We reduce the branching factor by using a finite state machine of
    ///   non-commutative moves.
    /// - We embed a "sequence symmetry" optimization into search, which takes
    ///   advantage of the properties of conjugacy classes.
    /// - We disallow the same move class at the beginning and end to optimize
    ///   the last depth in the search.
    /// - We promote the heuristic to one if the pruning value is zero.
    /// - We use pathmax to prune nodes with large child pruning values.
    ///
    /// The return value is an admissible goal heuristic. That is, it is a
    /// lower bound on the number of moves required to find the solution state
    /// at the exact node. When this lower bound is equal to zero, that means
    /// the node is a solution.
    ///
    /// [IDA]: https://en.wikipedia.org/wiki/Iterative_deepening_A*
    fn search_for_solution<H: PuzzleStateHistory<'id, P>>(
        &self,
        mutable: &mut CycleStructureSolverMutable<'id, P, H>,
        current_fsm_state: CanonicalFSMState,
        entry_index: usize,
        mut permitted_cost: u8,
    ) -> AdmissibleGoalHeuristic {
        if log_enabled!(Level::Debug) {
            mutable.traversed += 1;
        }
        // SAFETY: This function calls `pop_stack` for every `push_stack` call.
        // Therefore, the `pop_stack` cannot be called more than `push_stack`.
        let last_puzzle_state = unsafe { mutable.puzzle_state_history.last_state_unchecked() };

        let admissible_prune_cost = self.pruning_tables.admissible_heuristic(last_puzzle_state);
        if admissible_prune_cost > permitted_cost {
            // Note that `admissible_prune_heuristic` is impossible to be zero
            // here, so the enum instantiation is valid
            return AdmissibleGoalHeuristic(admissible_prune_cost);
        }

        // Sequence symmetry optimization, first observed by [Tomas Rokicki][ss],
        // and slightly improved by this implementation. Some solution to CCS
        // A B C D conjugated by A^-1 yields A^-1 (A B C D) A = B C D A, which
        // we observe to be a rotation of the original sequence. One of the
        // properties of conjugate elements is that they cannot be distinguished
        // by using only the group structure, and this includes the cycle
        // structure we are solving for. Recursively applying this conjugation
        // forms an equivalence class based on the rotations of sequences, so we
        // search only a single representative sequence to avoid duplicate work.
        // We choose the representative as the lexicographically minimal
        // sequence because this restriction is easy to embed in IDA* search.
        //
        // [ss]: https://github.com/cubing/twsearch/commit/7b1d62bd9d9d232fb4729c7227d5255deed9673c
        //
        // Let us define:
        //
        // - `X` as the node about to be explored in the recursive step of IDA*
        // - The inequality symbols lexicographically
        // - `HISTORY(i)` as a function that returns the one-indexed move in the
        //   current move history.
        // - The integer `i`, initialized to zero
        //
        // The sequence symmetry optimization recursively goes through the
        // following decision tree to ensure it only ever searches the
        // lexicographically minimal sequence by their rotation:
        //
        // - If i == 0 or X > HISTORY(i), then append X to the move history and
        //   set `i` to one in the next recursion.
        // - If X == HISTORY(i), then append X to the move history and increment
        //   `i` in the next recursion.
        // - If X < HISTORY(i), then prune X. X can be rotated to the front of
        //   the sequence to produce something lexicographically lesser.
        //
        // SAFETY: `entry_index` starts at zero in the initial call, and
        // `B::initialize` guarantees that the first entry is bound. For every
        // recursive call, the puzzle history stack is pushed and `entry_index`
        // can only be incremented by 1 at most. Therefore, `entry_index` is
        // always less than the number of entries in the puzzle state history
        // and always bound
        let mut move_index_prune_lt = unsafe {
            mutable
                .puzzle_state_history
                .move_index_unchecked(entry_index)
        };
        permitted_cost -= 1;
        // TODO: document this when I am less sleepy
        // only works on leaf nodes
        if permitted_cost == 0 {
            if entry_index == 1 {
                // Fast path. Prevents the leaf node from being the root move
                // class.
                move_index_prune_lt += 1;
            } else {
                let mut i = 1;
                // Note that this correctly evaluates 1 <= 0 and skips when the
                // root node is the leaf node (ie entry_index is 0)
                while i + entry_index <= mutable.puzzle_state_history.stack_pointer() {
                    // SAFETY: `i` and `i + entry_index` are both less than or
                    // equal to `stack_pointer`, so they are both bound
                    match unsafe {
                        mutable.puzzle_state_history.move_index_unchecked(i).cmp(
                            &mutable
                                .puzzle_state_history
                                .move_index_unchecked(i + entry_index),
                        )
                    } {
                        Ordering::Less => {
                            move_index_prune_lt += 1;
                            break;
                        }
                        Ordering::Equal => {
                            i += entry_index;
                        }
                        Ordering::Greater => {
                            break;
                        }
                    }
                }
            }
        }

        for (move_index, move_) in self
            .puzzle_def
            .moves
            .iter()
            .enumerate()
            // We are at the "X < HISTORY(i)" case described earlier. Pruning
            // all of the child nodes is as simple as skipping a "HISTORY(i)"
            // number of children from the beginning
            .skip(move_index_prune_lt)
        {
            let move_class_index = move_.class_index();
            // This is only ever true at the root node
            let is_root = entry_index == 0;
            // This branch should have high predictability
            if is_root {
                // Somehow it is faster to have this before the canonical
                // sequence optimization??
                mutable.root_canonical_fsm_reversed_state = unsafe {
                    self.canonical_fsm
                        // At root we are anyways at default so we might as well
                        // hardcode it for optimization
                        .reverse_next_state(CanonicalFSMState::default(), move_class_index)
                };
            } else if permitted_cost == 0
                // We take advantage of the fact that the shortest sequence can
                // never start and end with the moves in the same move class.
                // Otherwise the end could be rotated to the start and combined
                // together, thus contradicting that assumption. We also note
                // that if A and C commute and "A B ... C" is a solution, then
                // by performing a rotation and commuting A and C we show that
                // "A C B ..." is also a solution. We want to throw out the
                // "A B ... C" case to avoid the redundant work. This condition
                // performs both of these optimizations at once.
                //
                // Note that we *must* use this finnicky reverse_state logic.
                // It does NOT suffice to use next_state and is_some because
                // this condition incorrectly returns true in the case of two
                // non-commutative moves.
                && unsafe {
                    self.canonical_fsm
                        .reverse_state(move_class_index, mutable.root_canonical_fsm_reversed_state)
                        .is_none()
                }
            {
                continue;
            }

            // We use a canonical FSM to enforce a total ordering of commutating
            // moves. For example, U D and D U produce equivalent states, so
            // there is no point in searching both
            let next_fsm_state = unsafe {
                self.canonical_fsm
                    .next_state(current_fsm_state, move_class_index)
            };
            if next_fsm_state.is_none() {
                continue;
            }

            // SAFETY:
            // 1) `pop_stack` is called for every `push_stack` call, so
            //    pop_stack cannot be called more than push_stack.
            // 2) `resize_if_needed` is appropriately called in `solve` before
            //    every call to this function.
            // 3) `move_index` is defined to be bound.
            unsafe {
                mutable
                    .puzzle_state_history
                    .push_stack_unchecked(move_index, &self.puzzle_def);
            }

            // We handle when `permitted_cost == 0` (leaf node) inline to save
            // the recursive function call overhead otherwise incurred
            let child_admissible_goal_heuristic = if permitted_cost == 0 {
                // [Chen Shuang][cs] doesn't count the maxdepth nodes as
                // visited, so we won't either
                //
                // [cs]: https://github.com/cs0x7f/cube_solver_test/blob/bd78072c49f8a01aee16d592400cbd6e2bacca93/vcube.patch#L22

                // SAFETY: we just pushed something onto the stack
                let last_puzzle_state =
                    unsafe { mutable.puzzle_state_history.last_state_unchecked() };
                if last_puzzle_state.induces_sorted_cycle_structure(
                    self.pruning_tables.sorted_cycle_structure_ref(),
                    self.puzzle_def.sorted_orbit_defs_ref(),
                    mutable.aux_mem.as_ref_mut(),
                ) {
                    mutable
                        .solutions
                        .push(mutable.puzzle_state_history.create_move_history());
                    AdmissibleGoalHeuristic::SOLVED
                } else {
                    // If this node resulted in no solution, then we are at
                    // least one step away from a solution
                    AdmissibleGoalHeuristic(1)
                }
            } else {
                // This optimizes to branchless code
                let next_entry_index = if move_index == move_index_prune_lt {
                    // We are at the "X == HISTORY(i)" case described earlier.
                    // We increment the move history index in the next
                    // recursion.
                    entry_index + 1
                } else {
                    // We are at the "X > HISTORY(i)" case described earlier.
                    // We set the move history index to one in the next
                    // recursion.
                    1
                };
                self.search_for_solution(mutable, next_fsm_state, next_entry_index, permitted_cost)
            };

            // If we've found a solution, and our search strategy is to
            // find the first solution, we instantly terminate. No more
            // processing will occur once this returns
            if self.search_strategy == SearchStrategy::FirstSolution
                && child_admissible_goal_heuristic == AdmissibleGoalHeuristic::SOLVED
            {
                // We don't care about preserving `mutable.puzzle_state_history`
                // anymore because there is no further processing
                return AdmissibleGoalHeuristic::SOLVED;
            }

            mutable.puzzle_state_history.pop_stack();

            // Pathmax optimization. If the child node has a large pruning
            // value, then we can set the current node cost to that value minus
            // one and re-prune. This larger value is still admissible because
            // it is one less then a known lower bound.
            //
            // Note that this is only effective when the heuristics are
            // **inconsistent**, or when the pruning table entry is the minimum
            // of two or more other values. With exact tables, this if statement
            // will never run, which should be good for the branch predictor.
            if
            // Assume the current node to be the child node heuristic minus one,
            // and the permitted cost plus one because we subtracted one from it
            // before entering this loop.
            //
            // IMPORTANT: We carry over the minus one to the right side to
            // create plus two. It must be written with a plus two to prevent
            // overflow.
            child_admissible_goal_heuristic.0 > permitted_cost + 2 {
                // The child node heuristic minus one cannot be zero and break
                // the zero invariant because 1 > X + 2 cannot be true for any
                // u8 (we don't care about 254). It cannot be zero either and
                // overflow for the same reason.
                return AdmissibleGoalHeuristic(child_admissible_goal_heuristic.0 - 1);
            }
            // If we never returned, we might still be able to propagate the
            // pathmaxed value.
            // TODO: test when approximate tables work
            // if child_admissible_goal_heuristic.0 > admissible_prune_cost + 1 {
            //     admissible_prune_cost = child_admissible_goal_heuristic.0 - 1;
            // }
        }
        AdmissibleGoalHeuristic(
            // This optimizes to branchless code
            if admissible_prune_cost == 0 {
                // If this node resulted in no solution, then we are at least
                // one step away from a solution.
                1
            } else {
                admissible_prune_cost
            },
        )
    }

    fn prepare_next_depth<H: PuzzleStateHistory<'id, P>>(
        &self,
        mutable: &mut CycleStructureSolverMutable<'id, P, H>,
        next_depth: u8,
    ) -> Result<(), CycleStructureSolverError> {
        // During pathmax we increment the depth by one, so we ensure it
        // cannot overflow
        if next_depth == u8::MAX {
            return Err(CycleStructureSolverError::SolutionDoesNotExist);
        }
        if let Some(max_solution_length) = self.max_solution_length
            && usize::from(next_depth) > max_solution_length
        {
            return Err(CycleStructureSolverError::MaxSolutionLengthExceeded);
        }
        mutable.traversed = 0;
        mutable.tmp = 0;
        mutable
            .puzzle_state_history
            .resize_if_needed(usize::from(next_depth));
        Ok(())
    }

    /// Run Qter's cycle combination solver.
    ///
    /// # Errors
    ///
    /// The solver will fail if it cannot find a solution. See
    /// `CycleStructureSolverError`.
    pub fn solve<H: PuzzleStateHistory<'id, P>>(
        &self,
    ) -> Result<SolutionsIntoIter<'id, '_, P>, CycleStructureSolverError> {
        info!(start!(
            "Beginning Cycle Combination Solver solution search..."
        ));
        let start = Instant::now();

        let mut mutable: CycleStructureSolverMutable<P, H> = CycleStructureSolverMutable {
            puzzle_state_history: (&self.puzzle_def).into(),
            aux_mem: P::new_aux_mem(self.puzzle_def.sorted_orbit_defs_ref()),
            solutions: vec![],
            root_canonical_fsm_reversed_state: 0,
            traversed: 0,
            tmp: 0,
        };
        // SAFETY: `H::initialize` when puzzle_state_history is created
        // guarantees that the first entry is bound
        let last_puzzle_state = unsafe { mutable.puzzle_state_history.last_state_unchecked() };
        let mut depth = self.pruning_tables.admissible_heuristic(last_puzzle_state);
        // Manually check depth 0 because the `permitted_cost == 0` check was
        // moved inside of the main loop in `search_for_solution`.
        if depth == 0 {
            debug!(working!("Searching depth limit {}..."), depth);
            let depth_start = Instant::now();
            // The return values here don't matter since it's not used in the
            // below loop so we can get rid of `true` and `false`
            if last_puzzle_state.induces_sorted_cycle_structure(
                self.pruning_tables.sorted_cycle_structure_ref(),
                self.puzzle_def.sorted_orbit_defs_ref(),
                mutable.aux_mem.as_ref_mut(),
            ) {
                mutable
                    .solutions
                    .push(mutable.puzzle_state_history.create_move_history());
            } else {
                depth = 1;
                self.prepare_next_depth(&mut mutable, depth)?;
            }
            debug!(
                working!("Traversed {} nodes in {}"),
                mutable.traversed,
                depth_start.elapsed().human(Truncate::Millis)
            );
        }

        if !mutable.found_solution() {
            self.prepare_next_depth(&mut mutable, depth)?;
            loop {
                debug!(working!("Searching depth limit {}..."), depth);
                let depth_start = Instant::now();
                // `entry_index` must be zero here so the root level so sequence
                // symmetry doesn't access OOB move history entries.
                self.search_for_solution(
                    &mut mutable,
                    CanonicalFSMState::default(),
                    // Remember that `i` must be initialized to zero for the
                    // sequence symmetry optimization to work.
                    0,
                    depth,
                );
                debug!(
                    working!("Traversed {} nodes in {} (tmp: {})"),
                    mutable.traversed,
                    depth_start.elapsed().human(Truncate::Millis),
                    mutable.tmp,
                );
                if mutable.found_solution() {
                    break;
                }
                depth += 1;
                self.prepare_next_depth(&mut mutable, depth)?;
            }
        }

        info!(
            success!("Found {} raw solutions at depth {} in {}"),
            mutable.solutions.len(),
            depth,
            start.elapsed().human(Truncate::Millis)
        );
        debug!("");
        let result_1 = self.puzzle_def.new_solved_state();
        let result_2 = result_1.clone();
        Ok(SolutionsIntoIter {
            puzzle_def: &self.puzzle_def,
            result_1,
            result_2,
            solutions: mutable.solutions.into_iter(),
            expanded_count: 0,
            solution_length: depth.into(),
            expanded_solution: None,
            currently_expanding_solution: None,
            canonical_sequence_expansion: None,
            canonical_sequence_expansion_transformation: (0..depth.into()).collect_vec(),
            sequence_symmetry_expansion: None,
        })
    }
}

impl<'id, P: PuzzleState<'id>> Iterator for SolutionsIntoIter<'id, '_, P> {
    type Item = ();

    // Note that we cannot use a method that mutates self and also returns
    // an Option<&[&Move<'id, P>]>. This would create a mutable borrow that
    // would last while this slice is in use (which is likely in the entire
    // scope) preventing immutable methods from being called on self.
    // See: https://doc.rust-lang.org/nomicon/lifetime-mismatch.html
    fn next(&mut self) -> Option<()> {
        // The current expansion order:
        //
        // - Canonical sequences
        // - Sequence symmetry

        // We only load a new solution when we have exhausted all expansions
        // of the current solution
        if self.currently_expanding_solution.is_none() {
            self.currently_expanding_solution = self.solutions.next();
        }
        // If there are no more solutions, we are done
        let currently_expanding_solution = self.currently_expanding_solution.as_deref()?;

        // Initialize the canonical sequence expansion if it hasn't been
        if self.canonical_sequence_expansion.is_none() {
            let mut maybe_acc: Option<Cow<P>> = None;
            let mut expansion_length = 1;
            // Vector of (expansion_position, expansion_length)
            let mut expansion_intervals = vec![];
            // Identify all intervals of moves that commute with each other
            for (expansion_position, &move_index) in currently_expanding_solution.iter().enumerate()
            {
                let commutes = if let Some(acc) = &maybe_acc {
                    self.result_1.replace_compose(
                        acc,
                        self.puzzle_def.moves[move_index].puzzle_state(),
                        self.puzzle_def.sorted_orbit_defs_ref(),
                    );
                    self.result_2.replace_compose(
                        self.puzzle_def.moves[move_index].puzzle_state(),
                        acc,
                        self.puzzle_def.sorted_orbit_defs_ref(),
                    );
                    self.result_1 == self.result_2
                } else {
                    false
                };
                if commutes {
                    expansion_length += 1;
                    maybe_acc = Some(Cow::Owned(self.result_1.clone()));
                } else {
                    if expansion_length != 1 {
                        expansion_intervals
                            .push((expansion_position - expansion_length, expansion_length));
                        expansion_length = 1;
                    }
                    maybe_acc = Some(Cow::Borrowed(
                        self.puzzle_def.moves[move_index].puzzle_state(),
                    ));
                }
            }
            // Handle the last expansion interval
            if expansion_length != 1 {
                expansion_intervals.push((
                    currently_expanding_solution.len() - expansion_length,
                    expansion_length,
                ));
            }
            // If there is nothing to expand, we skip setting up the expansion
            if !expansion_intervals.is_empty() {
                self.canonical_sequence_expansion = Some(CanonicalSequenceExpansion {
                    expansion_intervals,
                });
            }
        }

        // Helper to apply the canonical sequence expansion transformation
        let canonical_sequence_expansion_transformation = |i: usize| {
            if self.canonical_sequence_expansion.is_none() {
                i
            } else {
                self.canonical_sequence_expansion_transformation[i]
            }
        };

        // Initialize the sequence symmetry expansion if it hasn't already been
        // done. We use get_or_insert_with unlike the first time because we
        // always have a sequence symmetry expansion.
        let sequence_symmetry_expansion =
            self.sequence_symmetry_expansion.get_or_insert_with(|| {
                // Properly initializing the sequence symmetry expansion
                // requires us to notice a subtle observation. The sequence
                // A B A B should only be rotated a single time, not four times.
                // We need to find the size of the "rotation class" of the
                // sequence.
                //
                // We use the technique described here:
                // https://leetcode.com/problems/rotate-string/solutions/5988868/rotate-string-by-leetcode-w5ch
                let mut rotation_class_size = 1;
                loop {
                    // If we have not yet exceeded the solution length,
                    if rotation_class_size >= self.solution_length {
                        break;
                    }
                    // And the concatenated string has not yet found a suitable
                    // rotation class size,
                    if (0..self.solution_length).all(|i| {
                        let transformed_sequence_symmetry_expansion_index =
                            (rotation_class_size + i) % self.solution_length;
                        currently_expanding_solution[canonical_sequence_expansion_transformation(i)]
                            == currently_expanding_solution
                                [canonical_sequence_expansion_transformation(
                                    transformed_sequence_symmetry_expansion_index,
                                )]
                    }) {
                        break;
                    }
                    // then we increment the rotation class size and try again
                    rotation_class_size += 1;
                }
                SequenceSymmetryExpansion {
                    rotation_index: 0,
                    rotation_class_size,
                }
            });

        // Helper to apply the sequence symmetry transformation
        let sequence_symmetry_expansion_transformation =
            |i: usize| (i + sequence_symmetry_expansion.rotation_index) % self.solution_length;

        // Once we've initialized all the expansions, we can now produce the
        // expanded solution

        // Combined helper to apply all transformations
        let transform_index = |i: usize| {
            let mut transformed_index = sequence_symmetry_expansion_transformation(i);
            transformed_index = canonical_sequence_expansion_transformation(transformed_index);
            transformed_index
        };

        // We reuse the buffer if it exists
        if let Some(expanded_solution) = self.expanded_solution.as_deref_mut() {
            for (i, es) in expanded_solution.iter_mut().enumerate() {
                *es = &self.puzzle_def.moves[currently_expanding_solution[transform_index(i)]];
            }
        } else {
            self.expanded_solution = Some(
                (0..self.solution_length)
                    .map(|i| {
                        &self.puzzle_def.moves[currently_expanding_solution[transform_index(i)]]
                    })
                    .collect(),
            );
        }

        // We are done processing this sequence symmetry rotation. Increase the
        // rotation index.
        sequence_symmetry_expansion.rotation_index += 1;
        // Sanity check: it's nonsensical for the rotation index to exceed
        // the rotation class size. ie for A B C A B C the valid rotation
        // indicies are 0, 1, and 2, and the class size is 3.
        assert!(
            sequence_symmetry_expansion.rotation_index
                <= sequence_symmetry_expansion.rotation_class_size,
        );
        // If we have reached the end of the sequence symmetry expansion,
        if sequence_symmetry_expansion.rotation_index
            == sequence_symmetry_expansion.rotation_class_size
        {
            // set it to None
            self.sequence_symmetry_expansion = None;

            // and check if the currently expanding solution has a canonical
            // sequence expansion. If it does,
            if let Some(cse) = &mut self.canonical_sequence_expansion {
                // Iterate through every expansion interval
                let mut i = cse.expansion_intervals.len();
                while i != 0 {
                    let (expansion_position, expansion_length) = cse.expansion_intervals[i - 1];
                    // And run a permutator algorithm on the expansion interval
                    // to get the next canonical sequence expansion. If there
                    // exists a next permutation,
                    if pandita1(
                        &mut self.canonical_sequence_expansion_transformation
                            [expansion_position..expansion_position + expansion_length],
                    ) {
                        // Then break
                        break;
                    }
                    // Else, reset the expansion slice
                    for j in expansion_position..expansion_position + expansion_length {
                        self.canonical_sequence_expansion_transformation[j] = j;
                    }
                    i -= 1;
                }
                // If we have iterated through all expansions and they all had
                // no next permutation, we have nothing left to expand so we set
                // both the current expanding solution and the canonical
                // sequence expansion to None.
                if i == 0 {
                    self.canonical_sequence_expansion = None;
                    self.currently_expanding_solution = None;
                }
            } else {
                // If it doesn't, we have nothing left to expand so we set the
                // currently expanding solution to None
                self.currently_expanding_solution = None;
            }
        }

        self.expanded_count += 1;
        trace!(
            "{:<4}",
            self.expanded_solution
                .as_ref()
                .unwrap()
                .iter()
                .map(|move_| move_.name())
                .format(" ")
        );
        Some(())
    }
}

impl<'id, 'a, P: PuzzleState<'id>> SolutionsIntoIter<'id, 'a, P> {
    /// Returns the expanded solution.
    ///
    /// # Panics
    ///
    /// Panics if this is called before `.next()`
    #[must_use]
    pub fn expanded_solution(&self) -> &[&'a Move<'id, P>] {
        self.expanded_solution.as_ref().unwrap()
    }

    #[must_use]
    pub fn puzzle_def(&self) -> &PuzzleDef<'id, P> {
        self.puzzle_def
    }

    #[must_use]
    pub fn solution_length(&self) -> usize {
        self.solution_length
    }

    #[must_use]
    pub fn expanded_count(&self) -> usize {
        self.expanded_count
    }
}

fn pandita1(perm: &mut [usize]) -> bool {
    let len = perm.len();
    assert!(len > 0);

    let mut i = len - 1;
    while perm[i] < perm[i - 1] {
        i -= 1;
        if i == 0 {
            return false;
        }
    }

    for j in (i..len).rev() {
        if perm[j] > perm[i - 1] {
            perm.swap(i - 1, j);
            break;
        }
    }

    perm[i..].reverse();
    true
}

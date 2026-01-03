//! Canonical finite state machine implementation, derived primarily from Lucas
//! Garron's implementation in twsearch with permission:
//! <https://github.com/cubing/twsearch/blob/main/src/rs/_internal/canonical_fsm/canonical_fsm.rs>

use super::puzzle::{PuzzleDef, PuzzleState};
use generativity::Id;
use std::{collections::HashMap, marker::PhantomData, num::NonZeroUsize};

// Bit N is indexed by a `MoveClassIndex` value of N.
type MoveClassMask = Vec<bool>;

pub type CanonicalFSMState = Option<NonZeroUsize>;

struct MaskToState(HashMap<MoveClassMask, usize>);

struct StateToMask(Vec<MoveClassMask>);

#[derive(Debug)]
pub struct PuzzleCanonicalFSM<'id, P: PuzzleState<'id>> {
    next_state_lookup: Vec<CanonicalFSMState>,
    num_move_classes: usize,
    _id: Id<'id>,
    _marker: PhantomData<P>,
}

impl<'id, P: PuzzleState<'id>> From<&PuzzleDef<'id, P>> for PuzzleCanonicalFSM<'id, P> {
    fn from(puzzle_def: &PuzzleDef<'id, P>) -> Self {
        let num_move_classes = puzzle_def.move_classes.len();
        let mut commutes: Vec<MoveClassMask> = vec![vec![true; num_move_classes]; num_move_classes];

        // Written this way so if we later iterate over all moves instead of
        // all move classes. This is because multiples can commute differently
        // than their quantum values. For example:
        //
        // - The standard T-Perm (`R U R' U' R' F R2 U' R' U' R U R' F'`) has
        //   order 2.
        // - `R2 U2` has order 6.
        // - T-perm and `(R2 U2)3` commute.
        let mut result_1 = puzzle_def.new_solved_state();
        let mut result_2 = result_1.clone();
        for (i, move_class_1) in puzzle_def.move_classes.iter().copied().enumerate() {
            for (j, move_class_2) in puzzle_def.move_classes.iter().copied().enumerate().take(i) {
                if !puzzle_def.moves[move_class_1].commutes_with(
                    &puzzle_def.moves[move_class_2],
                    &mut result_1,
                    &mut result_2,
                    puzzle_def.sorted_orbit_defs_ref(),
                ) {
                    commutes[i][j] = false;
                    commutes[j][i] = false;
                }
            }
        }

        let mut next_state_lookup: Vec<CanonicalFSMState> = vec![];

        let mut mask_to_state = MaskToState(HashMap::new());
        mask_to_state.0.insert(vec![false; num_move_classes], 0);
        // state_to_mask, indexed by state ordinal, holds the set of move
        // classes in the move sequence so far for which all subsequent moves
        // commute
        let mut state_to_mask = StateToMask(vec![vec![false; num_move_classes]]);

        // start state
        let mut queue_index = 0;
        while queue_index < state_to_mask.0.len() {
            // illegal state
            let mut next_state = vec![None; num_move_classes];
            let dequeue_move_class_mask = state_to_mask.0[queue_index].clone();

            queue_index += 1;

            for move_class_index in 0..puzzle_def.move_classes.len() {
                let mut skip = false;
                // If there's a greater move (multiple) in the state that
                // commutes with this move's `move_class`, we can't move
                // `move_class`.
                skip |= dequeue_move_class_mask
                    .iter()
                    .copied()
                    .zip(commutes[move_class_index].iter().copied())
                    .skip(move_class_index + 1)
                    .any(|(dequeue_move_class, commute)| dequeue_move_class && commute);
                skip |= dequeue_move_class_mask[move_class_index];
                if skip {
                    continue;
                }

                let mut next_state_mask = dequeue_move_class_mask.clone();
                for (next_state, &commute) in
                    next_state_mask.iter_mut().zip(&commutes[move_class_index])
                {
                    *next_state &= commute;
                }
                next_state_mask[move_class_index] = true;

                // If a pair of bits are set with the same commutating moves, we
                // can clear out the higher ones. This optimization keeps the
                // state count from going exponential for very big cubes.
                //
                // Originally implemented by Tomas Rokicki: https://github.com/cubing/twsearch/blob/0dced6e55f5612609a54c75056d00535fadee0c8/src/cpp/canon.cpp#L106
                for i in 0..num_move_classes {
                    if next_state_mask[i] {
                        for j in (i + 1)..num_move_classes {
                            if next_state_mask[j] && commutes[i] == commutes[j] {
                                next_state_mask[i] = false;
                            }
                        }
                    }
                }

                next_state[move_class_index] =
                    if let Some(&state) = mask_to_state.0.get(&next_state_mask) {
                        CanonicalFSMState::Some(state.try_into().unwrap())
                    } else {
                        let next_state = state_to_mask.0.len();
                        mask_to_state.0.insert(next_state_mask.clone(), next_state);
                        state_to_mask.0.push(next_state_mask);
                        CanonicalFSMState::Some(next_state.try_into().unwrap())
                    };
            }
            next_state_lookup.extend(next_state);
        }

        Self {
            next_state_lookup,
            num_move_classes,
            _id: puzzle_def.id(),
            _marker: PhantomData,
        }
    }
}

impl<'id, P: PuzzleState<'id>> PuzzleCanonicalFSM<'id, P> {
    fn next_state_lookup(&self) -> &[CanonicalFSMState] {
        &self.next_state_lookup
    }

    /// The next state of the FSM given the current state and a move class.
    ///
    /// None passed in means we're in the initial state
    /// None returned means the move is illegal
    pub unsafe fn next_state(
        &self,
        current_fsm_state: CanonicalFSMState,
        move_class_index: usize,
    ) -> CanonicalFSMState {
        let i = match current_fsm_state {
            Some(state) => state.get(),
            None => 0,
        };
        unsafe {
            *self
                .next_state_lookup()
                .get_unchecked(i * self.num_move_classes + move_class_index)
        }
    }

    pub unsafe fn reverse_state(
        &self,
        move_class_index: usize,
        reversed_state: usize,
    ) -> CanonicalFSMState {
        let reversed_move_class_index =
            Some(unsafe { NonZeroUsize::new_unchecked(move_class_index + 1) });
        unsafe { self.next_state(reversed_move_class_index, reversed_state) }
    }

    pub unsafe fn reverse_next_state(
        &self,
        current_fsm_state: CanonicalFSMState,
        move_class_index: usize,
    ) -> usize {
        unsafe {
            self.next_state(current_fsm_state, move_class_index)
                .unwrap()
                .get()
                - 1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::puzzle::{PuzzleDef, cube3::Cube3, slice_puzzle::HeapPuzzle};
    use generativity::make_guard;
    use puzzle_theory::puzzle_geometry::parsing::puzzle;

    #[test]
    fn test_canonical_fsm_puzzle_initially_all_legal() {
        make_guard!(guard);
        let cube3_def = PuzzleDef::<Cube3>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let canonical_fsm: PuzzleCanonicalFSM<Cube3> = (&cube3_def).into();

        for move_class_index in 0..cube3_def.move_classes.len() {
            assert!(unsafe {
                canonical_fsm
                    .next_state(CanonicalFSMState::default(), move_class_index)
                    .is_some()
            });
        }
    }

    #[test]
    fn test_canonical_fsm_puzzle_prevents_self() {
        make_guard!(guard);
        let cube3_def = PuzzleDef::<Cube3>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let canonical_fsm: PuzzleCanonicalFSM<Cube3> = (&cube3_def).into();
        for move_class_index in 0..cube3_def.move_classes.len() {
            assert!(unsafe {
                canonical_fsm
                    .next_state(
                        Some(
                            canonical_fsm
                                .next_state(CanonicalFSMState::default(), move_class_index)
                                .unwrap(),
                        ),
                        move_class_index,
                    )
                    .is_none()
            });
        }
    }

    #[test]
    fn test_canonical_fsm_puzzle_prevents_self_and_antipode() {
        make_guard!(guard);
        let cube3_def = PuzzleDef::<Cube3>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let canonical_fsm: PuzzleCanonicalFSM<Cube3> = (&cube3_def).into();

        let mut result_1 = cube3_def.new_solved_state();
        let mut result_2 = result_1.clone();
        for (move_class_index_1, move_class_1) in cube3_def.move_classes.iter().copied().enumerate()
        {
            let move_1 = &cube3_def.moves[move_class_1];
            for (move_class_index_2, move_class_2) in
                cube3_def.move_classes.iter().copied().enumerate()
            {
                let move_2 = &cube3_def.moves[move_class_2];
                if !move_1.commutes_with(
                    move_2,
                    &mut result_1,
                    &mut result_2,
                    cube3_def.sorted_orbit_defs_ref(),
                ) {
                    continue;
                }

                let allows_1_after_2 = unsafe {
                    canonical_fsm
                        .next_state(
                            Some(
                                canonical_fsm
                                    .next_state(CanonicalFSMState::default(), move_class_index_2)
                                    .unwrap(),
                            ),
                            move_class_index_1,
                        )
                        .is_some()
                };
                let allows_2_after_1 = unsafe {
                    canonical_fsm
                        .next_state(
                            Some(
                                canonical_fsm
                                    .next_state(CanonicalFSMState::default(), move_class_index_1)
                                    .unwrap(),
                            ),
                            move_class_index_2,
                        )
                        .is_some()
                };

                if move_class_index_1 == move_class_index_2 {
                    // No matter what the same face should not be allowed after
                    // another.
                    assert!(!allows_2_after_1 && !allows_1_after_2);
                } else {
                    // We expect a total ordering of commutative move classes.
                    // Therefore one should be allowed after the other but not
                    // the other way around. Xor gives me that truth table.
                    assert!(allows_1_after_2 ^ allows_2_after_1);
                }
            }
        }
    }

    #[test]
    #[ignore = "big cube stuff isnt working without puzzle working"]
    fn test_big_cube_prevents_move_class() {
        make_guard!(guard);
        let cube4_def = PuzzleDef::<HeapPuzzle>::new(&puzzle("4x4").ksolve(), guard).unwrap();
        let canonical_fsm: PuzzleCanonicalFSM<HeapPuzzle> = (&cube4_def).into();

        let mut result_1 = cube4_def.new_solved_state();
        let mut result_2 = result_1.clone();
        let mut commutes = vec![];
        for &move_class in &cube4_def.move_classes {
            let mut commute = vec![];
            for (other_move_class_index, &other_move_class) in
                cube4_def.move_classes.iter().enumerate()
            {
                if cube4_def.moves[move_class].commutes_with(
                    &cube4_def.moves[other_move_class],
                    &mut result_1,
                    &mut result_2,
                    cube4_def.sorted_orbit_defs_ref(),
                ) {
                    commute.push(other_move_class_index);
                }
            }
            commutes.push(commute);
        }
        commutes.dedup();

        // there are three axes of a cube whose move classes can commute
        assert_eq!(commutes.len(), 3);
        // for each axis, there are four commuting move classes for each slice
        // plus two more for wide moves
        assert!(commutes.iter().all(|commute| commute.len() == 6));

        for commute in commutes {
            for &move_class_index in &commute {
                for &other_move_class_index in &commute {
                    let current_then_other = unsafe {
                        canonical_fsm.next_state(
                            Some(
                                canonical_fsm
                                    .next_state(CanonicalFSMState::default(), move_class_index)
                                    .unwrap(),
                            ),
                            other_move_class_index,
                        )
                    };
                    if other_move_class_index <= move_class_index {
                        // a lesser multiple of the move class, not allowed to
                        // move the move class
                        assert!(current_then_other.is_none());
                    } else {
                        // a greater multiple of the move class, allowed to move
                        // the move class
                        assert!(current_then_other.is_some());
                    }
                }
            }
        }
    }
}

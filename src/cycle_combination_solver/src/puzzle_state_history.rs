use super::puzzle::{PuzzleDef, PuzzleState};
use std::{marker::PhantomData, ops::Index, slice::SliceIndex};

pub trait PuzzleStateHistory<'id, P: PuzzleState<'id>> {
    const UPPER_GODS_NUMBER_BOUND: Option<usize>;
    type Buf: Index<usize, Output = (P, usize)> + AsMut<[(P, usize)]> + AsRef<[(P, usize)]>;

    /// Create an initial `Self::Buf`. It must initialize the stack with a first
    /// entry of the solved state and a move index of 0.
    fn initialize(puzzle_def: &PuzzleDef<'id, P>) -> Self::Buf;

    /// Resize the underlying buffer capacity if needed.
    fn resize_if_needed(buf: &mut Self::Buf, max_stack_pointer: usize);

    /// Push a new state onto the stack by composing the given move with the
    /// last state in the stack, denoted by `stack_pointer`.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that:
    /// 1) `stack_pointer` is in bounds of `Self::Buf`.
    /// 2) `move_index` is in bounds of `puzzle_def.moves`.
    unsafe fn push_stack_unchecked(
        buf: &mut [(P, usize)],
        stack_pointer: usize,
        move_index: usize,
        puzzle_def: &PuzzleDef<'id, P>,
    ) {
        debug_assert!(stack_pointer < buf.len());
        // SAFETY: move_index is guaranteed to be in bounds by the caller
        let puzzle_state = unsafe { puzzle_def.moves.get_unchecked(move_index).puzzle_state() };
        let (left, right) = buf.split_at_mut(stack_pointer + 1);
        // SAFETY: `resize_if_needed` is guaranteed to have been correctly
        // called beforehand, so `stack_pointer` must be less than `self.len()`
        // and in bounds of the stack. Therefore, `right` is non-empty.
        let next_entry = unsafe { right.get_unchecked_mut(0) };
        // SAFETY: We split_at_mut at stack_pointer + 1 which is nonzero, so
        // stack_pointer is a valid index.
        let last_entry_puzzle_state = unsafe { &left.get_unchecked(stack_pointer).0 };
        next_entry.0.replace_compose(
            last_entry_puzzle_state,
            puzzle_state,
            puzzle_def.sorted_orbit_defs_ref(),
        );
        next_entry.1 = move_index;
    }
}

pub struct StackedPuzzleStateHistory<'id, P: PuzzleState<'id>, H: PuzzleStateHistory<'id, P>> {
    stack: H::Buf,
    stack_pointer: usize,
    _marker: PhantomData<P>,
}

impl<'id, P: PuzzleState<'id>, H: PuzzleStateHistory<'id, P>> From<&PuzzleDef<'id, P>>
    for StackedPuzzleStateHistory<'id, P, H>
{
    fn from(puzzle_def: &PuzzleDef<'id, P>) -> Self {
        Self {
            stack: H::initialize(puzzle_def),
            stack_pointer: 0,
            _marker: PhantomData,
        }
    }
}

impl<'id, P: PuzzleState<'id>, H: PuzzleStateHistory<'id, P>> StackedPuzzleStateHistory<'id, P, H> {
    /// Push a new state onto the stack by composing the given move with the
    /// last state in the stack.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that:
    /// 1) `pop_stack` is not called more times than `push_stack_unchecked`.
    /// 2) `resize_if_needed` is called before exploring a new maximum stack
    ///    depth.
    /// 3) `move_index` is in bounds of `puzzle_def.moves`.
    pub unsafe fn push_stack_unchecked(
        &mut self,
        move_index: usize,
        puzzle_def: &PuzzleDef<'id, P>,
    ) {
        // SAFETY: 1) guarantees that stack_pointer is never negative and 2)
        // guarantees that stack_pointer is never too high. Therefore,
        // `stack_pointer` is always in bounds. `move_index` is likewise in
        // bounds by 3).
        unsafe {
            H::push_stack_unchecked(
                self.stack.as_mut(),
                self.stack_pointer,
                move_index,
                puzzle_def,
            );
        }
        self.stack_pointer += 1;
    }

    /// Pop the last state from the stack.
    pub fn pop_stack(&mut self) {
        debug_assert!(self.stack_pointer > 0);
        self.stack_pointer -= 1;
    }

    /// Resize the underlying buffer capacity if needed.
    pub fn resize_if_needed(&mut self, max_stack_pointer: usize) {
        H::resize_if_needed(&mut self.stack, max_stack_pointer);
    }

    /// Get the last state in the stack.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that `pop_stack` was not called more times
    /// than `push_stack_unchecked`.
    ///
    /// There is no need to guarantee that `push_stack` was not called too many
    /// times because `push_stack_unchecked` has its own safety invariant check.
    pub unsafe fn last_state_unchecked(&self) -> &P {
        // SAFETY: stack_pointer is guaranteed to be in bounds by the caller
        unsafe { &(*self.stack_pointer.get_unchecked(self.stack.as_ref())).0 }
    }

    /// Get the move index of an entry at the given index.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that `entry_index` is in bounds of the stack.
    pub unsafe fn move_index_unchecked(&self, entry_index: usize) -> usize {
        debug_assert!(entry_index <= self.stack_pointer);
        // SAFETY: entry_index is guaranteed to be in bounds by the caller
        unsafe { (*entry_index.get_unchecked(self.stack.as_ref())).1 }
    }

    /// Create a new move history from the current state of the stack.
    pub fn create_move_history(&self) -> Vec<usize> {
        (1..=self.stack_pointer).map(|i| self.stack[i].1).collect()
    }

    pub fn stack_pointer(&self) -> usize {
        self.stack_pointer
    }
}

impl<'id, P: PuzzleState<'id>> PuzzleStateHistory<'id, P> for Vec<P> {
    const UPPER_GODS_NUMBER_BOUND: Option<usize> = None;
    type Buf = Vec<(P, usize)>;

    fn initialize(puzzle_def: &PuzzleDef<'id, P>) -> Vec<(P, usize)> {
        vec![(puzzle_def.new_solved_state(), 0)]
    }

    fn resize_if_needed(buf: &mut Self::Buf, max_stack_pointer: usize) {
        buf.resize(max_stack_pointer + 1, (buf[0].0.clone(), usize::MAX));
    }
}

/// # Safety
///
/// A marker trait that guarantees that the array buffer used for storing the
/// puzzle state history is always in bounds. That is, the stack pointer is
/// always less than the length of the buffer.
pub unsafe trait PuzzleStateHistoryArrayBuf<'id, P: PuzzleState<'id>> {}

impl<'id, const N: usize, P: PuzzleState<'id>> PuzzleStateHistory<'id, P> for [P; N]
where
    [P; N]: PuzzleStateHistoryArrayBuf<'id, P>,
{
    const UPPER_GODS_NUMBER_BOUND: Option<usize> = Some(N - 1);
    type Buf = [(P, usize); N];

    fn initialize(puzzle_def: &PuzzleDef<'id, P>) -> [(P, usize); N] {
        let mut ret = core::array::from_fn(|_| (puzzle_def.new_solved_state(), usize::MAX));
        // it is important this is zero so `start` is zero when empty
        ret[0].1 = 0;
        ret
    }

    fn resize_if_needed(_buf: &mut Self::Buf, _max_stack_pointer: usize) {
        // The stack pointer is always less than the length of the buffer
        // because of the trait bound. Therefore there is no need to resize the
        // buffer.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::puzzle::{Move, cube3::Cube3};
    use generativity::{Guard, make_guard};
    use puzzle_theory::puzzle_geometry::parsing::puzzle;

    fn move_index<'id, P: PuzzleState<'id>>(
        puzzle_def: &PuzzleDef<'id, P>,
        move_: &Move<'id, P>,
    ) -> usize {
        puzzle_def
            .moves
            .iter()
            .position(|move_iter| move_iter.puzzle_state() == move_.puzzle_state())
            .unwrap()
    }

    fn initialize<'id, H: PuzzleStateHistory<'id, Cube3>>(guard: Guard<'id>) {
        let cube3_def = PuzzleDef::<Cube3>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let puzzle_state_history: StackedPuzzleStateHistory<Cube3, H> = (&cube3_def).into();

        assert_eq!(puzzle_state_history.stack_pointer, 0);
        assert_eq!(
            &puzzle_state_history.stack[0].0,
            &cube3_def.new_solved_state()
        );
        assert_eq!(puzzle_state_history.stack[0].1, 0);
    }

    #[test]
    fn test_initialize() {
        make_guard!(guard);
        initialize::<Vec<Cube3>>(guard);
        make_guard!(guard);
        initialize::<[Cube3; 21]>(guard);
    }

    fn puzzle_state_history_composition<'id, H: PuzzleStateHistory<'id, Cube3>>(guard: Guard<'id>) {
        let cube3_def = PuzzleDef::<Cube3>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let r_move = cube3_def.find_move("R").unwrap();
        let r_move_index = move_index(&cube3_def, r_move);

        let mut puzzle_state_history: StackedPuzzleStateHistory<Cube3, H> = (&cube3_def).into();
        puzzle_state_history.resize_if_needed(2);

        unsafe {
            puzzle_state_history.push_stack_unchecked(r_move_index, &cube3_def);
            puzzle_state_history.push_stack_unchecked(r_move_index, &cube3_def);
        }

        assert_eq!(puzzle_state_history.stack_pointer, 2);

        let r2_move = cube3_def.find_move("R2").unwrap();
        assert_eq!(&puzzle_state_history.stack[2].0, r2_move.puzzle_state());
        assert_eq!(puzzle_state_history.stack[1].1, r_move_index);
        assert_eq!(puzzle_state_history.stack[2].1, r_move_index);
    }

    #[test]
    fn test_puzzle_state_history_composition() {
        make_guard!(guard);
        puzzle_state_history_composition::<Vec<Cube3>>(guard);
        make_guard!(guard);
        puzzle_state_history_composition::<[Cube3; 21]>(guard);
    }

    fn puzzle_state_history_pop<'id, H: PuzzleStateHistory<'id, Cube3>>(guard: Guard<'id>) {
        let cube3_def = PuzzleDef::<Cube3>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let solved = cube3_def.new_solved_state();
        let r_move = cube3_def.find_move("R").unwrap();
        let r2_move = cube3_def.find_move("R2").unwrap();
        let f2_move = cube3_def.find_move("F2").unwrap();
        let r_prime_move = cube3_def.find_move("R'").unwrap();

        let r_move_index = move_index(&cube3_def, r_move);
        let r2_move_index = move_index(&cube3_def, r2_move);
        let f2_move_index = move_index(&cube3_def, f2_move);
        let r_prime_move_index = move_index(&cube3_def, r_prime_move);

        let mut puzzle_state_history: StackedPuzzleStateHistory<Cube3, H> = (&cube3_def).into();
        puzzle_state_history.resize_if_needed(4);

        unsafe {
            puzzle_state_history.push_stack_unchecked(r_move_index, &cube3_def);
            puzzle_state_history.push_stack_unchecked(r2_move_index, &cube3_def);
        }

        assert_eq!(puzzle_state_history.stack_pointer, 2);
        let mut r_prime_state = solved.clone();
        r_prime_state.replace_compose(
            &solved,
            r_prime_move.puzzle_state(),
            cube3_def.sorted_orbit_defs_ref(),
        );
        assert_eq!(&puzzle_state_history.stack[2].0, &r_prime_state);
        assert_eq!(puzzle_state_history.stack[2].1, r2_move_index);

        puzzle_state_history.pop_stack();

        assert_eq!(puzzle_state_history.stack_pointer, 1);
        let mut r_state = solved.clone();
        r_state.replace_compose(
            &solved,
            r_move.puzzle_state(),
            cube3_def.sorted_orbit_defs_ref(),
        );
        assert_eq!(&puzzle_state_history.stack[1].0, &r_state);

        unsafe {
            puzzle_state_history.push_stack_unchecked(f2_move_index, &cube3_def);
        }

        assert_eq!(puzzle_state_history.stack_pointer, 2);
        let mut r_f2_state = solved.clone();
        r_f2_state.replace_compose(
            &r_state,
            f2_move.puzzle_state(),
            cube3_def.sorted_orbit_defs_ref(),
        );
        assert_eq!(&puzzle_state_history.stack[2].0, &r_f2_state);

        unsafe {
            puzzle_state_history.push_stack_unchecked(f2_move_index, &cube3_def);
            puzzle_state_history.push_stack_unchecked(r_prime_move_index, &cube3_def);
        }

        assert_eq!(puzzle_state_history.stack_pointer, 4);
        assert_eq!(&puzzle_state_history.stack[4].0, &solved);
        assert_eq!(puzzle_state_history.stack[1].1, r_move_index);
        assert_eq!(puzzle_state_history.stack[2].1, f2_move_index);
        assert_eq!(puzzle_state_history.stack[3].1, f2_move_index);
        assert_eq!(puzzle_state_history.stack[4].1, r_prime_move_index);
    }

    #[test]
    fn test_puzzle_state_history_pop() {
        make_guard!(guard);
        puzzle_state_history_pop::<Vec<Cube3>>(guard);
        make_guard!(guard);
        puzzle_state_history_pop::<[Cube3; 21]>(guard);
    }
}

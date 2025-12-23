use crate::orbit_puzzle::OrbitPuzzleStateImplementor;
use generativity::{Guard, Id};
use heuristic_graph_coloring::{VecVecGraph, color_rlf};
use humanize_duration::{Truncate, prelude::DurationExt};
use itertools::Itertools;
use log::debug;
use puzzle_theory::ksolve::KSolve;
use std::{fmt::Debug, hash::Hash, num::NonZeroU8, time::Instant};
use thiserror::Error;

pub mod cube3;
pub mod slice_puzzle;

/// The puzzle state interface at the heart of the cycle combination solver.
/// Users may either use the generic `HeapPuzzle` implementor for any `KSolve`
/// definition or define fast puzzle-specific implementations, like Cube3.
pub trait PuzzleState<'id>: Clone + PartialEq + Debug + 'id {
    type OrbitBytesBuf<'a>: AsRef<[u8]>
    where
        Self: 'a;
    type OrbitIdentifier: OrbitIdentifier<'id> + Copy + Debug;

    /// Get a default multi bit vector for use in `induces_sorted_cycle_structure`
    fn new_aux_mem(sorted_orbit_defs: SortedOrbitDefsRef<'id, '_>) -> AuxMem<'id>;

    /// Create a puzzle state from a sorted transformation and sorted
    /// orbit defs. `sorted_transformations` must to correspond to
    /// `sorted_orbit_defs`.
    ///
    /// # Errors
    ///
    /// If a puzzle state cannot be created from the orbit
    fn try_from_transformations_meta(
        transformations_meta: TransformationsMeta<'id, '_>,
        id: Id<'id>,
    ) -> Result<Self, TransformationsMetaError>;

    /// Compose two puzzle states in place.
    fn replace_compose(
        &mut self,
        a: &Self,
        b: &Self,
        sorted_orbit_defs: SortedOrbitDefsRef<'id, '_>,
    );

    /// Inverse of a puzzle state.
    fn replace_inverse(&mut self, a: &Self, sorted_orbit_defs: SortedOrbitDefsRef<'id, '_>);

    /// The goal state for IDA* search.
    fn induces_sorted_cycle_structure(
        &self,
        sorted_cycle_structure: SortedCycleStructureRef<'id, '_>,
        sorted_orbit_defs: SortedOrbitDefsRef<'id, '_>,
        aux_mem: AuxMemRefMut<'id, '_>,
    ) -> bool;

    /// Get the bytes of the specified orbit index in the form (permutation
    /// vector, orientation vector).
    fn orbit_bytes(
        &self,
        orbit_identifier: Self::OrbitIdentifier,
    ) -> (Self::OrbitBytesBuf<'_>, Self::OrbitBytesBuf<'_>);

    /// Return an integer that corresponds to a bijective mapping of the orbit
    /// identifier's states.
    fn exact_hasher_orbit(&self, orbit_identifier: Self::OrbitIdentifier) -> u64;

    /// Return a representation of the puzzle state that can be soundly hashed.
    fn approximate_hash_orbit(&self, orbit_identifier: Self::OrbitIdentifier) -> impl Hash;

    fn pick_orbit_puzzle(orbit_identifier: Self::OrbitIdentifier) -> OrbitPuzzleStateImplementor;
}

/// Get a usize that "identifies" an orbit. This is implementor-specific.
/// For slice puzzles, the identifier is the starting index of the orbit data
/// in the puzzle state buffer. For specific puzzles the identifier is the
/// index of the orbit in the orbit definition.
pub trait OrbitIdentifier<'id> {
    fn first_orbit_identifier(branded_orbit_def: BrandedOrbitDef<'id>) -> Self;

    #[must_use]
    fn next_orbit_identifier(self, branded_orbit_def: BrandedOrbitDef<'id>) -> Self;

    fn orbit_def(&self) -> OrbitDef;
}

#[derive(Debug)]
pub struct PuzzleDef<'id, P: PuzzleState<'id>> {
    pub moves: Box<[Move<'id, P>]>,
    // indicies into moves
    pub(crate) move_classes: Box<[usize]>,
    pub(crate) symmetries: Box<[Move<'id, P>]>,
    pub(crate) sorted_orbit_defs: Box<[OrbitDef]>,
    name: String,
    id: Id<'id>,
}

#[derive(Error, Debug)]
pub enum KSolveConversionError {
    #[error(
        "Puzzles with set sizes larger than 255 are not yet supported, but it will be in the future"
    )]
    SetSizeTooBig,
    #[error("Invalid move class")]
    InvalidMoveClass,
    #[error("Too many move classes")]
    TooManyMoveClasses,
    #[error("Invalid transformation while processing the KSolve definition: {0}")]
    TransformsMetaError(#[from] TransformationsMetaError),
}

#[derive(Debug, Clone)]
pub struct Move<'id, P: PuzzleState<'id>> {
    puzzle_state: P,
    class_index: usize,
    name: String,
    _id: Id<'id>,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct OrbitDef {
    pub piece_count: NonZeroU8,
    pub orientation_count: NonZeroU8,
}

#[derive(Copy, Clone, Debug)]
pub struct BrandedOrbitDef<'id> {
    pub inner: OrbitDef,
    _id: Id<'id>,
}

#[derive(Clone, Copy)]
pub struct SortedOrbitDefsRef<'id, 'a> {
    pub inner: &'a [OrbitDef],
    id: Id<'id>,
}

#[derive(Clone, Debug)]
pub struct SortedCycleStructure<'id> {
    pub inner: Vec<Vec<(NonZeroU8, bool)>>,
    id: Id<'id>,
}

#[derive(Copy, Clone)]
pub struct SortedCycleStructureRef<'id, 'a> {
    pub inner: &'a [Vec<(NonZeroU8, bool)>],
    _id: Id<'id>,
}

#[derive(Clone, Copy)]
pub struct TransformationsMeta<'id, 'a> {
    sorted_transformations: &'a [Vec<(u8, u8)>],
    sorted_orbit_defs: SortedOrbitDefsRef<'id, 'a>,
}

#[derive(Error, Debug)]
pub enum SortedCycleStructureCreationError {
    #[error(
        "Cycle structure uses too many pieces, expected at most {expected} pieces but got {actual}"
    )]
    TooManyPieces { expected: usize, actual: usize },
    #[error("Cycle structure uses zero-length cycles, which is not allowed")]
    ZeroLengthCycle,
    #[error(
        "There must be the same number of cycle structures as orbit definitions, expected {expected}, got {actual}"
    )]
    MismatchedLength { expected: usize, actual: usize },
}

#[derive(Error, Debug)]
pub enum TransformationsMetaError {
    #[error("Invalid KSolve orbit definitions. Expected: {expected:?}\nActual: {actual:?}")]
    InvalidOrbitDefs {
        expected: Vec<OrbitDef>,
        actual: Vec<OrbitDef>,
    },
    #[error("Not enough buffer space to convert move")]
    NotEnoughBufferSpace,
    #[error("Invalid set count, expected {expected} sets but got {actual}")]
    InvalidSetCount { expected: usize, actual: usize },
    #[error("Invalid piece count, expected {expected} pieces but got {actual}")]
    InvalidPieceCount { expected: u8, actual: usize },
    #[error(
        "Invalid orientation delta, expected a value between 0 and {expected} but got {actual}"
    )]
    InvalidOrientationDelta { expected: u8, actual: u8 },
    #[error("Permutation out of range, expected a value between 1 and {expected} but got {actual}")]
    PermutationOutOfRange { expected: u8, actual: u8 },
    #[error("Move is invalid: {0:?}")]
    InvalidTransformation(Vec<Vec<(u8, u8)>>),
}

pub struct AuxMem<'id> {
    inner: Option<Box<[u8]>>,
    id: Id<'id>,
}

pub struct AuxMemRefMut<'id, 'a> {
    inner: Option<&'a mut [u8]>,
    _id: Id<'id>,
}

impl<'id> AuxMem<'id> {
    fn new(inner: Option<Box<[u8]>>, id: Id<'id>) -> Self {
        AuxMem { inner, id }
    }

    pub fn as_ref_mut(&mut self) -> AuxMemRefMut<'id, '_> {
        AuxMemRefMut {
            inner: self.inner.as_mut().map(AsMut::as_mut),
            _id: self.id,
        }
    }
}

impl AuxMemRefMut<'_, '_> {
    /// Get a mutable reference to the auxiliary memory.
    ///
    /// # Safety
    ///
    /// The caller must ensure that auxilliary memory exists.
    pub unsafe fn aux_mem_unchecked(&mut self) -> &mut [u8] {
        unsafe { self.inner.as_mut().unwrap_unchecked() }
    }
}

impl<'id, 'a> TransformationsMeta<'id, 'a> {
    /// Create a `TransformationMeta` from `sorted_transformations` and
    /// `sorted_orbit_defs`.
    ///
    /// # Errors
    ///
    /// If the fields of the arguments are not valid. See
    /// `TransformationMetaError`.
    pub fn new(
        sorted_transformations: &'a [Vec<(u8, u8)>],
        sorted_orbit_defs: SortedOrbitDefsRef<'id, 'a>,
    ) -> Result<Self, TransformationsMetaError> {
        let actual_set_count = sorted_transformations.len();
        let expected_set_count = sorted_orbit_defs.inner.len();

        if sorted_transformations.len() != sorted_orbit_defs.inner.len() {
            return Err(TransformationsMetaError::InvalidSetCount {
                expected: expected_set_count,
                actual: actual_set_count,
            });
        }

        for (transformation, orbit_def) in sorted_transformations
            .iter()
            .zip(sorted_orbit_defs.branded_copied_iter())
        {
            let expected_piece_count = orbit_def.inner.piece_count.get();
            let actual_piece_count = transformation.len();

            if actual_piece_count != expected_piece_count as usize {
                return Err(TransformationsMetaError::InvalidPieceCount {
                    expected: expected_piece_count,
                    actual: actual_piece_count,
                });
            }

            let max_orientation_delta = orbit_def.inner.orientation_count.get() - 1;
            let mut covered_perms = vec![false; expected_piece_count as usize];

            for &(perm, orientation_delta) in transformation {
                if orientation_delta > max_orientation_delta {
                    return Err(TransformationsMetaError::InvalidOrientationDelta {
                        expected: max_orientation_delta,
                        actual: orientation_delta,
                    });
                }

                match covered_perms.get_mut(perm as usize) {
                    Some(i) => *i = true,
                    None => {
                        return Err(TransformationsMetaError::PermutationOutOfRange {
                            expected: expected_piece_count,
                            actual: perm,
                        });
                    }
                }
            }

            if covered_perms.iter().any(|&x| !x) {
                return Err(TransformationsMetaError::InvalidTransformation(
                    sorted_transformations.to_vec(),
                ));
            }
        }

        Ok(Self {
            sorted_transformations,
            sorted_orbit_defs,
        })
    }

    #[must_use]
    pub fn sorted_transformations(&self) -> &'a [Vec<(u8, u8)>] {
        self.sorted_transformations
    }

    #[must_use]
    pub fn sorted_orbit_defs(&self) -> SortedOrbitDefsRef<'id, 'a> {
        self.sorted_orbit_defs
    }
}

impl<'id> SortedCycleStructure<'id> {
    /// Create a new `SortedCycleStructure`.
    ///
    /// # Errors
    ///
    /// Returns an error if the creation fails.
    pub fn new(
        maybe_cycle_structure: &[Vec<(u8, bool)>],
        sorted_orbit_defs: SortedOrbitDefsRef<'id, '_>,
    ) -> Result<Self, SortedCycleStructureCreationError> {
        if maybe_cycle_structure.len() != sorted_orbit_defs.inner.len() {
            return Err(SortedCycleStructureCreationError::MismatchedLength {
                expected: sorted_orbit_defs.inner.len(),
                actual: maybe_cycle_structure.len(),
            });
        }

        let sorted_cycle_structure = maybe_cycle_structure
            .iter()
            .zip(sorted_orbit_defs.inner)
            .map(|(cycle_structure, &orbit_def)| {
                let max_piece_count_sum = orbit_def.piece_count.get() as usize;
                let mut cycle_structure_checked = Vec::with_capacity(cycle_structure.len());
                let mut piece_count_sum = 0;
                for &(length, oriented) in cycle_structure {
                    if length == 1 && !oriented {
                        continue;
                    }
                    match NonZeroU8::new(length) {
                        Some(length) => {
                            piece_count_sum += length.get() as usize;
                            cycle_structure_checked.push((length, oriented));
                        }
                        None => return Err(SortedCycleStructureCreationError::ZeroLengthCycle),
                    }
                }
                if piece_count_sum > max_piece_count_sum {
                    return Err(SortedCycleStructureCreationError::TooManyPieces {
                        expected: max_piece_count_sum,
                        actual: piece_count_sum,
                    });
                }
                cycle_structure_checked.sort_unstable();
                Ok(cycle_structure_checked)
            })
            .collect::<Result<Vec<_>, SortedCycleStructureCreationError>>()?;

        Ok(Self {
            inner: sorted_cycle_structure,
            id: sorted_orbit_defs.id,
        })
    }

    #[must_use]
    pub fn as_ref(&self) -> SortedCycleStructureRef<'id, '_> {
        SortedCycleStructureRef {
            inner: &self.inner,
            _id: self.id,
        }
    }
}

impl<'id> SortedOrbitDefsRef<'id, '_> {
    pub fn branded_copied_iter(&self) -> impl Iterator<Item = BrandedOrbitDef<'id>> {
        self.inner.iter().copied().map(|orbit_def| BrandedOrbitDef {
            inner: orbit_def,
            _id: self.id,
        })
    }

    fn id(&self) -> Id<'id> {
        self.id
    }
}

impl<'id, P: PuzzleState<'id>> Move<'id, P> {
    /// # Safety
    ///
    /// `self` and `other` must both correspond to `sorted_orbit_defs`.
    pub fn commutes_with(
        &self,
        other: &Self,
        result_1: &mut P,
        result_2: &mut P,
        sorted_orbit_defs: SortedOrbitDefsRef<'id, '_>,
    ) -> bool {
        result_1.replace_compose(&self.puzzle_state, &other.puzzle_state, sorted_orbit_defs);
        result_2.replace_compose(&other.puzzle_state, &self.puzzle_state, sorted_orbit_defs);
        result_1 == result_2
    }

    pub fn class_index(&self) -> usize {
        self.class_index
    }

    pub fn puzzle_state(&self) -> &P {
        &self.puzzle_state
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

fn solved_state_from_sorted_orbit_defs<'id, P: PuzzleState<'id>>(
    sorted_orbit_defs: &[OrbitDef],
    id: Id<'id>,
) -> Result<P, TransformationsMetaError> {
    let sorted_transformations = sorted_orbit_defs
        .iter()
        .copied()
        .map(|orbit_def| {
            (0..orbit_def.piece_count.get())
                .map(|i| (i, 0))
                .collect_vec()
        })
        .collect_vec();
    let sorted_orbit_defs_ref = SortedOrbitDefsRef {
        inner: sorted_orbit_defs,
        id,
    };
    let transformations_meta =
        TransformationsMeta::new(&sorted_transformations, sorted_orbit_defs_ref).unwrap();
    // We can't unwrap here because although `transformations_meta` is valid,
    // other errors like NotEnoughBufferSpace can still be thrown.
    P::try_from_transformations_meta(transformations_meta, id)
}

impl<'id, P: PuzzleState<'id>> PuzzleDef<'id, P> {
    #[must_use]
    pub fn find_move(&self, name: &str) -> Option<&Move<'id, P>> {
        self.moves.iter().find(|move_| move_.name == name)
    }

    #[must_use]
    pub fn find_symmetry(&self, name: &str) -> Option<&Move<'id, P>> {
        self.symmetries.iter().find(|move_| move_.name == name)
    }

    /// Create a new solved puzzle state.
    ///
    /// # Panics
    ///
    /// Panics if the solved state cannot be created from the sorted orbit definitions. We assume you would have wanted to panic here anyways.
    #[must_use]
    pub fn new_solved_state(&self) -> P {
        solved_state_from_sorted_orbit_defs(&self.sorted_orbit_defs, self.id).unwrap()
    }

    #[must_use]
    pub fn sorted_orbit_defs_ref(&self) -> SortedOrbitDefsRef<'id, '_> {
        SortedOrbitDefsRef {
            inner: &self.sorted_orbit_defs,
            id: self.id,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn id(&self) -> Id<'id> {
        self.id
    }

    /// Create a new `PuzzleDef` from a `KSolve` definition and a generativity
    /// `Guard`.
    ///
    /// # Errors
    ///
    /// The `KSolve` definition could not be converted to a `PuzzleDef`. See
    /// `KSolveConversionError`.
    pub fn new(ksolve: &KSolve, guard: Guard<'id>) -> Result<Self, KSolveConversionError> {
        let id = guard.into();
        let ksolve_orbit_defs: Vec<OrbitDef> = ksolve
            .sets()
            .iter()
            .map(|ksolve_set| {
                Ok(OrbitDef {
                    piece_count: ksolve_set
                        .piece_count()
                        .try_into()
                        .map_err(|_| KSolveConversionError::SetSizeTooBig)?,
                    // TODO: limit ori to 128 because of vectorized SlicePuzzle
                    // logic
                    orientation_count: ksolve_set.orientation_count(),
                })
            })
            .collect::<Result<_, KSolveConversionError>>()?;

        let mut arg_indicies = (0..ksolve_orbit_defs.len()).collect_vec();
        arg_indicies.sort_by_key(|&i| {
            (
                ksolve_orbit_defs[i].piece_count.get(),
                ksolve_orbit_defs[i].orientation_count.get(),
            )
        });

        let sorted_orbit_defs = arg_indicies
            .iter()
            .map(|&i| ksolve_orbit_defs[i])
            .collect_vec();

        let solved: P = solved_state_from_sorted_orbit_defs(&sorted_orbit_defs, id)?;

        let sorted_orbit_defs_ref = SortedOrbitDefsRef {
            inner: &sorted_orbit_defs,
            id,
        };

        let mut move_class_groups: Vec<Vec<Move<'id, P>>> = vec![];
        let mut symmetries = Vec::with_capacity(ksolve.symmetries().len());
        let mut move_iter = solved.clone();

        for (i, ksolve_move) in ksolve
            .moves()
            .iter()
            .chain(ksolve.symmetries().iter())
            .enumerate()
        {
            let mut sorted_transformations = ksolve_move
                .transformation()
                .iter()
                .enumerate()
                .map(|(i, perm_and_ori)| {
                    if perm_and_ori.is_empty() {
                        (0..ksolve_orbit_defs[i].piece_count.get())
                            .map(|j| Ok((j, 0)))
                            .collect::<Result<Vec<_>, KSolveConversionError>>()
                    } else {
                        perm_and_ori
                            .iter()
                            .map(|&(perm, orientation)| {
                                // we can unwrap because sorted_orbit_defs exists
                                Ok((
                                    (perm.get() - 1)
                                        .try_into()
                                        .map_err(|_| KSolveConversionError::SetSizeTooBig)?,
                                    orientation,
                                ))
                            })
                            .collect::<Result<Vec<_>, KSolveConversionError>>()
                    }
                })
                .collect::<Result<Vec<Vec<_>>, KSolveConversionError>>()?;
            sorted_transformations = arg_indicies
                .iter()
                .map(|&i| sorted_transformations[i].clone())
                .collect();
            let transformations_meta =
                TransformationsMeta::new(&sorted_transformations, sorted_orbit_defs_ref)?;

            let puzzle_state = P::try_from_transformations_meta(transformations_meta, id)?;

            if i >= ksolve.moves().len() {
                let base_move = Move {
                    name: ksolve_move.name().to_owned(),
                    class_index: 0,
                    puzzle_state,
                    _id: id,
                };
                symmetries.push(base_move);
                continue;
            }

            let init = move_class_groups.is_empty();
            if init {
                move_class_groups.push(vec![]);
            }
            #[allow(clippy::missing_panics_doc)]
            let move_class_group = move_class_groups.last_mut().unwrap();

            if !init {
                #[allow(clippy::missing_panics_doc)]
                let last_move = move_class_group.last().unwrap();
                #[allow(clippy::missing_panics_doc)]
                let base_move = move_class_group.first().unwrap();
                move_iter.replace_compose(
                    &last_move.puzzle_state,
                    &base_move.puzzle_state,
                    sorted_orbit_defs_ref,
                );
            }

            let in_move_class = move_iter == puzzle_state;
            let finished_move_class = move_iter == solved;
            let move_ = Move {
                name: ksolve_move.name().to_owned(),
                class_index: 0,
                puzzle_state,
                _id: id,
            };

            if init {
                move_class_group.push(move_);
            } else if finished_move_class {
                move_class_groups.push(vec![move_]);
            } else if in_move_class {
                move_class_group.push(move_);
            } else {
                return Err(KSolveConversionError::InvalidMoveClass);
            }
        }
        if let Some(move_class_group) = move_class_groups.last()
            && let Some(last_move) = move_class_group.last()
            && let Some(base_move) = move_class_group.first()
        {
            move_iter.replace_compose(
                &last_move.puzzle_state,
                &base_move.puzzle_state,
                sorted_orbit_defs_ref,
            );
        }
        if move_iter != solved {
            return Err(KSolveConversionError::InvalidMoveClass);
        }

        let mut graph = VecVecGraph::new(move_class_groups.len());
        let mut result_1 = move_iter;
        let mut result_2 = solved.clone();
        for (i, move_class_group_1) in move_class_groups.iter().enumerate() {
            for (j, move_class_group_2) in move_class_groups.iter().enumerate().take(i) {
                if !move_class_group_1[0].commutes_with(
                    &move_class_group_2[0],
                    &mut result_1,
                    &mut result_2,
                    sorted_orbit_defs_ref,
                ) {
                    graph.add_edge(i, j);
                }
            }
        }
        let start = Instant::now();
        let coloring = color_rlf(&graph);
        debug!("Coloring in {}", start.elapsed().human(Truncate::Millis));

        let mut coloring = coloring.into_iter().enumerate().collect_vec();
        coloring.sort_by_key(|&(_, color)| color);
        move_class_groups = coloring
            .into_iter()
            .map(|(i, _)| move_class_groups[i].clone())
            .collect_vec();

        let move_classes = move_class_groups.iter_mut().enumerate().fold(
            vec![],
            |mut acc, (move_class_index, move_class_group)| {
                for move_ in &mut *move_class_group {
                    move_.class_index = move_class_index;
                }
                acc.push(match acc.last().copied() {
                    Some(prev) => prev + move_class_group.len(),
                    None => 0,
                });
                acc
            },
        );
        let moves = move_class_groups.into_iter().flatten().collect_vec();
        Ok(PuzzleDef {
            moves: moves.into_boxed_slice(),
            move_classes: move_classes.into_boxed_slice(),
            symmetries: symmetries.into_boxed_slice(),
            sorted_orbit_defs: sorted_orbit_defs.into_boxed_slice(),
            name: ksolve.name().to_owned(),
            id,
        })
    }
}

/// A utility function for testing. Not optimized.
///
/// # Panics
///
/// Panics if the move sequence is invalid.
pub fn apply_moves<'id, P: PuzzleState<'id>>(
    puzzle_def: &PuzzleDef<'id, P>,
    puzzle_state: &P,
    moves: &str,
    repeat: u32,
) -> P {
    let mut result_1 = puzzle_state.clone();
    let mut result_2 = puzzle_state.clone();
    for _ in 0..repeat {
        for name in moves.split_whitespace() {
            let move_ = puzzle_def.find_move(name).unwrap();
            result_2.replace_compose(
                &result_1,
                &move_.puzzle_state,
                puzzle_def.sorted_orbit_defs_ref(),
            );
            std::mem::swap(&mut result_1, &mut result_2);
        }
    }
    result_1
}

/// Return a random 3x3 puzzle state
pub fn apply_random_moves<'id, P: PuzzleState<'id>>(
    puzzle_def: &PuzzleDef<'id, P>,
    solved: &P,
    random_move_count: u32,
) -> P {
    let mut result_1 = solved.clone();
    let mut result_2 = solved.clone();
    for _ in 0..random_move_count {
        #[allow(clippy::missing_panics_doc)]
        let move_ = fastrand::choice(puzzle_def.moves.iter()).unwrap();
        result_1.replace_compose(
            &result_2,
            &move_.puzzle_state,
            puzzle_def.sorted_orbit_defs_ref(),
        );
        std::mem::swap(&mut result_2, &mut result_1);
    }
    result_2
}

#[cfg(test)]
mod tests {
    extern crate test;

    use super::{
        slice_puzzle::{HeapPuzzle, StackPuzzle},
        *,
    };
    use generativity::make_guard;
    use puzzle_theory::puzzle_geometry::parsing::puzzle;
    use test::Bencher;

    type StackCube3<'id> = StackPuzzle<'id, 40>;

    fn commutes_with<'id, P: PuzzleState<'id>>(guard: Guard<'id>) {
        let cube3_def = PuzzleDef::<P>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let mut result_1 = cube3_def.new_solved_state();
        let mut result_2 = result_1.clone();

        let u_move = cube3_def.find_move("U").unwrap();
        let d2_move = cube3_def.find_move("D2").unwrap();
        let r_move = cube3_def.find_move("R").unwrap();

        assert!(u_move.commutes_with(
            u_move,
            &mut result_1,
            &mut result_2,
            cube3_def.sorted_orbit_defs_ref()
        ));
        assert!(d2_move.commutes_with(
            d2_move,
            &mut result_1,
            &mut result_2,
            cube3_def.sorted_orbit_defs_ref()
        ));
        assert!(u_move.commutes_with(
            d2_move,
            &mut result_1,
            &mut result_2,
            cube3_def.sorted_orbit_defs_ref()
        ));
        assert!(!u_move.commutes_with(
            r_move,
            &mut result_1,
            &mut result_2,
            cube3_def.sorted_orbit_defs_ref()
        ));
        assert!(!d2_move.commutes_with(
            r_move,
            &mut result_1,
            &mut result_2,
            cube3_def.sorted_orbit_defs_ref()
        ));
        assert!(!r_move.commutes_with(
            u_move,
            &mut result_1,
            &mut result_2,
            cube3_def.sorted_orbit_defs_ref()
        ));
        assert!(!r_move.commutes_with(
            d2_move,
            &mut result_1,
            &mut result_2,
            cube3_def.sorted_orbit_defs_ref()
        ));
    }

    #[test]
    fn test_commutes_with() {
        make_guard!(guard);
        commutes_with::<StackCube3>(guard);
        make_guard!(guard);
        commutes_with::<HeapPuzzle>(guard);
        #[cfg(simd8and16)]
        {
            make_guard!(guard);
            commutes_with::<cube3::simd8and16::Cube3>(guard);
            make_guard!(guard);
            commutes_with::<cube3::simd8and16::UncompressedCube3>(guard);
        }
        #[cfg(avx2)]
        {
            make_guard!(guard);
            commutes_with::<cube3::avx2::Cube3>(guard);
        }
    }

    #[test]
    fn test_not_enough_buffer_space() {
        make_guard!(guard);
        let try_cube3_def = PuzzleDef::<StackPuzzle<39>>::new(&puzzle("3x3").ksolve(), guard);
        assert!(matches!(
            try_cube3_def,
            Err(KSolveConversionError::TransformsMetaError(
                TransformationsMetaError::NotEnoughBufferSpace
            ))
        ));
    }

    pub fn many_compositions<'id, P: PuzzleState<'id>>(guard: Guard<'id>) {
        let cube3_def = PuzzleDef::<P>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let solved = cube3_def.new_solved_state();
        let also_solved = apply_moves(&cube3_def, &solved, "R F", 105);
        assert_eq!(also_solved, solved);
    }

    #[test]
    fn test_many_compositions() {
        make_guard!(guard);
        many_compositions::<StackCube3>(guard);
        make_guard!(guard);
        many_compositions::<HeapPuzzle>(guard);
        #[cfg(simd8and16)]
        {
            make_guard!(guard);
            many_compositions::<cube3::simd8and16::Cube3>(guard);
            make_guard!(guard);
            many_compositions::<cube3::simd8and16::UncompressedCube3>(guard);
        }
        #[cfg(avx2)]
        {
            make_guard!(guard);
            many_compositions::<cube3::avx2::Cube3>(guard);
        }
    }

    pub fn s_u4_symmetry<'id, P: PuzzleState<'id>>(guard: Guard<'id>) {
        let cube3_def = PuzzleDef::<P>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let s_u4_symmetry = cube3_def.find_symmetry("S_U4").unwrap();
        let solved = cube3_def.new_solved_state();

        let mut result_1 = solved.clone();
        let mut result_2 = solved.clone();
        for _ in 0..4 {
            result_2.replace_compose(
                &result_1,
                &s_u4_symmetry.puzzle_state,
                cube3_def.sorted_orbit_defs_ref(),
            );
            std::mem::swap(&mut result_1, &mut result_2);
        }

        assert_eq!(result_1, solved);
    }

    #[test]
    #[ignore = "symmetries not implemented"]
    fn test_s_u4_symmetry() {
        make_guard!(guard);
        s_u4_symmetry::<StackCube3>(guard);
        make_guard!(guard);
        s_u4_symmetry::<HeapPuzzle>(guard);
        #[cfg(simd8and16)]
        {
            make_guard!(guard);
            s_u4_symmetry::<cube3::simd8and16::Cube3>(guard);
            make_guard!(guard);
            s_u4_symmetry::<cube3::simd8and16::UncompressedCube3>(guard);
        }
        #[cfg(avx2)]
        {
            make_guard!(guard);
            s_u4_symmetry::<cube3::avx2::Cube3>(guard);
        }
    }

    pub fn expanded_move<'id, P: PuzzleState<'id>>(guard: Guard<'id>) {
        let cube3_def = PuzzleDef::<P>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let actual_solved = cube3_def.new_solved_state();
        let expected_solved = apply_moves(
            &cube3_def,
            &actual_solved,
            "R R' D2 D2 U U U2 F B' F' B",
            10,
        );
        assert_eq!(actual_solved, expected_solved);
    }

    #[test]
    fn test_expanded_move() {
        make_guard!(guard);
        expanded_move::<StackCube3>(guard);
        make_guard!(guard);
        expanded_move::<HeapPuzzle>(guard);
        #[cfg(simd8and16)]
        {
            make_guard!(guard);
            expanded_move::<cube3::simd8and16::Cube3>(guard);
            make_guard!(guard);
            expanded_move::<cube3::simd8and16::UncompressedCube3>(guard);
        }
        #[cfg(avx2)]
        {
            make_guard!(guard);
            expanded_move::<cube3::avx2::Cube3>(guard);
        }
    }

    pub fn inversion<'id, P: PuzzleState<'id>>(guard: Guard<'id>) {
        let cube3_def = PuzzleDef::<P>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let solved = cube3_def.new_solved_state();
        let mut result = solved.clone();

        let state_r2_b_prime = apply_moves(&cube3_def, &solved, "R2 B'", 1);
        result.replace_inverse(&state_r2_b_prime, cube3_def.sorted_orbit_defs_ref());

        let state_b_r2 = apply_moves(&cube3_def, &solved, "B R2", 1);
        assert_eq!(result, state_b_r2);

        let in_r_f_cycle = apply_moves(&cube3_def, &solved, "R F", 40);
        result.replace_inverse(&in_r_f_cycle, cube3_def.sorted_orbit_defs_ref());

        let remaining_r_f_cycle = apply_moves(&cube3_def, &solved, "R F", 65);
        assert_eq!(result, remaining_r_f_cycle);

        for i in 1..=5 {
            let state = apply_moves(&cube3_def, &solved, "L F L' F'", i);
            result.replace_inverse(&state, cube3_def.sorted_orbit_defs_ref());
            let remaining_state = apply_moves(&cube3_def, &solved, "L F L' F'", 6 - i);
            assert_eq!(result, remaining_state);
        }
    }

    #[test]
    fn test_inversion() {
        make_guard!(guard);
        inversion::<StackCube3>(guard);
        make_guard!(guard);
        inversion::<HeapPuzzle>(guard);
        #[cfg(simd8and16)]
        {
            make_guard!(guard);
            inversion::<cube3::simd8and16::Cube3>(guard);
            make_guard!(guard);
            inversion::<cube3::simd8and16::UncompressedCube3>(guard);
        }
        #[cfg(avx2)]
        {
            make_guard!(guard);
            inversion::<cube3::avx2::Cube3>(guard);
        }
    }

    pub fn random_inversion<'id, P: PuzzleState<'id>>(guard: Guard<'id>) {
        let cube3_def = PuzzleDef::<P>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let solved = cube3_def.new_solved_state();

        for _ in 0..50 {
            let random_state = apply_random_moves(&cube3_def, &solved, 20);
            let mut result_1 = solved.clone();
            let mut result_2 = solved.clone();
            result_1.replace_inverse(&random_state, cube3_def.sorted_orbit_defs_ref());
            result_2.replace_compose(&result_1, &random_state, cube3_def.sorted_orbit_defs_ref());

            assert_eq!(result_2, solved);
        }
    }

    #[test]
    fn test_random_inversion() {
        make_guard!(guard);
        random_inversion::<StackCube3>(guard);
        make_guard!(guard);
        random_inversion::<HeapPuzzle>(guard);
        #[cfg(simd8and16)]
        {
            make_guard!(guard);
            random_inversion::<cube3::simd8and16::Cube3>(guard);
            make_guard!(guard);
            random_inversion::<cube3::simd8and16::UncompressedCube3>(guard);
        }
        #[cfg(avx2)]
        {
            make_guard!(guard);
            random_inversion::<cube3::avx2::Cube3>(guard);
        }
    }

    pub fn induces_sorted_cycle_structure_within_cycle<'id, P: PuzzleState<'id>>(
        guard: Guard<'id>,
    ) {
        let cube3_def = PuzzleDef::<P>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let solved = cube3_def.new_solved_state();
        let mut aux_mem = P::new_aux_mem(cube3_def.sorted_orbit_defs_ref());

        let order_1260 = apply_moves(&cube3_def, &solved, "R U2 D' B D'", 1);
        let sorted_cycle_structure = SortedCycleStructure::new(
            &[
                vec![(3, true), (5, true)],
                vec![(2, false), (2, true), (7, true)],
            ],
            cube3_def.sorted_orbit_defs_ref(),
        )
        .unwrap();
        assert!(order_1260.induces_sorted_cycle_structure(
            sorted_cycle_structure.as_ref(),
            cube3_def.sorted_orbit_defs_ref(),
            aux_mem.as_ref_mut(),
        ));

        let order_1260_in_cycle = apply_moves(&cube3_def, &solved, "R U2 D' B D'", 209);
        assert!(order_1260_in_cycle.induces_sorted_cycle_structure(
            sorted_cycle_structure.as_ref(),
            cube3_def.sorted_orbit_defs_ref(),
            aux_mem.as_ref_mut(),
        ));
    }

    #[test]
    fn test_induces_sorted_cycle_structure_within_cycle() {
        make_guard!(guard);
        induces_sorted_cycle_structure_within_cycle::<StackCube3>(guard);
        make_guard!(guard);
        induces_sorted_cycle_structure_within_cycle::<HeapPuzzle>(guard);
        #[cfg(simd8and16)]
        {
            make_guard!(guard);
            induces_sorted_cycle_structure_within_cycle::<cube3::simd8and16::Cube3>(guard);
            make_guard!(guard);
            induces_sorted_cycle_structure_within_cycle::<cube3::simd8and16::UncompressedCube3>(
                guard,
            );
        }
        #[cfg(avx2)]
        {
            make_guard!(guard);
            induces_sorted_cycle_structure_within_cycle::<cube3::avx2::Cube3>(guard);
        }
    }

    pub fn induces_sorted_cycle_structure_many<'id, P: PuzzleState<'id>>(guard: Guard<'id>) {
        let cube3_def = PuzzleDef::<P>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let solved = cube3_def.new_solved_state();
        let mut aux_mem = P::new_aux_mem(cube3_def.sorted_orbit_defs_ref());

        let sorted_cycle_structure =
            SortedCycleStructure::new(&[vec![], vec![]], cube3_def.sorted_orbit_defs_ref())
                .unwrap();
        assert!(solved.induces_sorted_cycle_structure(
            sorted_cycle_structure.as_ref(),
            cube3_def.sorted_orbit_defs_ref(),
            aux_mem.as_ref_mut(),
        ));

        let tests = [
            (
                "F2 L' U2 F U F U L' B U' F' U D2 L F2 B'",
                SortedCycleStructure::new(
                    &[vec![(1, true), (3, true)], vec![(1, true), (5, true)]],
                    cube3_def.sorted_orbit_defs_ref(),
                )
                .unwrap(),
            ),
            (
                "U2 L B L2 F U2 B' U2 R U' F R' F' R F' L' U2",
                SortedCycleStructure::new(
                    &[vec![(1, true), (5, true)], vec![(1, true), (7, true)]],
                    cube3_def.sorted_orbit_defs_ref(),
                )
                .unwrap(),
            ),
            (
                "R' U2 R' U2 F' D' L F L2 F U2 F2 D' L' D2 F R2",
                SortedCycleStructure::new(
                    &[vec![(1, true), (3, true)], vec![(1, true), (7, true)]],
                    cube3_def.sorted_orbit_defs_ref(),
                )
                .unwrap(),
            ),
            (
                "B2 U' B' D B' L' D' B U' R2 B2 R U B2 R B' R U",
                SortedCycleStructure::new(
                    &[
                        vec![(1, true), (1, true), (3, true)],
                        vec![(1, true), (7, true)],
                    ],
                    cube3_def.sorted_orbit_defs_ref(),
                )
                .unwrap(),
            ),
            (
                "R2 L2 D' B L2 D' B L' B D2 R2 B2 R' D' B2 L2 U'",
                SortedCycleStructure::new(
                    &[vec![(2, true), (3, true)], vec![(4, true), (5, true)]],
                    cube3_def.sorted_orbit_defs_ref(),
                )
                .unwrap(),
            ),
            (
                "F' B2 R L U2 B U2 L2 F2 U R L B' L' D' R' D' B'",
                SortedCycleStructure::new(
                    &[
                        vec![(1, true), (2, true), (3, true)],
                        vec![(4, true), (5, true)],
                    ],
                    cube3_def.sorted_orbit_defs_ref(),
                )
                .unwrap(),
            ),
            (
                "L' D2 F B2 U F' L2 B R F2 D R' L F R' F' D",
                SortedCycleStructure::new(
                    &[
                        vec![(2, true), (3, true)],
                        vec![(1, true), (4, true), (5, false)],
                    ],
                    cube3_def.sorted_orbit_defs_ref(),
                )
                .unwrap(),
            ),
            (
                "B' L' F2 R U' R2 F' L2 F R' L B L' U' F2 U' D2 L",
                SortedCycleStructure::new(
                    &[
                        vec![(1, true), (2, true), (3, true)],
                        vec![(1, true), (4, true), (5, false)],
                    ],
                    cube3_def.sorted_orbit_defs_ref(),
                )
                .unwrap(),
            ),
            (
                "F2 D2 L' F D R2 F2 U2 L2 F R' B2 D2 R2 U R2 U",
                SortedCycleStructure::new(
                    &[
                        vec![(1, true), (2, false), (3, true)],
                        vec![(4, true), (5, true)],
                    ],
                    cube3_def.sorted_orbit_defs_ref(),
                )
                .unwrap(),
            ),
            (
                "F2 B' R' F' L' D B' U' F U B' U2 D L' F' L' B R2",
                SortedCycleStructure::new(
                    &[
                        vec![(1, true), (2, false), (3, true)],
                        vec![(1, true), (4, true), (5, false)],
                    ],
                    cube3_def.sorted_orbit_defs_ref(),
                )
                .unwrap(),
            ),
            (
                "U L U L2 U2 B2",
                SortedCycleStructure::new(
                    &[
                        vec![(1, true), (2, false), (3, true)],
                        vec![(2, false), (3, false), (3, false)],
                    ],
                    cube3_def.sorted_orbit_defs_ref(),
                )
                .unwrap(),
            ),
            (
                "U",
                SortedCycleStructure::new(
                    &[vec![(4, false)], vec![(4, false)]],
                    cube3_def.sorted_orbit_defs_ref(),
                )
                .unwrap(),
            ),
        ];

        for (i, (moves_str, expected_cts)) in tests.iter().enumerate() {
            let random_state = apply_moves(&cube3_def, &solved, moves_str, 1);

            assert!(random_state.induces_sorted_cycle_structure(
                expected_cts.as_ref(),
                cube3_def.sorted_orbit_defs_ref(),
                aux_mem.as_ref_mut(),
            ));

            assert!(!solved.induces_sorted_cycle_structure(
                expected_cts.as_ref(),
                cube3_def.sorted_orbit_defs_ref(),
                aux_mem.as_ref_mut(),
            ));

            for (j, &(other_moves, _)) in tests.iter().enumerate() {
                if i == j {
                    continue;
                }
                let other_state = apply_moves(&cube3_def, &solved, other_moves, 1);
                assert!(!other_state.induces_sorted_cycle_structure(
                    expected_cts.as_ref(),
                    cube3_def.sorted_orbit_defs_ref(),
                    aux_mem.as_ref_mut()
                ));
            }
        }
    }

    #[test]
    fn test_induces_sorted_cycle_structure_many() {
        make_guard!(guard);
        induces_sorted_cycle_structure_many::<StackCube3>(guard);
        make_guard!(guard);
        induces_sorted_cycle_structure_many::<HeapPuzzle>(guard);
        #[cfg(simd8and16)]
        {
            make_guard!(guard);
            induces_sorted_cycle_structure_many::<cube3::simd8and16::Cube3>(guard);
            make_guard!(guard);
            induces_sorted_cycle_structure_many::<cube3::simd8and16::UncompressedCube3>(guard);
        }
        #[cfg(avx2)]
        {
            make_guard!(guard);
            induces_sorted_cycle_structure_many::<cube3::avx2::Cube3>(guard);
        }
    }

    fn exact_hasher_orbit<'id, P: PuzzleState<'id>>(guard: Guard<'id>) {
        let cube3_def = PuzzleDef::<P>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let solved = cube3_def.new_solved_state();

        for (test_state, exp_hashes) in [
            (solved.clone(), [0, 0]),
            (
                apply_moves(&cube3_def, &solved, "U", 1),
                [24_476_904, 161_792],
            ),
            (
                apply_moves(&cube3_def, &solved, "U", 2),
                [57_868_020, 219_136],
            ),
            (
                apply_moves(&cube3_def, &solved, "R U R' U'", 1),
                [11_876_463, 825_765_658_624],
            ),
            (
                apply_moves(&cube3_def, &solved, "R U2 D' B D'", 1),
                [61_275_986, 279_798_716_817],
            ),
            (
                apply_moves(
                    &cube3_def,
                    &solved,
                    "B2 U' B' D B' L' D' B U' R2 B2 R U B2 R B' R U",
                    1,
                ),
                [857_489, 7_312_476_362],
            ),
            (
                apply_moves(
                    &cube3_def,
                    &solved,
                    "F2 B' R' F' L' D B' U' F U B' U2 D L' F' L' B R2",
                    1,
                ),
                [79_925_404, 38_328_854_695],
            ),
        ] {
            let mut maybe_orbit_identifier: Option<P::OrbitIdentifier> = None;
            for (i, branded_orbit_def) in cube3_def
                .sorted_orbit_defs_ref()
                .branded_copied_iter()
                .enumerate()
            {
                maybe_orbit_identifier = Some(if i == 0 {
                    P::OrbitIdentifier::first_orbit_identifier(branded_orbit_def)
                } else {
                    maybe_orbit_identifier
                        .unwrap()
                        .next_orbit_identifier(branded_orbit_def)
                });
                let orbit_identifier = maybe_orbit_identifier.unwrap();
                let hash = test_state.exact_hasher_orbit(orbit_identifier);
                assert_eq!(hash, exp_hashes[i]);
            }
        }
    }

    #[test]
    #[ignore = "exact hasher orbit not implemented"]
    fn test_exact_hasher_orbit() {
        make_guard!(guard);
        exact_hasher_orbit::<StackCube3>(guard);
        make_guard!(guard);
        exact_hasher_orbit::<HeapPuzzle>(guard);
        #[cfg(simd8and16)]
        {
            make_guard!(guard);
            many_compositions::<cube3::simd8and16::Cube3>(guard);
            make_guard!(guard);
            exact_hasher_orbit::<cube3::simd8and16::UncompressedCube3>(guard);
        }
        #[cfg(avx2)]
        {
            make_guard!(guard);
            exact_hasher_orbit::<cube3::avx2::Cube3>(guard);
        }
    }

    pub fn bench_compose_helper<'id, P: PuzzleState<'id>>(guard: Guard<'id>, b: &mut Bencher) {
        let cube3_def = PuzzleDef::<P>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let mut solved = cube3_def.new_solved_state();
        let r_move = cube3_def.find_move("R").unwrap();
        let f_move = cube3_def.find_move("F").unwrap();
        b.iter(|| {
            test::black_box(&mut solved).replace_compose(
                test::black_box(&r_move.puzzle_state),
                test::black_box(&f_move.puzzle_state),
                cube3_def.sorted_orbit_defs_ref(),
            );
        });
    }

    pub fn bench_inverse_helper<'id, P: PuzzleState<'id>>(guard: Guard<'id>, b: &mut Bencher) {
        let cube3_def = PuzzleDef::<P>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let solved = cube3_def.new_solved_state();
        let mut result = solved.clone();
        let order_1260 = apply_moves(&cube3_def, &solved, "R U2 D' B D'", 100);
        b.iter(|| {
            test::black_box(&mut result).replace_inverse(
                test::black_box(&order_1260),
                cube3_def.sorted_orbit_defs_ref(),
            );
        });
    }

    pub fn bench_induces_sorted_cycle_structure_worst_helper<'id, P: PuzzleState<'id>>(
        guard: Guard<'id>,
        b: &mut Bencher,
    ) {
        let cube3_def = PuzzleDef::<P>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let sorted_cycle_structure = SortedCycleStructure::new(
            &[
                vec![(3, true), (5, true)],
                vec![(2, false), (2, true), (7, true)],
            ],
            cube3_def.sorted_orbit_defs_ref(),
        )
        .unwrap();
        let order_1260 = apply_moves(&cube3_def, &cube3_def.new_solved_state(), "R U2 D' B D'", 1);
        let mut aux_mem = P::new_aux_mem(cube3_def.sorted_orbit_defs_ref());
        b.iter(|| {
            test::black_box(&order_1260).induces_sorted_cycle_structure(
                test::black_box(sorted_cycle_structure.as_ref()),
                cube3_def.sorted_orbit_defs_ref(),
                aux_mem.as_ref_mut(),
            );
        });
    }

    pub fn bench_induces_sorted_cycle_structure_average_helper<'id, P: PuzzleState<'id>>(
        guard: Guard<'id>,
        b: &mut Bencher,
    ) {
        let cube3_def = PuzzleDef::<P>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let solved = cube3_def.new_solved_state();

        // let sorted_cycle_structures = [
        //     [
        //         ct(&[(3, true), (5, true)]),
        //         ct(&[(2, false), (2, true), (7, true)]),
        //     ],
        //     [ct(&[(1, true), (3, true)]), ct(&[(1, true), (5, true)])],
        //     [ct(&[(2, true), (3, true)]), ct(&[(4, true), (5, true)])],
        //     [
        //         ct(&[(1, true), (2, true), (3, true)]),
        //         ct(&[(4, true), (5, true)]),
        //     ],
        //     [
        //         ct(&[(2, true), (3, true)]),
        //         ct(&[(1, true), (4, true), (5, false)]),
        //     ],
        //     [ct(&[(4, false)]), ct(&[(4, false)])],
        // ];
        let sorted_cycle_structures = [
            SortedCycleStructure::new(
                &[
                    vec![(3, true), (5, true)],
                    vec![(2, false), (2, true), (7, true)],
                ],
                cube3_def.sorted_orbit_defs_ref(),
            )
            .unwrap(),
            SortedCycleStructure::new(
                &[vec![(1, true), (3, true)], vec![(1, true), (5, true)]],
                cube3_def.sorted_orbit_defs_ref(),
            )
            .unwrap(),
            SortedCycleStructure::new(
                &[vec![(2, true), (3, true)], vec![(4, true), (5, true)]],
                cube3_def.sorted_orbit_defs_ref(),
            )
            .unwrap(),
            SortedCycleStructure::new(
                &[
                    vec![(1, true), (2, true), (3, true)],
                    vec![(4, true), (5, true)],
                ],
                cube3_def.sorted_orbit_defs_ref(),
            )
            .unwrap(),
            SortedCycleStructure::new(
                &[
                    vec![(2, true), (3, true)],
                    vec![(1, true), (4, true), (5, false)],
                ],
                cube3_def.sorted_orbit_defs_ref(),
            )
            .unwrap(),
            SortedCycleStructure::new(
                &[vec![(4, false)], vec![(4, false)]],
                cube3_def.sorted_orbit_defs_ref(),
            )
            .unwrap(),
        ];
        let sorted_cycle_structures: Vec<_> = sorted_cycle_structures
            .into_iter()
            .cycle()
            .take(1000)
            .collect();
        let mut sorted_cycle_structure_iter = sorted_cycle_structures.iter().cycle();

        let random_1000: Vec<P> = (0..1000)
            .map(|_| apply_random_moves(&cube3_def, &solved, 20))
            .collect();
        let mut random_iter = random_1000.iter().cycle();

        let mut aux_mem = P::new_aux_mem(cube3_def.sorted_orbit_defs_ref());
        b.iter(|| {
            test::black_box(unsafe { random_iter.next().unwrap_unchecked() })
                .induces_sorted_cycle_structure(
                    test::black_box(unsafe {
                        sorted_cycle_structure_iter
                            .next()
                            .unwrap_unchecked()
                            .as_ref()
                    }),
                    cube3_def.sorted_orbit_defs_ref(),
                    aux_mem.as_ref_mut(),
                );
        });
    }

    // --- HeapPuzzle benchmarks ---

    #[bench]
    fn bench_compose_cube3_heap(b: &mut Bencher) {
        make_guard!(guard);
        bench_compose_helper::<HeapPuzzle>(guard, b);
    }

    #[bench]
    fn bench_inverse_cube3_heap(b: &mut Bencher) {
        make_guard!(guard);
        bench_inverse_helper::<HeapPuzzle>(guard, b);
    }

    #[bench]
    fn bench_induces_sorted_cycle_structure_cube3_heap_worst(b: &mut Bencher) {
        make_guard!(guard);
        bench_induces_sorted_cycle_structure_worst_helper::<HeapPuzzle>(guard, b);
    }

    #[bench]
    fn bench_induces_sorted_cycle_structure_cube3_heap_average(b: &mut Bencher) {
        make_guard!(guard);
        bench_induces_sorted_cycle_structure_average_helper::<HeapPuzzle>(guard, b);
    }

    // --- simd8and16::UncompressedCube3 benchmarks ---

    #[bench]
    #[cfg_attr(not(simd8and16), ignore = "simd8and16 not enabled")]
    fn bench_compose_uncompressed_cube3_simd8and16(b: &mut Bencher) {
        make_guard!(guard);
        bench_compose_helper::<cube3::simd8and16::UncompressedCube3>(guard, b);
    }

    #[bench]
    #[cfg_attr(not(simd8and16), ignore = "simd8and16 not enabled")]
    fn bench_inverse_uncompressed_cube3_simd8and16(b: &mut Bencher) {
        make_guard!(guard);
        bench_inverse_helper::<cube3::simd8and16::UncompressedCube3>(guard, b);
    }

    #[bench]
    #[cfg_attr(not(simd8and16), ignore = "simd8and16 not enabled")]
    fn bench_induces_sorted_cycle_structure_uncompressed_cube3_simd8and16_worst(b: &mut Bencher) {
        make_guard!(guard);
        bench_induces_sorted_cycle_structure_worst_helper::<cube3::simd8and16::UncompressedCube3>(
            guard, b,
        );
    }

    #[bench]
    #[cfg_attr(not(simd8and16), ignore = "simd8and16 not enabled")]
    fn bench_induces_sorted_cycle_structure_uncompressed_cube3_simd8and16_average(b: &mut Bencher) {
        make_guard!(guard);
        bench_induces_sorted_cycle_structure_average_helper::<cube3::simd8and16::UncompressedCube3>(
            guard, b,
        );
    }

    // --- simd8and16::Cube3 benchmarks ---

    #[bench]
    #[cfg_attr(not(simd8and16), ignore = "simd8and16 not enabled")]
    fn bench_compose_cube3_simd8and16(b: &mut Bencher) {
        make_guard!(guard);
        bench_compose_helper::<cube3::simd8and16::Cube3>(guard, b);
    }

    #[bench]
    #[cfg_attr(not(simd8and16), ignore = "simd8and16 not enabled")]
    fn bench_inverse_cube3_simd8and16(b: &mut Bencher) {
        make_guard!(guard);
        bench_inverse_helper::<cube3::simd8and16::Cube3>(guard, b);
    }

    #[bench]
    #[cfg_attr(not(simd8and16), ignore = "simd8and16 not enabled")]
    fn bench_induces_sorted_cycle_structure_cube3_simd8and16_worst(b: &mut Bencher) {
        make_guard!(guard);
        bench_induces_sorted_cycle_structure_worst_helper::<cube3::simd8and16::Cube3>(guard, b);
    }

    #[bench]
    #[cfg_attr(not(simd8and16), ignore = "simd8and16 not enabled")]
    fn bench_induces_sorted_cycle_structure_cube3_simd8and16_average(b: &mut Bencher) {
        make_guard!(guard);
        bench_induces_sorted_cycle_structure_average_helper::<cube3::simd8and16::Cube3>(guard, b);
    }

    // --- avx2::Cube3 benchmarks ---

    #[bench]
    #[cfg_attr(not(avx2), ignore = "AVX2 not enabled")]
    fn bench_compose_cube3_avx2(b: &mut Bencher) {
        make_guard!(guard);
        bench_compose_helper::<cube3::avx2::Cube3>(guard, b);
    }

    #[bench]
    #[cfg_attr(not(avx2), ignore = "AVX2 not enabled")]
    fn bench_inverse_cube3_avx2(b: &mut Bencher) {
        make_guard!(guard);
        bench_inverse_helper::<cube3::avx2::Cube3>(guard, b);
    }

    #[bench]
    #[cfg_attr(not(avx2), ignore = "AVX2 not enabled")]
    fn bench_induces_sorted_cycle_structure_cube3_avx2_worst(b: &mut Bencher) {
        make_guard!(guard);
        bench_induces_sorted_cycle_structure_worst_helper::<cube3::avx2::Cube3>(guard, b);
    }

    #[bench]
    #[cfg_attr(not(avx2), ignore = "AVX2 not enabled")]
    fn bench_induces_sorted_cycle_structure_cube3_avx2_average(b: &mut Bencher) {
        make_guard!(guard);
        bench_induces_sorted_cycle_structure_average_helper::<cube3::avx2::Cube3>(guard, b);
    }
}

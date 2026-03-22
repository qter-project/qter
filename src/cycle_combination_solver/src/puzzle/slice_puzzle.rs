//! The default, generic implementation for representing puzzle states.

use std::{fmt::Debug, hint::assert_unchecked, slice};

use generativity::Id;
use itertools::Itertools;

use super::{
    BrandedOrbitDef, OrbitIdentifier, SortedOrbitDefsRef, TransformationsMeta,
    TransformationsMetaError,
};
use crate::{
    FACT_UNTIL_19,
    orbit_puzzle::{
        OrbitPuzzleStateImplementor,
        slice_orbit_puzzle::{
            SliceOrbitPuzzle, induces_sorted_cycle_structure_slice_orbit,
            replace_compose_slice_orbit,
        },
    },
    puzzle::{
        AuxMem, AuxMemRefMut, OrbitDef, PuzzleState, SortedCycleStructure, SortedCycleStructureRef,
    },
};

trait SlicePuzzle<'id>: PartialEq + Debug + Clone + 'id {
    fn as_slice(&self) -> &[u8];
    unsafe fn as_slice_mut(&mut self) -> &mut [u8];
    fn try_from_transformations_meta(
        transformations_meta: TransformationsMeta<'id, '_>,
        id: Id<'id>,
    ) -> Result<Self, TransformationsMetaError>
    where
        Self: Sized;
}

/// A puzzle state represented as a slice of bytes on the stack. Each orbit is
/// represented as a permutation vector followed by an orientation vector.
#[derive(Clone, PartialEq, Debug)]
pub struct StackPuzzle<'id, const N: usize>([u8; N], Id<'id>);

/// A puzzle state represented as a slice of bytes on the heap. Each orbit is
/// represented as a permutation vector followed by an orientation vector.
#[derive(Clone, PartialEq, Debug)]
pub struct HeapPuzzle<'id>(Box<[u8]>, Id<'id>);

/// A newtyped index into the start of an orbit in a `StackPuzzle` or
/// `HeapPuzzle`.
#[derive(Clone, Copy, Debug)]
pub struct SliceOrbitIdentifier<'id> {
    base_index: usize,
    branded_orbit_def: BrandedOrbitDef<'id>,
}

// TODO: what happens if this impl is wrong? if UB, mark unsafe
impl<'id> OrbitIdentifier<'id> for SliceOrbitIdentifier<'id> {
    fn first_orbit_identifier(branded_orbit_def: BrandedOrbitDef<'id>) -> Self {
        SliceOrbitIdentifier {
            base_index: 0,
            branded_orbit_def,
        }
    }

    fn next_orbit_identifier(
        self,
        branded_orbit_def: BrandedOrbitDef<'id>,
    ) -> SliceOrbitIdentifier<'id> {
        // TODO: panic if out of bounds
        SliceOrbitIdentifier {
            base_index: self.base_index
                + self.branded_orbit_def.inner.piece_count.get() as usize * 2,
            branded_orbit_def,
        }
    }

    fn orbit_def(&self) -> OrbitDef {
        self.branded_orbit_def.inner
    }
}

impl<'id, S: SlicePuzzle<'id>> PuzzleState<'id> for S {
    type OrbitBytesBuf<'a>
        = &'a [u8]
    where
        Self: 'a;
    type OrbitIdentifier = SliceOrbitIdentifier<'id>;

    fn new_aux_mem(sorted_orbit_defs: SortedOrbitDefsRef<'id, '_>) -> AuxMem<'id> {
        AuxMem {
            inner: Some(
                vec![
                    0;
                    sorted_orbit_defs
                        .inner
                        .last()
                        .unwrap()
                        .piece_count
                        .get()
                        .div_ceil(4) as usize
                ]
                .into_boxed_slice(),
            ),
            id: sorted_orbit_defs.id,
        }
    }

    fn try_from_transformations_meta(
        transformations_meta: TransformationsMeta<'id, '_>,
        id: Id<'id>,
    ) -> Result<Self, TransformationsMetaError> {
        Self::try_from_transformations_meta(transformations_meta, id)
    }

    fn replace_compose(
        &mut self,
        a: &Self,
        b: &Self,
        sorted_orbit_defs: SortedOrbitDefsRef<'id, '_>,
    ) {
        let slice_orbit_states_mut = unsafe { self.as_slice_mut() };
        let a = a.as_slice();
        let b = b.as_slice();
        debug_assert_eq!(
            sorted_orbit_defs
                .branded_copied_iter()
                .map(|branded_orbit_def| slice_orbit_size(branded_orbit_def.inner))
                .sum::<usize>(),
            slice_orbit_states_mut.len()
        );
        debug_assert_eq!(slice_orbit_states_mut.len(), a.len());
        debug_assert_eq!(a.len(), b.len());

        let mut base = 0;
        for branded_orbit_def in sorted_orbit_defs.branded_copied_iter() {
            unsafe {
                replace_compose_slice_orbit(
                    slice_orbit_states_mut,
                    base,
                    a,
                    b,
                    branded_orbit_def.inner,
                );
            }
            base += slice_orbit_size(branded_orbit_def.inner);
        }
    }

    fn replace_inverse(&mut self, a: &Self, sorted_orbit_defs: SortedOrbitDefsRef<'id, '_>) {
        let slice_orbit_states_mut = unsafe { self.as_slice_mut() };
        let a = a.as_slice();
        debug_assert_eq!(
            sorted_orbit_defs
                .branded_copied_iter()
                .map(|branded_orbit_def| slice_orbit_size(branded_orbit_def.inner))
                .sum::<usize>(),
            slice_orbit_states_mut.len()
        );
        debug_assert_eq!(slice_orbit_states_mut.len(), a.len());

        let mut base = 0;
        for branded_orbit_def in sorted_orbit_defs.branded_copied_iter() {
            let piece_count = branded_orbit_def.inner.piece_count.get();
            let orientation_count = branded_orbit_def.inner.orientation_count.get();
            // SAFETY: Permutation vectors and orientation vectors are shuffled
            // around, based on code from twsearch [1]. Testing has shown this is
            // sound.
            //
            // [1] https://github.com/cubing/twsearch
            if orientation_count == 1 {
                for i in 0..piece_count {
                    let base_i = base + i as usize;
                    unsafe {
                        *slice_orbit_states_mut.get_unchecked_mut(base + a[base_i] as usize) = i;
                        *slice_orbit_states_mut
                            .get_unchecked_mut(base + (a[base_i] + piece_count) as usize) = 0;
                    }
                }
            } else {
                for i in 0..piece_count {
                    let base_i = base + i as usize;
                    unsafe {
                        *slice_orbit_states_mut.get_unchecked_mut(base + (a[base_i]) as usize) = i;
                        *slice_orbit_states_mut
                            .get_unchecked_mut(base + (a[base_i] + piece_count) as usize) =
                            (orientation_count - a[base_i + piece_count as usize])
                                .min(a[base_i + piece_count as usize].wrapping_neg());
                    }
                }
            }
            base += slice_orbit_size(branded_orbit_def.inner);
        }
    }

    fn induces_sorted_cycle_structure(
        &self,
        sorted_cycle_structure: SortedCycleStructureRef<'id, '_>,
        sorted_orbit_defs: SortedOrbitDefsRef<'id, '_>,
        aux_mem: AuxMemRefMut<'id, '_>,
    ) -> bool {
        let slice_orbit_states = self.as_slice();
        let aux_mem = unsafe { aux_mem.inner.unwrap_unchecked() };
        unsafe {
            assert_unchecked(
                sorted_cycle_structure.inner.len() == sorted_cycle_structure.inner.len(),
            );
        }
        let mut base = 0;
        for (branded_orbit_def, sorted_cycle_structure_orbit) in sorted_orbit_defs
            .branded_copied_iter()
            .zip(sorted_cycle_structure.inner.iter())
        {
            unsafe {
                if !induces_sorted_cycle_structure_slice_orbit(
                    slice_orbit_states,
                    base,
                    sorted_cycle_structure_orbit,
                    branded_orbit_def.inner,
                    aux_mem,
                ) {
                    return false;
                }
            };
            base += slice_orbit_size(branded_orbit_def.inner);
        }
        true
    }

    fn orbit_bytes(
        &self,
        orbit_identifier: Self::OrbitIdentifier,
    ) -> (Self::OrbitBytesBuf<'_>, Self::OrbitBytesBuf<'_>) {
        let slice_orbit_states = self.as_slice();
        (
            unsafe {
                slice::from_raw_parts(
                    slice_orbit_states.as_ptr().add(orbit_identifier.base_index),
                    orbit_identifier.branded_orbit_def.inner.piece_count.get() as usize,
                )
            },
            unsafe {
                slice::from_raw_parts(
                    slice_orbit_states.as_ptr().add(
                        orbit_identifier.base_index
                            + orbit_identifier.branded_orbit_def.inner.piece_count.get() as usize,
                    ),
                    orbit_identifier.branded_orbit_def.inner.piece_count.get() as usize,
                )
            },
        )
    }

    fn exact_hasher_orbit(&self, orbit_identifier: Self::OrbitIdentifier) -> u64 {
        let (perm, ori) = PuzzleState::orbit_bytes(self, orbit_identifier);
        unsafe { exact_hasher_slice_orbit_bytes(perm, ori, orbit_identifier.orbit_def()) }
    }

    fn approximate_hash_orbit(
        &self,
        orbit_identifier: Self::OrbitIdentifier,
    ) -> impl std::hash::Hash {
        let slice_orbit_states = self.as_slice();
        let start = orbit_identifier.base_index;
        let end = orbit_identifier
            .next_orbit_identifier(orbit_identifier.branded_orbit_def)
            .base_index;
        unsafe { slice::from_raw_parts(slice_orbit_states.as_ptr().add(start), end - start) }
    }

    fn pick_orbit_puzzle(orbit_identifier: Self::OrbitIdentifier) -> OrbitPuzzleStateImplementor {
        let orbit_def = orbit_identifier.orbit_def();
        let perm = (0..orbit_def.piece_count.get()).collect_vec();
        let ori = vec![0; orbit_def.piece_count.get() as usize];
        unsafe {
            SliceOrbitPuzzle::from_orbit_transformation_and_def_unchecked(perm, ori, orbit_def)
                .into()
        }
    }
}

impl<'id, const N: usize> SlicePuzzle<'id> for StackPuzzle<'id, N> {
    fn as_slice(&self) -> &[u8] {
        &self.0
    }

    unsafe fn as_slice_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }

    fn try_from_transformations_meta(
        transformations_meta: TransformationsMeta<'id, '_>,
        id: Id<'id>,
    ) -> Result<Self, TransformationsMetaError>
    where
        Self: Sized,
    {
        let mut slice_orbit_states = [0_u8; N];
        transformation_meta_to_slice(&mut slice_orbit_states, transformations_meta)?;
        Ok(StackPuzzle(slice_orbit_states, id))
    }
}

impl<'id> SlicePuzzle<'id> for HeapPuzzle<'id> {
    fn as_slice(&self) -> &[u8] {
        &self.0
    }

    unsafe fn as_slice_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }

    fn try_from_transformations_meta(
        transformations_meta: TransformationsMeta<'id, '_>,
        id: Id<'id>,
    ) -> Result<Self, TransformationsMetaError>
    where
        Self: Sized,
    {
        let sorted_orbit_defs = transformations_meta.sorted_orbit_defs();
        let mut slice_orbit_states = vec![
            0_u8;
            sorted_orbit_defs
                .branded_copied_iter()
                .map(|branded_orbit_def| slice_orbit_size(branded_orbit_def.inner))
                .sum::<usize>()
        ]
        .into_boxed_slice();
        // No validation needed. from_sorted_transformations_unchecked creates
        // an orbit states buffer that is guaranteed to be the right size, and
        // there is no restriction on the expected orbit defs
        transformation_meta_to_slice(&mut slice_orbit_states, transformations_meta).unwrap();
        Ok(HeapPuzzle(slice_orbit_states, id))
    }
}

/// Populate `slice_orbit_states` with `transformation_metas`.
fn transformation_meta_to_slice(
    slice_orbit_states: &mut [u8],
    transformations_meta: TransformationsMeta,
) -> Result<(), TransformationsMetaError> {
    let sorted_orbit_defs = transformations_meta.sorted_orbit_defs();

    if slice_orbit_states.len()
        < sorted_orbit_defs
            .branded_copied_iter()
            .map(|branded_orbit_def| slice_orbit_size(branded_orbit_def.inner))
            .sum()
    {
        return Err(TransformationsMetaError::NotEnoughBufferSpace);
    }
    let sorted_transformations = transformations_meta.sorted_transformations();
    let mut base = 0;
    for (transformation, branded_orbit_def) in sorted_transformations
        .iter()
        .zip(sorted_orbit_defs.branded_copied_iter())
    {
        let piece_count = branded_orbit_def.inner.piece_count.get();
        // TODO: make this more efficient:
        // - zero orientation mod optimization (change next_orbit_identifier_slice too)
        // - avoid the transformation for identities entirely
        for i in 0..piece_count {
            let (perm, orientation_delta) = transformation[i as usize];
            slice_orbit_states[base + (i + piece_count) as usize] = orientation_delta;
            slice_orbit_states[base + i as usize] = perm;
        }
        base += slice_orbit_size(branded_orbit_def.inner);
    }
    Ok(())
}

// TODO: https://stackoverflow.com/a/24689277 https://freedium.cfd/https://medium.com/@benjamin.botto/sequentially-indexing-permutations-a-linear-algorithm-for-computing-lexicographic-rank-a22220ffd6e3 https://stackoverflow.com/questions/1506078/fast-permutation-number-permutation-mapping-algorithms/1506337#1506337
pub(crate) unsafe fn exact_hasher_slice_orbit_bytes(
    perm: &[u8],
    ori: &[u8],
    orbit_def: OrbitDef,
) -> u64 {
    let piece_count = orbit_def.piece_count.get();
    assert!(piece_count as usize <= FACT_UNTIL_19.len());

    let mut exact_perm_hash = u64::from(perm[0]) * FACT_UNTIL_19[(piece_count - 1) as usize];
    for i in 1..piece_count - 1 {
        let mut res = 0;
        for j in (i + 1)..piece_count {
            if perm[j as usize] < perm[i as usize] {
                res += 1;
            }
        }
        exact_perm_hash += res * FACT_UNTIL_19[(piece_count - i - 1) as usize];
    }

    // TODO: IMPORTANT: we need parity as a const generic maybe? or an argument
    // see the screenshot

    let mut exact_ori_hash = 0;
    for i in 0..piece_count - 1 {
        exact_ori_hash *= u64::from(orbit_def.orientation_count.get());
        exact_ori_hash += u64::from(ori[i as usize]);
    }

    exact_perm_hash
        * u64::pow(
            u64::from(orbit_def.orientation_count.get()),
            u32::from(piece_count) - 1,
        )
        + exact_ori_hash
}

#[must_use]
pub fn slice_orbit_size(orbit_def: OrbitDef) -> usize {
    orbit_def.piece_count.get() as usize * 2
}

impl<'id> HeapPuzzle<'id> {
    /// Utility function for testing. Not optimized.
    ///
    /// # Panics
    ///
    /// Panics if the generated cycle structure is deemed to be invalid because
    /// of bad implementation of the function.
    #[must_use]
    pub fn sorted_cycle_structure(
        &self,
        sorted_orbit_defs: SortedOrbitDefsRef<'id, '_>,
        aux_mem: &mut AuxMem<'id>,
    ) -> SortedCycleStructure<'id> {
        let aux_mem_inner = aux_mem.inner.as_mut().unwrap().as_mut();
        let mut cycle_structure = vec![];
        let mut base = 0;
        for branded_orbit_def in sorted_orbit_defs.branded_copied_iter() {
            let mut cycle_structure_piece = vec![];
            aux_mem_inner.fill(0);
            let piece_count = branded_orbit_def.inner.piece_count.get() as usize;
            for i in 0..piece_count {
                let (div, rem) = (i / 4, i % 4);
                if aux_mem_inner[div] & (1 << rem) != 0 {
                    continue;
                }

                aux_mem_inner[div] |= 1 << rem;
                let mut actual_cycle_length = 1;
                let mut piece = self.0[base + i] as usize;
                let mut orientation_sum = self.0[base + piece + piece_count];

                while piece != i {
                    actual_cycle_length += 1;
                    let (div, rem) = (piece / 4, piece % 4);
                    aux_mem_inner[div] |= 1 << rem;
                    piece = self.0[base + piece] as usize;
                    orientation_sum += self.0[base + piece + piece_count];
                }

                let actual_orients =
                    orientation_sum % branded_orbit_def.inner.orientation_count != 0;
                if actual_cycle_length != 1 || actual_orients {
                    cycle_structure_piece.push((actual_cycle_length, actual_orients));
                }
            }
            base += slice_orbit_size(branded_orbit_def.inner);
            cycle_structure_piece.sort_unstable();
            cycle_structure.push(cycle_structure_piece);
        }
        let sorted_cycle_structure =
            SortedCycleStructure::new(&cycle_structure, sorted_orbit_defs).unwrap();
        // We don't actually need to test this function because we have this
        // assert!(self.indu
        assert!(PuzzleState::induces_sorted_cycle_structure(
            self,
            sorted_cycle_structure.as_ref(),
            sorted_orbit_defs,
            aux_mem.as_ref_mut()
        ));
        sorted_cycle_structure
    }
}

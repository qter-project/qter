//! The default and fallback implementation for 3x3 orbits during pruning table
//! generation.

use std::{cmp::Ordering, hint::unreachable_unchecked, num::NonZeroU8};

use itertools::Itertools;

use super::OrbitPuzzleState;
use crate::{
    orbit_puzzle::OrbitPuzzleStateImplementor,
    puzzle::{
        AuxMemRefMut, OrbitDef,
        slice_puzzle::{exact_hasher_slice_orbit_bytes, slice_orbit_size},
    },
};

/// An orbit state of a puzzle that is represented as a slice of bytes. The
/// memory layout is a permutation vector directly followed by an orientation
/// vector.
#[derive(Clone, PartialEq, Debug, Hash)]
pub struct SliceOrbitPuzzle(Box<[u8]>);

impl OrbitPuzzleState for SliceOrbitPuzzle {
    unsafe fn replace_compose(
        &mut self,
        a: &OrbitPuzzleStateImplementor,
        b: &OrbitPuzzleStateImplementor,
        orbit_def: OrbitDef,
    ) {
        let OrbitPuzzleStateImplementor::SliceOrbitPuzzle(a) = a else {
            unsafe { unreachable_unchecked() };
        };
        let OrbitPuzzleStateImplementor::SliceOrbitPuzzle(b) = b else {
            unsafe { unreachable_unchecked() };
        };
        // SAFETY: `a`, `b`, and `self` are all branded to correspond to
        // `orbit_def`.
        unsafe { replace_compose_slice_orbit(&mut self.0, 0, &a.0, &b.0, orbit_def) };
    }

    unsafe fn induces_sorted_cycle_structure(
        &self,
        sorted_cycle_structure_orbit: &[(NonZeroU8, bool)],
        orbit_def: OrbitDef,
        mut aux_mem: AuxMemRefMut,
    ) -> bool {
        unsafe {
            induces_sorted_cycle_structure_slice_orbit(
                &self.0,
                0,
                sorted_cycle_structure_orbit,
                orbit_def,
                aux_mem.aux_mem_unchecked(),
            )
        }
    }

    unsafe fn exact_hasher(&self, orbit_def: OrbitDef) -> u64 {
        let (perm, ori) = unsafe {
            self.0
                .split_at_unchecked(orbit_def.piece_count.get() as usize)
        };
        unsafe { exact_hasher_slice_orbit_bytes(perm, ori, orbit_def) }
    }
}

impl SliceOrbitPuzzle {
    pub unsafe fn from_orbit_transformation_and_def_unchecked<B: AsRef<[u8]>>(
        perm: B,
        ori: B,
        orbit_def: OrbitDef,
    ) -> Self {
        let mut slice_orbit_states = vec![0_u8; slice_orbit_size(orbit_def)];
        let piece_count = orbit_def.piece_count.get();
        for i in 0..piece_count {
            slice_orbit_states[(piece_count + i) as usize] = ori.as_ref()[i as usize];
            slice_orbit_states[i as usize] = perm.as_ref()[i as usize];
        }
        SliceOrbitPuzzle(slice_orbit_states.into_boxed_slice())
    }

    pub unsafe fn new_solved_state(orbit_def: OrbitDef) -> Self
    where
        Self: Sized,
    {
        let perm = (0..orbit_def.piece_count.get()).collect_vec();
        let ori = vec![0; orbit_def.piece_count.get() as usize];
        unsafe { Self::from_orbit_transformation_and_def_unchecked(perm, ori, orbit_def) }
    }

    pub fn approximate_hash(&self) -> &Self {
        self
    }
}

/// Compose orbits in two slice puzzle states, `a` and `b`, into
/// `slice_orbit_states_mut` at the given `base` index.
///
/// # Safety
///
/// 1) `slice_orbit_states_mut`, `a`, and `b` must all correspond to `orbit_def`
/// 2) `base` must be a valid index to the start of an orbit
#[inline]
pub unsafe fn replace_compose_slice_orbit(
    slice_orbit_states_mut: &mut [u8],
    base: usize,
    a: &[u8],
    b: &[u8],
    orbit_def: OrbitDef,
) {
    let piece_count = orbit_def.piece_count.get() as usize;
    let orientation_count = orbit_def.orientation_count.get();
    // Permutation vectors and orientation vectors are shuffled
    // around, based on code from twsearch [1].
    //
    // [1] https://github.com/cubing/twsearch
    if orientation_count == 1 {
        for i in 0..piece_count {
            let base_i = base + i;
            // SAFETY: the caller guarantees that everything is in bounds.
            // Testing has shown this is sound.
            unsafe {
                let pos = *a.get_unchecked(base + *b.get_unchecked(base_i) as usize);
                *slice_orbit_states_mut.get_unchecked_mut(base_i) = pos;
                *slice_orbit_states_mut.get_unchecked_mut(base_i + piece_count) = 0;
            }
        }
    } else {
        for i in 0..piece_count {
            let base_i = base + i;
            // SAFETY: the caller guarantees that everything is in bounds.
            // Testing has shown this is sound.
            unsafe {
                let pos = *a.get_unchecked(base + *b.get_unchecked(base_i) as usize);
                let a_ori =
                    *a.get_unchecked(base + *b.get_unchecked(base_i) as usize + piece_count);
                let b_ori = *b.get_unchecked(base_i + piece_count);
                *slice_orbit_states_mut.get_unchecked_mut(base_i) = pos;
                *slice_orbit_states_mut.get_unchecked_mut(base_i + piece_count) =
                    (a_ori + b_ori).min((a_ori + b_ori).wrapping_sub(orientation_count));
            }
        }
    }
}

/// Check if a slice puzzle state induces a sorted cycle structure.
///
/// # Safety
///
/// 1) `slice_orbit_states` must correspond to `orbit_def`
/// 2) `base` must be a valid index to the start of an orbit
/// 3) `sorted_cycle_structure_orbit` must be sorted by cycle length and then by
///    `is_oriented`
/// 4) `multi_bv` must be the result of `new_multi_bv` on the same `orbit_def`
#[inline]
pub unsafe fn induces_sorted_cycle_structure_slice_orbit(
    slice_orbit_state: &[u8],
    base: usize,
    sorted_cycle_structure_orbit: &[(NonZeroU8, bool)],
    orbit_def: OrbitDef,
    // We cannot brand this field because a newtype holding a mutable reference
    // cannot be moved in a loop
    aux_mem: &mut [u8],
) -> bool {
    aux_mem.fill(0);
    let mut covered_cycles_count = 0;
    let piece_count = orbit_def.piece_count.get() as usize;
    for i in 0..piece_count {
        let (div, rem) = (i / 4, i % 4);
        // SAFETY: default_aux_mem_slice ensures that (i / 4) always fits
        // in aux_mem
        if unsafe { *aux_mem.get_unchecked(div) } & (1 << rem) != 0 {
            continue;
        }
        // SAFETY: see above
        unsafe {
            *aux_mem.get_unchecked_mut(div) |= 1 << rem;
        }
        let mut actual_cycle_length = 1;
        // SAFETY: sorted_orbit_defs guarantees that base (the orbit state
        // base pointer) + i (a permutation vector element) is valid
        let mut piece = unsafe { *slice_orbit_state.get_unchecked(base + i) } as usize;
        // SAFETY: sorted_orbit_defs guarantees that base (the orbit state
        // base pointer) + i + piece (an orientation vector element) is valid
        let mut orientation_sum =
            unsafe { *slice_orbit_state.get_unchecked(base + piece + piece_count) };

        while piece != i {
            actual_cycle_length += 1;
            let (div, rem) = (piece / 4, piece % 4);
            // SAFETY: default_aux_mem_slice ensures that (piece / 4)
            // always in fits in aux_mem
            unsafe {
                *aux_mem.get_unchecked_mut(div) |= 1 << rem;
            }
            // SAFETY: sorted_orbit_defs guarantees that base (the orbit
            // state base pointer) + piece (a permutation vector element) is
            // valid
            unsafe {
                piece = *slice_orbit_state.get_unchecked(base + piece) as usize;
            }
            // SAFETY: sorted_orbit_defs guarantees that base (the orbit
            // state base pointer) + piece + piece_count (an orientation
            // vector element) is valid
            unsafe {
                orientation_sum += *slice_orbit_state.get_unchecked(base + piece + piece_count);
            }
        }

        let actual_orients = orientation_sum % orbit_def.orientation_count != 0;
        if actual_cycle_length == 1 && !actual_orients {
            continue;
        }
        let mut valid_cycle_index = None;
        for (j, &(expected_cycle_length, expected_orients)) in
            sorted_cycle_structure_orbit.iter().enumerate()
        {
            match expected_cycle_length.get().cmp(&actual_cycle_length) {
                Ordering::Less => (),
                Ordering::Equal => {
                    let (div, rem) = (j / 4, j % 4);
                    if expected_orients == actual_orients
                        // SAFETY: default_aux_mem_slice ensures that (j / 4)
                        // always fits in aux_mem
                        && unsafe { *aux_mem.get_unchecked(div) } & (1 << (rem + 4)) == 0
                    {
                        valid_cycle_index = Some(j);
                        break;
                    }
                }
                Ordering::Greater => return false,
            }
        }
        let Some(valid_cycle_index) = valid_cycle_index else {
            return false;
        };
        let (div, rem) = (valid_cycle_index / 4, valid_cycle_index % 4);
        // SAFETY: default_aux_mem_slice ensures that
        // (valid_cycle_index / 4) always fits in aux_mem
        unsafe {
            *aux_mem.get_unchecked_mut(div) |= 1 << (rem + 4);
        }
        covered_cycles_count += 1;
        // cannot possibly return true if this runs
        if covered_cycles_count > sorted_cycle_structure_orbit.len() {
            return false;
        }
    }
    covered_cycles_count == sorted_cycle_structure_orbit.len()
}

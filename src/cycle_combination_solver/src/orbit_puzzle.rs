use std::{
    hash::Hash,
    num::NonZeroU8,
    simd::{LaneCount, Simd, SupportedLaneCount, cmp::SimdPartialOrd, num::SimdUint},
};

use enum_dispatch::enum_dispatch;
use itertools::Itertools;

use super::FACT_UNTIL_19;
use crate::{
    orbit_puzzle::{cube3::Cube3Edges, cubeN::CubeNCorners, slice_orbit_puzzle::SliceOrbitPuzzle},
    puzzle::{AuxMemRefMut, OrbitDef},
};

pub mod cube3;
#[allow(non_snake_case)]
pub mod cubeN;
pub mod slice_orbit_puzzle;

/// A puzzle state interface for manipulating orbits during pruning table
/// generation. Users may either use the generic `SliceOrbitPuzzle` or define
/// fast puzzle-specific implementations for their orbits.
#[enum_dispatch]
pub trait OrbitPuzzleState: Clone {
    /// Compose two orbit puzzle states in place
    unsafe fn replace_compose(
        &mut self,
        a: &OrbitPuzzleStateImplementor,
        b: &OrbitPuzzleStateImplementor,
        orbit_def: OrbitDef,
    );

    /// Check if the orbit puzzle state induces a sorted cycle structure.
    unsafe fn induces_sorted_cycle_structure(
        &self,
        sorted_cycle_structure_orbit: &[(NonZeroU8, bool)],
        orbit_def: OrbitDef,
        aux_mem: AuxMemRefMut,
    ) -> bool;

    /// An exact hasher for the orbit puzzle state used for exact pruning
    /// tables.
    unsafe fn exact_hasher(&self, orbit_def: OrbitDef) -> u64;
}

pub trait SpecializedOrbitPuzzleState {
    unsafe fn from_implementor_enum_unchecked(
        implementor_enum: &OrbitPuzzleStateImplementor,
    ) -> &Self;
    unsafe fn from_orbit_transformation_unchecked<B: AsRef<[u8]>>(perm: B, ori: B) -> Self;
    fn replace_compose(&mut self, a: &Self, b: &Self);
    fn induces_sorted_cycle_structure(
        &self,
        sorted_cycle_structure_orbit: &[(NonZeroU8, bool)],
    ) -> bool;
    fn exact_hasher(&self) -> u64;
    /// An approximate hash of the orbit puzzle state used for approximate
    /// pruning tables.
    fn approximate_hash(&self) -> impl Hash;

    unsafe fn from_orbit_transformation_and_def_unchecked<B: AsRef<[u8]>>(
        perm: B,
        ori: B,
        _orbit_def: OrbitDef,
    ) -> Self
    where
        Self: Sized,
    {
        unsafe { Self::from_orbit_transformation_unchecked(perm, ori) }
    }
    unsafe fn new_solved_state(orbit_def: OrbitDef) -> Self
    where
        Self: Sized,
    {
        let perm = (0..orbit_def.piece_count.get()).collect_vec();
        let ori = vec![0; orbit_def.piece_count.get() as usize];
        unsafe { Self::from_orbit_transformation_unchecked(perm, ori) }
    }
}

#[enum_dispatch(OrbitPuzzleState)]
#[derive(PartialEq, Clone)]
pub enum OrbitPuzzleStateImplementor {
    SliceOrbitPuzzle,
    Cube3Edges,
    CubeNCorners,
}

impl OrbitPuzzleStateImplementor {
    pub fn approximate_hash(&self) -> impl Hash {
        match self {
            OrbitPuzzleStateImplementor::SliceOrbitPuzzle(s) => {
                fxhash::hash64(s.approximate_hash())
            }
            OrbitPuzzleStateImplementor::Cube3Edges(e) => fxhash::hash64(&e.approximate_hash()),
            OrbitPuzzleStateImplementor::CubeNCorners(c) => fxhash::hash64(&c.approximate_hash()),
        }
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn from_orbit_transformation_unchecked<B: AsRef<[u8]>>(
        &self,
        perm: B,
        ori: B,
        orbit_def: OrbitDef,
    ) -> Self {
        match self {
            OrbitPuzzleStateImplementor::SliceOrbitPuzzle(_) => unsafe {
                SliceOrbitPuzzle::from_orbit_transformation_and_def_unchecked(perm, ori, orbit_def)
                    .into()
            },
            OrbitPuzzleStateImplementor::Cube3Edges(_) => unsafe {
                Cube3Edges::from_orbit_transformation_and_def_unchecked(perm, ori, orbit_def).into()
            },
            OrbitPuzzleStateImplementor::CubeNCorners(_) => unsafe {
                CubeNCorners::from_orbit_transformation_and_def_unchecked(perm, ori, orbit_def)
                    .into()
            },
        }
    }
}

impl<C: SpecializedOrbitPuzzleState + Clone> OrbitPuzzleState for C {
    unsafe fn replace_compose(
        &mut self,
        a: &OrbitPuzzleStateImplementor,
        b: &OrbitPuzzleStateImplementor,
        _orbit_def: OrbitDef,
    ) {
        let a = unsafe { Self::from_implementor_enum_unchecked(a) };
        let b = unsafe { Self::from_implementor_enum_unchecked(b) };
        self.replace_compose(a, b);
    }

    unsafe fn induces_sorted_cycle_structure(
        &self,
        sorted_cycle_structure: &[(NonZeroU8, bool)],
        _orbit_def: OrbitDef,
        _aux_mem: AuxMemRefMut<'_, '_>,
    ) -> bool {
        self.induces_sorted_cycle_structure(sorted_cycle_structure)
    }

    unsafe fn exact_hasher(&self, _orbit_def: OrbitDef) -> u64 {
        self.exact_hasher()
    }
}

/// Efficiently exactly hash an orbit into a u64, panicking at compile-time if
/// not possible. This function uses a combination of SIMD lehmer coding and an
/// efficient n-ary base hash. Uses `u16`s for const generics because usize
/// implements From<u16>.
pub(crate) fn exact_hasher_orbit<const PIECE_COUNT: u16, const ORI_COUNT: u16, const LEN: usize>(
    perm: Simd<u8, LEN>,
    ori: Simd<u8, LEN>,
) -> u64
where
    LaneCount<LEN>: SupportedLaneCount,
{
    // Powers of ORI_COUNT used to efficiently hash the orientation to an n-ary
    // base. The hash is essentially a dot product of the orientation vector
    // with the powers of ORI_COUNT
    let powers: Simd<u16, LEN> = const {
        // Everything not a power must be zero to make sure nothing interferes
        let mut arr = [0; LEN];
        let mut i = 0;
        // We do an important check that the next power does not overflow `u16`.
        // The dot product will eventually be collapsed to a value larger than
        // ORI_COUNT.pow(PIECE_COUNT - 2) but less than
        // ORI_COUNT.pow(PIECE_COUNT - 1).
        u16::checked_pow(ORI_COUNT, PIECE_COUNT as u32 - 1).unwrap();
        // The sum of the orientation vector must be divisible by ORI_COUNT.
        // As a consequence, you don't need the last element to uniquely
        // identify an orientation vector, so we skip processing for it by
        // only computing powers up to PIECE_COUNT - 1
        while i < PIECE_COUNT - 1 {
            // Under the hood LLVM splits up the dot product calculation into
            // chunks of 128 bit registers so having a the smallest possible
            // data type (u16) is important
            arr[i as usize] = u16::checked_pow(
                ORI_COUNT,
                (
                    // The powers are computed in reverse order to match the
                    // order of lexicographic permutation with replacement.
                    // Reverse order in general is len - i - 1, and len is
                    // PIECE_COUNT - 1
                    (PIECE_COUNT - 1) - i - 1
                ) as u32,
            )
            .unwrap();
            i += 1;
        }
        Simd::<u16, LEN>::from_array(arr)
    };
    // We compute: lehmer code * number_of_states(n-ary hash) + n-ary hash
    //
    // One thing to note about the last element for Lehmer codes is no matter
    // what, there will always be an equal number of elements to its left that
    // are less than it. This allows us to hard code it to 0 and iterate from 0
    // to PIECE_COUNT - 1
    (0..usize::from(PIECE_COUNT) - 1)
        .map(|i| {
            let lt_before_current_count = if i == 0 {
                // There are no elements left of the first element less than it
                u64::from(perm[0])
            } else {
                // Count how many elements to the left of the current element
                // are less than it
                let lt_current_mask = perm.simd_lt(Simd::<u8, LEN>::splat(perm[i]));
                u64::from((lt_current_mask.to_bitmask() >> i).count_ones())
            };
            // FACT_UNTIL_19[i] = i!
            let fact = FACT_UNTIL_19[usize::from(PIECE_COUNT) - 1 - i];
            lt_before_current_count * fact
        })
        .sum::<u64>()
        // Orientation is a permutation with replacement. The number of states
        // is trivially ORI_COUNT.pow(PIECE_COUNT), but subtract one because the
        // last element is ignored as described above
        * u64::from(ORI_COUNT.pow(u32::from(PIECE_COUNT) - 1))
        // Compute the aforementioned dot product
        + u64::from((ori.cast::<u16>() * powers).reduce_sum())
}

//! A SIMD optimized implementation for N-cube corners for platforms that
//! support AVX2.

#![cfg_attr(not(avx2), allow(dead_code, unused_variables))]

use std::{
    hash::Hash,
    num::NonZeroU8,
    simd::{
        cmp::{SimdPartialEq, SimdPartialOrd},
        u8x8,
    },
};

use crate::orbit_puzzle::{
    OrbitPuzzleStateImplementor, SpecializedOrbitPuzzleState, exact_hasher_orbit,
};

/// A lookup table used to correct corner orientation during composition
const CO_MOD_SWIZZLE: u8x8 = u8x8::from_array([0, 1, 2, 0, 1, 0, 0, 0]);
/// A lookup table used to inverse a corner orientation.
const CO_INV_SWIZZLE: u8x8 = u8x8::from_array([0, 2, 1, 0, 0, 0, 0, 0]);
/// The identity permutation for corners.
const CP_IDENTITY: u8x8 = u8x8::from_array([0, 1, 2, 3, 4, 5, 6, 7]);

/// A SIMD-optimized N-cube corners representation for AVX2 platforms.
/// All cubes have exactly 8 corners.
#[derive(PartialEq, Clone, Hash)]
pub struct CubeNCorners {
    /// Corner permutation using u8x8 SIMD vector
    cp: u8x8,
    /// Corner orientation using u8x8 SIMD vector
    co: u8x8,
}

impl SpecializedOrbitPuzzleState for CubeNCorners {
    unsafe fn from_implementor_enum_unchecked(
        implementor_enum: &OrbitPuzzleStateImplementor,
    ) -> &Self {
        #[cfg(avx2)]
        match implementor_enum {
            OrbitPuzzleStateImplementor::CubeNCorners(c) => c,
            _ => unsafe { std::hint::unreachable_unchecked() },
        }
        #[cfg(not(avx2))]
        unimplemented!()
    }

    unsafe fn from_orbit_transformation_unchecked<B: AsRef<[u8]>>(perm: B, ori: B) -> Self {
        create_from_arrays(perm.as_ref(), ori.as_ref())
    }

    fn replace_compose(&mut self, a: &Self, b: &Self) {
        compose(self, a, b);
    }

    fn induces_sorted_cycle_structure(
        &self,
        sorted_cycle_structure_orbit: &[(NonZeroU8, bool)],
    ) -> bool {
        induces_sorted_cycle_structure(self, sorted_cycle_structure_orbit)
    }

    fn exact_hasher(&self) -> u64 {
        exact_hasher(self)
    }

    fn approximate_hash(&self) -> impl Hash {
        self
    }
}

/// Create a `CubeNCorners` from permutation and orientation arrays
pub fn create_from_arrays(perm: &[u8], ori: &[u8]) -> CubeNCorners {
    debug_assert_eq!(perm.len(), 8);
    debug_assert_eq!(ori.len(), 8);

    let mut cp_array = [0u8; 8];
    let mut co_array = [0u8; 8];

    cp_array.copy_from_slice(perm);
    co_array.copy_from_slice(ori);

    CubeNCorners {
        cp: u8x8::from_array(cp_array),
        co: u8x8::from_array(co_array),
    }
}

/// Compose two corner states using SIMD operations
pub fn compose(result: &mut CubeNCorners, a: &CubeNCorners, b: &CubeNCorners) {
    // Compose corner permutation using the built-in SIMD swizzle
    result.cp = a.cp.swizzle_dyn(b.cp);

    // Corner orientation composition: (A*B)(x).o=A(B(x).c).o+B(x).o
    // Corner orientation is defined as 0, 1, or 2. Adding two corner
    // orientations together may result in 3 or 4. Use a lookup table
    // to perform this correction efficiently.
    result.co = CO_MOD_SWIZZLE.swizzle_dyn(a.co.swizzle_dyn(b.cp) + b.co);
}

/// Check if this state induces the given sorted cycle structure
pub fn induces_sorted_cycle_structure(
    cube: &CubeNCorners,
    sorted_cycle_structure_orbit: &[(NonZeroU8, bool)],
) -> bool {
    let mut seen_cp = cube.cp.simd_eq(CP_IDENTITY);
    let oriented_one_cycle_corner_mask = seen_cp & cube.co.simd_ne(u8x8::splat(0));
    let mut cycle_structure_pointer =
        (oriented_one_cycle_corner_mask.to_bitmask().count_ones() as usize).wrapping_sub(1);

    // Check oriented one cycles
    if cycle_structure_pointer == usize::MAX {
        if let Some(&first_cycle) = sorted_cycle_structure_orbit.first()
            && first_cycle == (1.try_into().unwrap(), true)
        {
            return false;
        }
    } else if cycle_structure_pointer >= sorted_cycle_structure_orbit.len()
        || sorted_cycle_structure_orbit[cycle_structure_pointer] != (1.try_into().unwrap(), true)
    {
        return false;
    }

    let mut reps = NonZeroU8::new(2).unwrap();
    let mut iter_cp = cube.cp;
    let mut iter_co = cube.co;

    while !seen_cp.all() {
        iter_cp = iter_cp.swizzle_dyn(cube.cp);
        iter_co = iter_co.swizzle_dyn(cube.cp) + cube.co;

        let cp_identity_eq = iter_cp.simd_eq(CP_IDENTITY);
        let new_corners = cp_identity_eq & !seen_cp;
        seen_cp |= cp_identity_eq;

        let reps_corner_cycle_count = new_corners.to_bitmask().count_ones();
        if new_corners.any() {
            let mut oriented_corner_mask = (iter_co * u8x8::splat(171)).simd_gt(u8x8::splat(85));
            oriented_corner_mask &= new_corners;
            let reps_oriented_corner_cycle_count = oriented_corner_mask.to_bitmask().count_ones();

            // Unoriented cycles
            if reps_oriented_corner_cycle_count != reps_corner_cycle_count {
                cycle_structure_pointer = cycle_structure_pointer.wrapping_add(
                    ((reps_corner_cycle_count - reps_oriented_corner_cycle_count)
                        / u32::from(reps.get())) as usize,
                );
                if cycle_structure_pointer >= sorted_cycle_structure_orbit.len()
                    || sorted_cycle_structure_orbit[cycle_structure_pointer] != (reps, false)
                {
                    return false;
                }
            }

            // Oriented cycles
            if reps_oriented_corner_cycle_count != 0 {
                cycle_structure_pointer = cycle_structure_pointer.wrapping_add(
                    (reps_oriented_corner_cycle_count / u32::from(reps.get())) as usize,
                );
                if cycle_structure_pointer >= sorted_cycle_structure_orbit.len()
                    || sorted_cycle_structure_orbit[cycle_structure_pointer] != (reps, true)
                {
                    return false;
                }
            }
        }
        // SAFETY: this loop will only ever run 8 times at max because that
        // is the longest cycle length among corners
        reps = unsafe { NonZeroU8::new_unchecked(reps.get() + 1) };
    }

    cycle_structure_pointer == sorted_cycle_structure_orbit.len().wrapping_sub(1)
}

/// Exact hasher for corner orbit
pub fn exact_hasher(cube: &CubeNCorners) -> u64 {
    // Use the exact same constants as 3x3 cube corners since all cubes have 8
    // corners
    const PIECE_COUNT: u16 = 8;
    const ORI_COUNT: u16 = 3;

    exact_hasher_orbit::<PIECE_COUNT, ORI_COUNT, { PIECE_COUNT.next_power_of_two() as usize }>(
        cube.cp, cube.co,
    )
}

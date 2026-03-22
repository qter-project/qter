//! A SIMD optimized implementation for N-cube corners for platforms that
//! support 8 and 16 byte SIMD. This file was derived from
//! `puzzle/cube3/simd8and16.rs`

#![cfg_attr(any(avx2, not(simd8)), allow(dead_code, unused_variables))]

use std::{
    hash::Hash,
    num::NonZeroU8,
    simd::{
        cmp::{SimdPartialEq, SimdPartialOrd},
        u8x8,
    },
};

use crate::{
    orbit_puzzle::{OrbitPuzzleStateImplementor, SpecializedOrbitPuzzleState, exact_hasher_orbit},
    puzzle::cube3::CUBE_3_SORTED_ORBIT_DEFS,
};

/// A lookup table used to correct corner orientation during composition.
const CO_MOD_SWIZZLE: u8x8 = u8x8::from_array([0, 1, 2, 0, 1, 0, 0, 0]);
/// The identity permutation for corners.
const CP_IDENTITY: u8x8 = u8x8::from_array([0, 1, 2, 3, 4, 5, 6, 7]);

/// An uncompressed N-cube corners representation. All cubes have exactly 8
/// corners.
#[derive(PartialEq, Clone, Hash)]
pub struct CubeNCorners {
    /// The corner permutation.
    cp: u8x8,
    /// The corner orientation.
    co: u8x8,
}

impl SpecializedOrbitPuzzleState for CubeNCorners {
    unsafe fn from_implementor_enum_unchecked(
        implementor_enum: &OrbitPuzzleStateImplementor,
    ) -> &Self {
        #[cfg(simd8)]
        match implementor_enum {
            OrbitPuzzleStateImplementor::CubeNCorners(c) => c,
            _ => unsafe { std::hint::unreachable_unchecked() },
        }
        #[cfg(not(simd8))]
        unimplemented!()
    }

    unsafe fn from_orbit_transformation_unchecked<B: AsRef<[u8]>>(perm: B, ori: B) -> Self {
        // dbg!(perm.as_ref(), ori.as_ref());
        // let perm = unsafe { perm.as_ref().try_into().unwrap_unchecked() };
        // let ori = unsafe { ori.as_ref().try_into().unwrap_unchecked() };
        // TODO: fix
        let perm = perm.as_ref();
        let perm = unsafe { std::ptr::read(perm.as_ptr().cast::<[u8; 8]>()) };
        let ori = ori.as_ref();
        let ori = unsafe { std::ptr::read(ori.as_ptr().cast::<[u8; 8]>()) };

        CubeNCorners {
            cp: u8x8::from_array(perm),
            co: u8x8::from_array(ori),
        }
    }

    fn replace_compose(&mut self, a: &Self, b: &Self) {
        // Compose corner permutation using the built-in SIMD swizzle
        self.cp = a.cp.swizzle_dyn(b.cp);

        // Corner orientation composition: (A*B)(x).o=A(B(x).c).o+B(x).o
        // Corner orientation is defined as 0, 1, or 2. Adding two corner
        // orientations together may result in 3 or 4. Use a lookup table
        // to perform this correction efficiently.
        self.co = CO_MOD_SWIZZLE.swizzle_dyn(a.co.swizzle_dyn(b.cp) + b.co);
    }

    fn induces_sorted_cycle_structure(
        &self,
        sorted_cycle_structure_orbit: &[(NonZeroU8, bool)],
    ) -> bool {
        let mut seen_cp = self.cp.simd_eq(CP_IDENTITY);
        let oriented_one_cycle_corner_mask = seen_cp & self.co.simd_ne(u8x8::splat(0));
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
            || sorted_cycle_structure_orbit[cycle_structure_pointer]
                != (1.try_into().unwrap(), true)
        {
            return false;
        }

        let mut reps = NonZeroU8::new(2).unwrap();
        let mut iter_cp = self.cp;
        let mut iter_co = self.co;

        while !seen_cp.all() {
            iter_cp = iter_cp.swizzle_dyn(self.cp);
            iter_co = iter_co.swizzle_dyn(self.cp) + self.co;

            let cp_identity_eq = iter_cp.simd_eq(CP_IDENTITY);
            let new_corners = cp_identity_eq & !seen_cp;
            seen_cp |= cp_identity_eq;

            let reps_corner_cycle_count = new_corners.to_bitmask().count_ones();
            if new_corners.any() {
                let mut oriented_corner_mask =
                    (iter_co * u8x8::splat(171)).simd_gt(u8x8::splat(85));
                oriented_corner_mask &= new_corners;
                let reps_oriented_corner_cycle_count =
                    oriented_corner_mask.to_bitmask().count_ones();

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

    fn exact_hasher(&self) -> u64 {
        // Use the exact same constants as 3x3 cube corners since all cubes have
        // 8 corners
        const ORI_COUNT: u16 = CUBE_3_SORTED_ORBIT_DEFS[0].orientation_count.get() as u16;
        const PIECE_COUNT: u16 = CUBE_3_SORTED_ORBIT_DEFS[0].piece_count.get() as u16;

        exact_hasher_orbit::<PIECE_COUNT, ORI_COUNT, { PIECE_COUNT.next_power_of_two() as usize }>(
            self.cp, self.co,
        )
    }

    fn approximate_hash(&self) -> impl Hash {
        self
    }
}

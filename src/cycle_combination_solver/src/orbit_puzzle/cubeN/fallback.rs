#![allow(unused)]

use std::{hash::Hash, num::NonZeroU8};

use crate::orbit_puzzle::{OrbitPuzzleStateImplementor, SpecializedOrbitPuzzleState};

#[derive(Clone, PartialEq)]
pub struct CubeNCorners {
    cp: [u8; 8],
    co: [u8; 8],
}

impl SpecializedOrbitPuzzleState for CubeNCorners {
    unsafe fn from_implementor_enum_unchecked(
        implementor_enum: &OrbitPuzzleStateImplementor,
    ) -> &Self {
        #[cfg(not(any(avx2, simd8)))]
        match implementor_enum {
            OrbitPuzzleStateImplementor::CubeNCorners(c) => c,
            _ => unsafe { std::hint::unreachable_unchecked() },
        }
        #[cfg(any(avx2, simd8))]
        unimplemented!()
    }

    unsafe fn from_orbit_transformation_unchecked<B: AsRef<[u8]>>(perm: B, ori: B) -> Self {
        todo!();
    }

    fn replace_compose(&mut self, a: &Self, b: &Self) {
        todo!();
    }

    fn induces_sorted_cycle_structure(
        &self,
        sorted_cycle_structure_orbit: &[(NonZeroU8, bool)],
    ) -> bool {
        todo!();
    }

    fn exact_hasher(&self) -> u64 {
        todo!()
    }

    fn approximate_hash(&self) -> impl Hash {
        todo!()
    }
}

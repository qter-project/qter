//! SIMD optimized implementations for 3x3 orbits during pruning table
//! generation.

#![allow(unused)]

use std::{hash::Hash, hint::unreachable_unchecked, num::NonZeroU8};

use crate::orbit_puzzle::{OrbitPuzzleStateImplementor, SpecializedOrbitPuzzleState};

pub mod avx2;
pub mod portable;
pub mod simd8and16;

#[derive(PartialEq, Clone, Hash)]
pub struct Cube3Edges;

impl SpecializedOrbitPuzzleState for Cube3Edges {
    unsafe fn from_implementor_enum_unchecked(
        implementor_enum: &OrbitPuzzleStateImplementor,
    ) -> &Self {
        match implementor_enum {
            OrbitPuzzleStateImplementor::Cube3Edges(e) => e,
            _ => unsafe { unreachable_unchecked() },
        }
    }

    unsafe fn from_orbit_transformation_unchecked<B: AsRef<[u8]>>(perm: B, ori: B) -> Self {
        todo!()
    }

    fn replace_compose(&mut self, a: &Self, b: &Self) {
        todo!()
    }

    fn induces_sorted_cycle_structure(
        &self,
        sorted_cycle_structure_orbit: &[(NonZeroU8, bool)],
    ) -> bool {
        todo!()
    }

    fn exact_hasher(&self) -> u64 {
        todo!()
    }

    fn approximate_hash(&self) -> impl Hash {
        self
    }
}

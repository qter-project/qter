#![allow(unused)]

use std::hash::Hash;

use crate::puzzle::{
    SortedCycleStructureRef,
    cube3::common::{CornersTransformation, Cube3OrbitType, Cube3State, EdgesTransformation},
};

#[derive(Debug, PartialEq, Clone)]
pub struct Cube3 {
    cp: [u8; 8],
    co: [u8; 8],
    ep: [u8; 12],
    eo: [u8; 12],
}

impl Cube3State for Cube3 {
    type OrbitBytesBuf = [u8; 12];

    fn from_corner_and_edge_transformations(
        corners_transformation: CornersTransformation<'_>,
        edges_transformation: EdgesTransformation<'_>,
    ) -> Self {
        todo!()
    }

    fn replace_compose(&mut self, a: &Self, b: &Self) {
        todo!()
    }

    fn replace_inverse(&mut self, a: &Self) {
        todo!()
    }

    fn induces_sorted_cycle_structure(
        &self,
        sorted_cycle_structure: SortedCycleStructureRef,
    ) -> bool {
        todo!()
    }

    fn orbit_bytes(
        &self,
        orbit_type: Cube3OrbitType,
    ) -> (Self::OrbitBytesBuf, Self::OrbitBytesBuf) {
        todo!()
    }

    fn exact_hasher_orbit(&self, orbit_type: Cube3OrbitType) -> u64 {
        todo!()
    }

    fn approximate_hash_orbit(&self, orbit_type: Cube3OrbitType) -> impl Hash {
        todo!()
    }
}

use puzzle_theory::{permutations::Permutation, puzzle_geometry::PuzzleGeometry};
use qter_core::architectures::{Architecture, decode};
use serde::Serialize;
use tsify::Tsify;
use wasm_bindgen::prelude::*;

use crate::BigInt;

// declare_ts_alias!(type Two = u8 = "0 | 1");
// declare_ts_alias!(type Three = u8 = "0 | 1 | 2");
// declare_ts_alias!(type Eight = u8 = "0 | 1 | 2 | 3 | 4 | 5 | 6 | 7");
// declare_ts_alias!(type Twelve = u8 = "0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11");

#[wasm_bindgen]
pub struct CubeState {
    cube: CubeStateData,
    registers: Vec<RegisterState>,
}

#[wasm_bindgen]
impl CubeState {
    pub(crate) fn new(perm: &Permutation, puzzle: &PuzzleGeometry, arch: &Architecture) -> Self {
        Self {
            cube: CubeStateData::from_permutation(perm, puzzle),
            registers: RegisterState::decode(perm, arch),
        }
    }

    #[wasm_bindgen(getter, unchecked_return_type = "CubeStateData")]
    pub fn cube(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.cube).unwrap()
    }

    #[wasm_bindgen(getter, unchecked_return_type = "RegisterState[]")]
    pub fn registers(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.registers).unwrap()
    }
}

#[derive(Tsify, Serialize)]
pub struct RegisterState {
    value: BigInt,
    cycle_values: Vec<BigInt>,
}

impl RegisterState {
    fn decode(perm: &Permutation, arch: &Architecture) -> Vec<Self> {
        arch.registers()
            .iter()
            .map(|reg| RegisterState {
                value: BigInt::from(
                    decode(perm, reg.signature_facelets().facelets(), reg.algorithm()).unwrap(),
                ),
                cycle_values: reg
                    .unshared_cycles()
                    .iter()
                    .map(|cycle| {
                        BigInt::from(decode(perm, cycle.facelet_cycle(), reg.algorithm()).unwrap())
                    })
                    .collect(),
            })
            .collect()
    }
}

#[derive(Tsify, Serialize, Clone)]
pub struct CubeStateData {
    facelets: Vec<u8>,
}

impl CubeStateData {
    fn from_permutation(perm: &Permutation, puzzle: &PuzzleGeometry) -> Self {
        Self {
            facelets: perm
                .state()
                .iter_infinite()
                .take(puzzle.non_fixed_stickers().len())
                .map(|v| v as u8)
                .collect::<Vec<_>>(),
        }
        // let pieces_data = puzzle.pieces_data();
        // let [corners, edges] = pieces_data.orbits() else {
        //     unreachable!()
        // };
        // let (corners, edges) = if corners.pieces().len() == 8 {
        //     (corners, edges)
        // } else {
        //     (edges, corners)
        // };
        // fn mod_sub(a: usize, b: usize, m: usize) -> usize {
        //     ((a % m) + m - (b % m)) % m
        // }
        // fn orbit_array<const N: usize>(
        //     orbit: &OrbitData,
        //     pieces_data: &PiecesData,
        //     perm: &Permutation,
        // ) -> [(u8, u8); N] {
        //     assert_eq!(N, orbit.pieces().len());
        //     core::array::from_fn(|i| {
        //         let sticker = orbit.pieces()[i].stickers()[0];
        //         let maps_to = perm.state().get(sticker);
        //         let ori = mod_sub(
        //             pieces_data.orientation_numbers()[maps_to].num(),
        //             pieces_data.orientation_numbers()[sticker].num(),
        //             orbit.orientation_count(),
        //         );
        //         (maps_to as u8, ori as u8)
        //     })
        // }
        // Self {
        //     corners: orbit_array(corners, &pieces_data, perm),
        //     edges: orbit_array(edges, &pieces_data, perm),
        // }
    }
}

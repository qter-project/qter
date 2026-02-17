use puzzle_theory::{permutations::Permutation, puzzle_geometry::PuzzleGeometry};
use qter_core::architectures::{Architecture, decode};
use serde::Serialize;
use tsify::Tsify;
use wasm_bindgen::prelude::*;

use crate::BigInt;

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
// #[tsify(into_wasm_abi)]
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

#[derive(Tsify, Serialize)]
// #[tsify(into_wasm_abi)]
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
    }
}

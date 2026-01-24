#![feature(type_alias_impl_trait)]

mod cube;
mod interpreter;
mod program;

use puzzle_theory::numbers::{Int, U};
use serde::Serialize;
use wasm_bindgen::prelude::*;

// These (and their methods) are the public WASM exports
pub use crate::{
    cube::{CubeState, CubeStateData, RegisterState},
    interpreter::{Interpreter, StepResult},
    program::{CompileError, Program, Register, RegisterCycle},
};

macro_rules! declare_ts_alias {
    (type $Name:ident = $ty:ty = $ts_ty:literal) => {
        type $Name = $ty;

        #[wasm_bindgen(typescript_custom_section)]
        const _: &str = concat!("type ", stringify!($Name), " = ", $ts_ty, ";");
    };
}
pub(crate) use declare_ts_alias;

#[derive(Serialize, Clone)]
pub struct BigInt(#[serde(with = "serde_wasm_bindgen::preserve")] web_sys::js_sys::BigInt);
#[wasm_bindgen(typescript_custom_section)]
const _: &str = "type BigInt = bigint;";

impl From<&Int<U>> for BigInt {
    fn from(value: &Int<U>) -> Self {
        Self(web_sys::js_sys::BigInt::new(&value.to_string().into()).unwrap())
    }
}

impl From<Int<U>> for BigInt {
    fn from(value: Int<U>) -> Self {
        Self::from(&value)
    }
}

#[wasm_bindgen(start)]
fn start() {
    console_error_panic_hook::set_once();
}

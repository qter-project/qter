#![feature(test, slice_index_methods, portable_simd, abi_vectorcall)]
#![warn(clippy::pedantic)]
#![allow(clippy::similar_names, clippy::too_many_lines)]

pub(crate) mod canonical_fsm;
pub(crate) mod orbit_puzzle;
pub(crate) mod permutator;
pub(crate) mod all_topos;
pub mod pruning;
pub mod puzzle;
pub(crate) mod puzzle_state_history;
pub mod solver;
pub use generativity::*;

#[macro_export]
macro_rules! start {
    ($msg:expr) => {
        concat!("⏳ ", $msg)
    };
}

#[macro_export]
macro_rules! working {
    ($msg:expr) => {
        concat!("🛠  ", $msg)
    };
}

#[macro_export]
macro_rules! success {
    ($msg:expr) => {
        concat!("✅ ", $msg)
    };
}

/// A precomputed factorial table for u8 0! to 19!, where index[i] is i!. We can
/// do one more however it will overflow when adding more to it which is common
/// in context.
const FACT_UNTIL_19: [u64; 20] = {
    let mut arr = [0; 20];
    arr[0] = 1;
    let mut i = 1;
    while i < arr.len() {
        arr[i] = arr[i - 1] * i as u64;
        i += 1;
    }
    arr
};

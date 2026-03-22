#![warn(clippy::pedantic)]
#![allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::bool_to_int_with_if,
    clippy::unreadable_literal,
    // TODO
    clippy::cast_possible_truncation
)]
#![feature(portable_simd, exact_div)]

pub const N: usize = 32;

pub mod finder;
pub mod orderexps;
pub mod puzzle;
pub mod trie;

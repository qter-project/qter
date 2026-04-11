#![warn(clippy::pedantic)]
#![allow(
    non_snake_case,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::bool_to_int_with_if,
    clippy::unreadable_literal,
    // TODO
    clippy::cast_possible_truncation
)]
#![feature(portable_simd, exact_div)]

pub const N: usize = 32;
pub const PRIMES: [u8; N] = [
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131,
];
pub const PRIME_AFTER_LAST: u8 = 137;

pub mod ac3;
pub mod finder;
pub mod number_theory;
pub mod orderexps;
mod possible_orders;
pub mod puzzle;
pub mod trie;

#![warn(clippy::pedantic)]
#![allow(
    non_snake_case,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::bool_to_int_with_if,
    clippy::unreadable_literal,
    clippy::similar_names,
    // TODO
    clippy::cast_possible_truncation
)]
#![feature(portable_simd, exact_div, gen_blocks)]

use bitgauss::BitMatrix;

// pub const N: usize = 16;
// pub const PRIMES: [u8; N] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41,
// 43, 47, 53]; pub const PRIME_AFTER_LAST: u8 = 59;
pub const N: usize = 32;
pub const PRIMES: [u8; N] = [
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131,
];
pub const PRIME_AFTER_LAST: u8 = 137;

fn gauss_jordan_without_zero_rows(m: &mut BitMatrix, expected_rows: usize) -> Vec<usize> {
    let pivot_cols = m.gauss(true);
    if expected_rows != pivot_cols.len() {
        *m = BitMatrix::build(pivot_cols.len(), m.cols(), |i, j| m[(i, j)]);
    }
    pivot_cols
}

pub mod ac3;
pub mod finder;
pub mod number_theory;
pub mod orderexps;
pub mod possible_orders;
pub mod puzzle;
pub mod trie;

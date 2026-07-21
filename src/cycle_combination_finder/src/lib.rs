#![warn(clippy::pedantic)]
#![allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::similar_names
)]
#![feature(
    once_cell_try,
    mpmc_channel,
    slice_swap_unchecked,
    portable_simd,
    gen_blocks,
    clone_from_ref,
    split_array
)]

use bitgauss::BitMatrix;

pub const P9: u16 = FIRST_65_PRIMES[8];
pub const P17: u16 = FIRST_65_PRIMES[16];
pub const P33: u16 = FIRST_65_PRIMES[32];
pub const P65: u16 = FIRST_65_PRIMES[64];

/// Non-trivial to increase; make sure to look at `cycle_combination_details`
pub const FIRST_65_PRIMES: [u16; 65] = [
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193,
    197, 199, 211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 269, 271, 277, 281, 283, 293, 307,
    311, 313,
];

fn gauss_jordan_without_zero_rows(m: &mut BitMatrix, expected_rows: usize) -> Vec<usize> {
    let pivot_cols = m.gauss(true);
    if expected_rows != pivot_cols.len() {
        *m = BitMatrix::build(pivot_cols.len(), m.cols(), |i, j| m[(i, j)]);
    }
    pivot_cols
}

pub mod ac3;
pub mod cycle_combination_details;
pub mod cycle_combinations_tree;
pub mod finder;
pub mod min_piece_count;
pub mod nonemptyvec;
pub mod number_theory;
pub mod orderexps;
pub mod pareto_front;
pub mod possible_orders;
pub mod puzzle;
pub mod trie;

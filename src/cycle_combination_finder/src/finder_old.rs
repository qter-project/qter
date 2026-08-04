use std::{num::NonZeroU16, time::Instant};

use humanize_duration::{Truncate, prelude::DurationExt};
use log::info;
use puzzle_theory::numbers::{self, Int, U};

use crate::{
    number_theory::{MaxPrimePower, max_prime_powers_below},
    puzzle::PuzzleDef,
};

struct OrderIteration {
    index: usize,
    piece_count: u16,
    product: Int<U>,
    prime_powers: Vec<u16>,
    min_piece_count: Vec<u16>,
}

struct ComboIteration {
    register: usize,
    power: usize,
    orbit_sums: Vec<u16>,
    assignments: Vec<Assignment>,
    available_pieces: u16,
}

type Assignment = Vec<Vec<u16>>;

#[derive(Clone, Debug)]
struct PossibleOrder {
    // this is a candidate order
    order: Int<U>,
    prime_powers: Vec<u16>,
    min_piece_counts: Vec<u16>,
}

#[derive(Debug)]
struct Partition(Vec<u16>);

#[derive(Debug)]
pub struct Cycle {
    order: Int<U>,
    partitions: Vec<Partition>,
}

#[derive(Debug)]
pub struct CycleCombination {
    order_product: Int<U>,
    cycles: Vec<Cycle>,
    shared_pieces: Vec<u16>,
}

// ---------------

#[derive(Clone, Copy)]
pub enum Optimality {
    Equivalent,
    Optimal,
}

#[derive(Clone, Copy)]
pub enum RegisterCount {
    Exactly(NonZeroU16),
    All,
}

pub struct CycleCombinationFinder<const N: usize> {
    puzzle_def: PuzzleDef<N>,
}

impl Cycle {
    #[must_use]
    pub fn order(&self) -> Int<U> {
        self.order
    }
}

impl CycleCombination {
    #[must_use]
    pub fn cycles(&self) -> &[Cycle] {
        &self.cycles
    }
}

impl<const N: usize> From<PuzzleDef<N>> for CycleCombinationFinder<N> {
    fn from(puzzle_def: PuzzleDef<N>) -> Self {
        Self { puzzle_def }
    }
}

impl<const N: usize> CycleCombinationFinder<N> {
    fn find_equivalent(&self, register_count: RegisterCount) -> Vec<CycleCombination> {
        let RegisterCount::Exactly(register_count) = register_count else {
            panic!("expected exactly variant for now");
        };
        let register_count = register_count.get();
        // this is the main function. it returns a 'near optimal' combination such that
        // all registers have equivalent order it may not be the most
        // optimal, since there are some assumptions made to help efficiency

        // get number of pieces in each orbit. if the orbit pieces can orient, set a
        // shared piece aside to allow free orientation.
        let total_pieces = self
            .puzzle_def
            .orbit_defs()
            .iter()
            .fold(0, |sum, &orbit_def| {
                sum + orbit_def.piece_count.get()
                    - if orbit_def.orientation_count() > 1 {
                        1
                    } else {
                        0
                    }
            });

        let pieces_per_register = total_pieces / register_count;

        let partition_max = self
            .puzzle_def
            .orbit_defs()
            .iter()
            .map(|orbit_def| orbit_def.piece_count.get())
            .max()
            .unwrap()
            .min(pieces_per_register);

        // get list of prime powers that fit within the largest partition
        let max_prime_powers = max_prime_powers_below(self.puzzle_def.orbit_defs(), partition_max);

        // get a list of all orders that would fit within a pieces_per_register amount
        // of pieces
        let possible_orders: Vec<PossibleOrder> =
            self.possible_order_list(pieces_per_register, &max_prime_powers);

        // check the possible orders, descending, until one is found that fits
        for possible_order in possible_orders {
            // debug!("Testing Order {}", possible_order.order);

            // by default, prime_combo.piece_counts assumes all orientation efficiencies can
            // be made here we check if they can actually fit, or if
            // they must be handled by non-orienting pieces
            let mut unorientable_excess: u16 = 0;
            for (p, &prime_power) in possible_order.prime_powers.iter().enumerate() {
                if prime_power % 2 == 0 {
                    // find the amount of registers that can't be oriented
                    let orientable_registers = (self
                        .puzzle_def
                        .orbit_defs()
                        .iter()
                        .find_map(|&orbit_def| {
                            if orbit_def.orientation_count() == 2 {
                                Some(orbit_def.piece_count.get())
                            } else {
                                None
                            }
                        })
                        .unwrap()
                        / 1.max(possible_order.min_piece_counts[p]))
                    .min(register_count);
                    // each unorientable register will use 'value' pieces instead of
                    // 'prime_combo.piece_counts[v]' pieces
                    // so we need to account for that difference
                    unorientable_excess += (register_count - orientable_registers)
                        * (prime_power - possible_order.min_piece_counts[p]);
                } else if prime_power % 3 == 0 {
                    let orientable_registers = (self
                        .puzzle_def
                        .orbit_defs()
                        .iter()
                        .find_map(|&orbit_def| {
                            if orbit_def.orientation_count() == 3 {
                                Some(orbit_def.piece_count.get())
                            } else {
                                None
                            }
                        })
                        .unwrap()
                        / 1.max(possible_order.min_piece_counts[p]))
                    .min(register_count);
                    unorientable_excess += (register_count - orientable_registers)
                        * (prime_power - possible_order.min_piece_counts[p]);
                }
            }

            let available_pieces = total_pieces
                - register_count * (possible_order.min_piece_counts.iter().sum::<u16>())
                + 2;
            // if the excess exceeds the total number of pieces, the order won't fit so we
            // skip to the next
            if unorientable_excess > available_pieces {
                continue;
            }

            let registers = vec![possible_order.clone(); register_count as usize];
            let shared_pieces: Vec<u16> = vec![0, 0, 1, 1];
            if let Some(mut assignments) =
                self.possible_order_test(&registers, available_pieces, &shared_pieces)
            {
                return vec![self.assignments_to_combo(
                    &mut assignments,
                    &registers,
                    &shared_pieces,
                )];
            }
        }

        vec![]
    }
}

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
    /// given some order, test if it will fit on the puzzle
    fn possible_order_test(
        &self,
        registers: &[PossibleOrder],
        available_pieces: u16,
        shared_pieces: &[u16],
    ) -> Option<Vec<Assignment>> {
        let mut shared_sum = 0;
        for &orbit_def in self.puzzle_def.orbit_defs() {
            shared_sum += shared_pieces[orbit_def.orientation_count() as usize];
        }
        if shared_sum > available_pieces {
            return None;
        }
        let parity_covered = shared_pieces[2] == 2 || shared_pieces[3] == 2;

        // create a stack to recursively add cycles for prime powers from each register
        let mut stack: Vec<ComboIteration> = vec![ComboIteration {
            register: 0, // current register to add
            power: registers[0].prime_powers.len(), /* current prime power index
                          * to add (reversed) */
            orbit_sums: vec![0; self.puzzle_def.orbit_defs().len()], // pieces used in each orbit
            assignments: vec![vec![vec![]; self.puzzle_def.orbit_defs().len()]; registers.len()],
            available_pieces: available_pieces - shared_sum, // extra pieces beyond the minimum
        }];

        let mut loops: u16 = 0;
        while let Some(mut s) = stack.pop() {
            loops += 1;
            if loops > 1000 {
                return None; // a fit is usually found quickly, so quit if the search takes a while
            }

            let mut seen = vec![]; // this is used to detect duplicates

            // if we've added the last prime power for this register, move to the next
            // register
            if s.power == 0 {
                s.register += 1;
                // if that was the last register, we found a fit! return it.
                if s.register == registers.len() {
                    return Some(s.assignments);
                }
                s.power = registers[s.register].prime_powers.len() - 1;
            } else {
                s.power -= 1;
            }

            // try adding the current prime power to each orbit
            for (o, &orbit_def) in self.puzzle_def.orbit_defs().iter().enumerate() {
                let orbit_orient = orbit_def.orientation_count();

                // orbits with no orientation and the same piece count act the same. we should
                // only check the first one continue if this is a duplicate of
                // an orbit that was already checked.
                if orbit_orient == 1 {
                    if seen.contains(&orbit_def.piece_count) {
                        continue;
                    }
                    seen.push(orbit_def.piece_count);
                }

                let mut new_cycle: u16;
                let new_available: u16;
                // if this orbit orients using the same prime as the power, add a cycle
                if orbit_orient > 1
                    && registers[s.register].prime_powers[s.power]
                        .is_multiple_of(orbit_orient.into())
                {
                    let flippers = s.assignments[s.register][o].len() as u16
                        + shared_pieces[orbit_orient as usize].min(1);
                    new_cycle = registers[s.register].min_piece_counts[s.power];

                    //TODO allow for 2 corners to twist
                    let excess = if new_cycle == 0 {
                        if flippers == 1 {
                            1
                        } else if flippers == 0 {
                            2
                        } else {
                            0
                        }
                    } else if flippers == 0 {
                        1
                    } else {
                        0
                    };

                    if s.available_pieces < excess {
                        continue;
                    }
                    new_cycle += excess;
                    new_available = s.available_pieces - excess;
                } else if registers[s.register].prime_powers[s.power] == 1 {
                    new_cycle = 0;
                    new_available = s.available_pieces;
                }
                // otherwise, we get no orientation multiplier, so the cycle will use the same
                // number of pieces as the power itself if there are enough
                // available pieces to make this happen, add a cycle
                else if registers[s.register].prime_powers[s.power]
                    - registers[s.register].min_piece_counts[s.power]
                    <= s.available_pieces
                {
                    new_cycle = registers[s.register].prime_powers[s.power];
                    new_available = s.available_pieces
                        + registers[s.register].min_piece_counts[s.power]
                        - registers[s.register].prime_powers[s.power];
                }
                // but if there are not enough available, continue
                else {
                    continue;
                }

                /*
                // we assume 0 min_pieces for a prime cycle if there is an orbit with that prime as its orient_count
                // but that requires the orbit to have a cycle of length of a different prime
                // if there is no cycle in this register, we need to use a piece for this.
                if new_cycle == 0 && s.assignments[s.register][o].is_empty() {
                    if s.available_pieces == 0 {
                        continue;
                    }
                    new_cycle = 1;
                    new_available -= 1;
                }*/

                // assume that every even cycle needs a parity to go with it. TODO could be more
                // efficient to share parity.
                let parity: u16 = if new_cycle.is_multiple_of(2) && new_cycle > 0 && !parity_covered
                {
                    2
                } else {
                    0
                };
                if parity > new_available {
                    continue;
                }

                // if there is room for the new cycle in this orbit, add it and push to stack
                if new_cycle + parity + s.orbit_sums[o] + shared_pieces[orbit_orient as usize]
                    <= orbit_def.piece_count.get()
                {
                    let mut combo_iteraton = ComboIteration {
                        register: s.register,
                        power: s.power,
                        orbit_sums: s.orbit_sums.clone(),
                        assignments: s.assignments.clone(),
                        available_pieces: new_available - parity,
                    };

                    if new_cycle > 0 {
                        combo_iteraton.orbit_sums[o] += new_cycle;
                        combo_iteraton.assignments[s.register][o].push(new_cycle);
                        if parity > 0 {
                            combo_iteraton.orbit_sums[o] += 2;
                            combo_iteraton.assignments[s.register][o].push(2);
                        }
                    }

                    stack.push(combo_iteraton);
                }
            }
        }

        None
    }

    /// once an order is found that fits on the cube, process into an output
    /// format
    fn assignments_to_combo(
        &self,
        assignments: &mut [Vec<Vec<u16>>],
        registers: &[PossibleOrder],
        shared_pieces: &[u16],
    ) -> CycleCombination {
        let mut cycle_combination: Vec<Cycle> = vec![];

        for (r, register) in registers.iter().rev().enumerate() {
            let mut partitions: Vec<Partition> = vec![];

            for (o, &orbit_def) in self.puzzle_def.orbit_defs().iter().enumerate() {
                let mut lcm: Int<U> = Int::<U>::from(1_u16);
                for &a in &assignments[registers.len() - 1 - r][o] {
                    lcm = numbers::lcm(lcm, Int::<U>::from(a));
                }

                if orbit_def.orientation_count() > 1 {
                    lcm *= Int::<U>::from(orbit_def.orientation_count());
                    //assignments[r][o].push(1);
                }

                // debug!(
                //     "{register:#?}\n{orbit_def:#?}\n{:?}",
                //     assignments[registers.len() - 1 - r][o]
                // );

                partitions.push(Partition(assignments[registers.len() - 1 - r][o].clone()));
            }

            cycle_combination.push(Cycle {
                order: register.order,
                partitions,
            });
        }

        let order_product = registers.iter().map(|v| v.order).product();

        CycleCombination {
            order_product,
            cycles: cycle_combination,
            shared_pieces: shared_pieces.to_vec(),
        }
    }

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

    fn find_optimal(&self, register_count: RegisterCount) -> Vec<CycleCombination> {
        let RegisterCount::Exactly(register_count) = register_count else {
            panic!("expected exactly variant for now");
        };
        let register_count = register_count.get();

        let total_pieces = self
            .puzzle_def
            .orbit_defs()
            .iter()
            .fold(0, |sum, &orbit_def| sum + orbit_def.piece_count.get());

        let partition_max = self
            .puzzle_def
            .orbit_defs()
            .iter()
            .map(|orbit_def| orbit_def.piece_count.get())
            .max()
            .unwrap();

        // get list of prime powers that fit within the largest partition
        let max_prime_powers = max_prime_powers_below(self.puzzle_def.orbit_defs(), partition_max);

        // get a list of all orders that would fit within a pieces_per_register amount
        // of pieces
        let possible_orders = self.possible_order_list(total_pieces, &max_prime_powers);
        // for possible_order in &possible_orders {
        //     println!("{:?}", u64::try_from(possible_order.order).unwrap());
        // }

        // debug!("Possible Orders: {possible_orders:?}");

        let mut cycle_combinations: Vec<CycleCombination> = vec![];
        let shared_piece_options: Vec<Vec<u16>> = vec![
            vec![0, 0, 0, 0],
            vec![0, 0, 0, 1],
            vec![0, 0, 0, 2],
            vec![0, 0, 1, 0],
            vec![0, 0, 1, 1],
            vec![0, 0, 1, 2],
            vec![0, 0, 2, 0],
            vec![0, 0, 2, 1],
        ];

        self.add_order_to_registers(
            register_count,
            &[],
            &possible_orders,
            total_pieces,
            &mut cycle_combinations,
            &shared_piece_options,
        );

        cycle_combinations
    }
}

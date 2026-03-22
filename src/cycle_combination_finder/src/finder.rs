// TOOD: nonzero for everything

use std::{fmt, num::NonZeroU16, time::Instant};

use humanize_duration::{Truncate, prelude::DurationExt};
use log::{debug, info};
use puzzle_theory::{
    ksolve::{KSolve, KSolveSet},
    numbers::{self, Int, U},
};

use crate::puzzle::{OrbitDef, PuzzleDef};

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

#[derive(Clone)]
struct PossibleOrder {
    // this is a candidate order
    order: Int<U>,
    prime_powers: Vec<u16>,
    min_piece_counts: Vec<u16>,
}

impl fmt::Debug for PossibleOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        //write!(f,"order {}, prime powers {:?}", self.order, self.prime_powers)
        write!(f, "{}, {:?}", self.order, self.prime_powers)
    }
}

struct Partition(Vec<u16>);

pub struct Cycle {
    order: Int<U>,
    partitions: Vec<Partition>,
}

impl fmt::Debug for Cycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        //write!(f, "{}, {:?}", self.order, self.partitions)
        write!(f, "{}", self.order)
    }
}

#[derive(Debug)]
pub struct CycleCombination {
    order_product: Int<U>,
    cycles: Vec<Cycle>,
    shared_pieces: Vec<u16>,
}

// ---------------

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MaxPrimePower {
    pub prime: u16,
    pub exponent: u16,
}

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

pub struct CycleCombinationFinder {
    puzzle_def: PuzzleDef,
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

impl From<PuzzleDef> for CycleCombinationFinder {
    fn from(puzzle_def: PuzzleDef) -> Self {
        Self { puzzle_def }
    }
}

impl CycleCombinationFinder {
    // pub fn from_ksolve(ksolve: &KSolve) -> Option<Self> {
    //     let orbit_defs =
    // ksolve.sets().iter().map(OrbitDef::from).collect::<Vec<_>>();
    //     Self::from_puzzle(orbit_defs)
    // }

    /// return a 2D list of prime powers below n. The first index is the prime,
    /// the second is the power of that prime Return all
    pub fn max_prime_powers_below(&self, n: u16) -> Vec<MaxPrimePower> {
        #[derive(Copy, Clone, Debug, PartialEq)]
        enum SieveNumberState {
            Prime,
            Other,
        }

        let n = usize::from(n);

        let mut sieve = vec![SieveNumberState::Prime; n + 1];
        sieve[0] = SieveNumberState::Other;
        if let Some(v) = sieve.get_mut(1) {
            *v = SieveNumberState::Other;
        }

        for i in 2..=n.isqrt() {
            if sieve[i] != SieveNumberState::Prime {
                continue;
            }
            let prime = i;

            for multiple in (prime * prime..=n).step_by(prime) {
                sieve[multiple] = SieveNumberState::Other;
            }
        }

        let mut max_prime_powers = vec![];
        for (i, &state) in sieve.iter().enumerate().take(n + 1).skip(2) {
            if state != SieveNumberState::Prime {
                continue;
            }
            let prime = i;

            let mut exponent = 1;
            let mut min_piece_count = prime;
            loop {
                let next = min_piece_count * prime;
                if next > n {
                    break;
                }
                min_piece_count = next;
                exponent += 1;
            }
            if self
                .puzzle_def
                .orbit_defs()
                .iter()
                .find(|&&orbit_def| orbit_def.orientation_count.get() as usize == prime)
                .is_some_and(|&orbit_def| min_piece_count as u16 <= orbit_def.piece_count.get())
            {
                exponent += 1;
            }

            max_prime_powers.push(MaxPrimePower {
                prime: prime as u16,
                exponent,
            });
        }
        max_prime_powers.sort_by(|a, b| a.prime.cmp(&b.prime));
        max_prime_powers
    }

    /// get a list of all possible orders to fit within a given number of pieces
    /// and partitions
    fn possible_order_list(
        &self,
        total_pieces: u16,
        max_prime_powers: &[MaxPrimePower],
    ) -> Vec<PossibleOrder> {
        debug!("{total_pieces:?} {max_prime_powers:?}");
        let mut paths = vec![];
        let mut log_path = |s: OrderIteration| {
            let prime_powers = if s.product == Int::<U>::from(1_u16) {
                vec![1]
            } else {
                s.prime_powers.clone()
            };
            let min_piece_counts = if s.product == Int::<U>::from(1_u16) {
                vec![0]
            } else {
                s.min_piece_count.clone()
            };

            paths.push(PossibleOrder {
                order: s.product,
                prime_powers,
                min_piece_counts,
            });
        };
        let mut stack = vec![OrderIteration {
            index: 0,
            piece_count: 0,
            product: Int::<U>::from(1_u16),
            prime_powers: vec![],
            min_piece_count: vec![],
        }];

        // loop through the prime powers, taking all combinations that will fit within
        // total_pieces
        while let Some(s) = stack.pop() {
            // if all primes have been added, log this order
            let Some(&max_prime_power) = max_prime_powers.get(s.index) else {
                log_path(s);
                continue;
            };

            let maybe_orbit_def = self
                .puzzle_def
                .orbit_defs()
                .iter()
                .copied()
                .find(|&orbit_def| {
                    orbit_def.orientation_count.get() as u16 == max_prime_power.prime
                });
            let min_piece_count = if maybe_orbit_def.is_some() {
                0
            } else {
                max_prime_power.prime
            };

            // if there's no room for the next prime, log this order
            if min_piece_count + s.piece_count > total_pieces {
                log_path(s);
                continue;
            }

            // try adding all powers of the current prime
            let mut prime_power_iter = 1u16;
            for i in 0..=max_prime_power.exponent {
                let min_piece_count = if i == 0 {
                    0
                } else if let Some(orbit_def) = maybe_orbit_def {
                    if i == 1 {
                        0
                    } else {
                        // TODO You're dividing the order that you want by the maximum orientation
                        // improvement
                        let ideal = prime_power_iter
                            // .checked_exact_div(max_prime_power.prime.pow(exponent_value_from_orbit))
                            .checked_exact_div(max_prime_power.prime)
                            .unwrap();
                        // if the power exceeds the size of orientable orbit, remove the multiplier
                        if ideal > orbit_def.piece_count.get() {
                            prime_power_iter
                        } else {
                            ideal
                        }
                    }
                } else {
                    prime_power_iter
                };
                // the new piece count will add min_pieces for the current power, plus two if
                // parity needs handling
                let new_piece_count = s.piece_count
                    + min_piece_count
                    + if min_piece_count > 0 && min_piece_count.is_multiple_of(2) {
                        2
                    } else {
                        0
                    }; // TODO this should not happen on 4x4

                // if the new prime power fits on the puzzle, add to the stack
                if new_piece_count <= total_pieces {
                    let mut order_iteraton = OrderIteration {
                        index: s.index + 1,
                        piece_count: new_piece_count,
                        product: s.product,
                        prime_powers: s.prime_powers.clone(),
                        min_piece_count: s.min_piece_count.clone(),
                    };

                    if prime_power_iter > 1 {
                        order_iteraton.product *= Int::<U>::from(prime_power_iter);
                        order_iteraton.prime_powers.push(prime_power_iter);
                        order_iteraton.min_piece_count.push(min_piece_count);
                    }
                    stack.push(order_iteraton);
                }
                if i != max_prime_power.exponent {
                    prime_power_iter *= max_prime_power.prime;
                }
            }
        }

        paths
            .sort_by(|a: &PossibleOrder, b: &PossibleOrder| b.order.partial_cmp(&a.order).unwrap());

        debug!("{paths:#?}");
        // println!("{:?}", paths.len());
        paths
    }

    /// given some order, test if it will fit on the puzzle
    fn possible_order_test(
        &self,
        registers: &[PossibleOrder],
        available_pieces: u16,
        shared_pieces: &[u16],
    ) -> Option<Vec<Assignment>> {
        let mut shared_sum = 0;
        for &orbit_def in self.puzzle_def.orbit_defs() {
            shared_sum += shared_pieces[orbit_def.orientation_count.get() as usize];
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
                let orbit_orient = orbit_def.orientation_count.get();

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

                if orbit_def.orientation_count.get() > 1 {
                    lcm *= Int::<U>::from(orbit_def.orientation_count.get());
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

    fn add_order_to_registers(
        &self,
        num_registers: u16,
        registers: &[PossibleOrder],
        possible_orders: &[PossibleOrder],
        available_pieces: u16,
        cycle_combinations: &mut Vec<CycleCombination>,
        shared_piece_options: &Vec<Vec<u16>>,
    ) {
        let last_reg = registers.len() as i32 - 1;
        let last_order: Int<U> = if last_reg == -1 {
            possible_orders[0].order
        } else {
            registers[0].order
        };

        // debug!("new\n{cycle_combinations:#?}");
        let mut max_redundant = Int::<U>::from(0_u16);
        for combo in &*cycle_combinations {
            for reg_from_last in 0..registers.len() {
                if registers[last_reg as usize - reg_from_last].order
                    > combo.cycles[reg_from_last].order
                {
                    break;
                }

                max_redundant = combo.cycles[(num_registers - 1) as usize]
                    .order
                    .max(max_redundant);
            }
        }

        for possible_order in possible_orders {
            //debug!("possible_order At {:?}, {}", possible_order, last_order);
            if possible_order.order <= max_redundant {
                return;
            }

            if possible_order.min_piece_counts.iter().sum::<u16>() > available_pieces
                || possible_order.order > last_order
            {
                continue;
            }

            let mut registers_with_new: Vec<PossibleOrder> = vec![possible_order.clone()];
            registers_with_new.extend(registers.iter().cloned());

            if (last_reg + 2) as u16 == num_registers {
                for shared_pieces in shared_piece_options {
                    if let Some(mut assignments) = self.possible_order_test(
                        &registers_with_new,
                        available_pieces,
                        shared_pieces,
                    ) {
                        cycle_combinations.push(self.assignments_to_combo(
                            &mut assignments,
                            &registers_with_new,
                            shared_pieces,
                        ));
                        return;
                    }
                }
            } else {
                self.add_order_to_registers(
                    num_registers,
                    &registers_with_new,
                    possible_orders,
                    available_pieces - possible_order.min_piece_counts.iter().sum::<u16>(),
                    cycle_combinations,
                    shared_piece_options,
                );
            }
        }
    }

    #[must_use]
    pub fn find(
        &self,
        optimality: Optimality,
        register_count: RegisterCount,
    ) -> Vec<CycleCombination> {
        let now = Instant::now();

        let RegisterCount::Exactly(register_count) = register_count else {
            panic!("expected exactly variant for now");
        };
        let register_count = register_count.get();

        let ret = match optimality {
            Optimality::Equivalent => {
                // this is the main function. it returns a 'near optimal' combination such that
                // all registers have equivalent order it may not be the most
                // optimal, since there are some assumptions made to help efficiency

                // get number of pieces in each orbit. if the orbit pieces can orient, set a
                // shared piece aside to allow free orientation.
                let total_pieces =
                    self.puzzle_def
                        .orbit_defs()
                        .iter()
                        .fold(0, |sum, &orbit_def| {
                            sum + orbit_def.piece_count.get()
                                - if orbit_def.orientation_count.get() > 1 {
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
                let max_prime_powers = self.max_prime_powers_below(partition_max);

                // get a list of all orders that would fit within a pieces_per_register amount
                // of pieces
                let possible_orders: Vec<PossibleOrder> =
                    self.possible_order_list(pieces_per_register, &max_prime_powers);

                // check the possible orders, descending, until one is found that fits
                for possible_order in possible_orders {
                    debug!("Testing Order {}", possible_order.order);

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
                                    if orbit_def.orientation_count.get() == 2 {
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
                                    if orbit_def.orientation_count.get() == 3 {
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
            Optimality::Optimal => {
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
                    // TODO enforce length is non zero
                    .unwrap();

                let now2 = Instant::now();
                // get list of prime powers that fit within the largest partition
                let max_prime_powers = self.max_prime_powers_below(partition_max);

                // get a list of all orders that would fit within a pieces_per_register amount
                // of pieces
                let possible_orders = self.possible_order_list(total_pieces, &max_prime_powers);
                println!("{}", now2.elapsed().human(Truncate::Millis));
                println!("a {:?}", possible_orders.len());

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
        };
        info!("Finished in {}", now.elapsed().human(Truncate::Millis));
        ret
    }
}

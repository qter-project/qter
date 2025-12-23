#![allow(unused)]
use std::fmt;

use puzzle_theory::{ksolve::KSolveSet, numbers::{self, Int, U}, puzzle_geometry::{num, parsing::puzzle}};

struct PrimePower {
    value: u16,
    min_pieces: u16,
}
struct OrderIteration {
    index: usize,
    piece_count: u16,
    product: Int<U>,
    powers: Vec<u16>,
    min_pieces: Vec<u16>,
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

struct Partition {
    name: String,
    partition: Vec<u16>,
    order: Int<U>,
}

impl fmt::Debug for Partition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.partition)
    }
}

struct Cycle {
    order: Int<U>,
    partitions: Vec<Partition>,
}

impl fmt::Debug for Cycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        //write!(f, "{}, {:?}", self.order, self.partitions)
        write!(f, "{}", self.order)
    }
}

struct CycleCombination {
    used_cubie_counts: Vec<u16>,
    order_product: Int<U>,
    cycles: Vec<Cycle>,
    shared_pieces: Vec<u16>,
}

/// return a 2D list of prime powers below n. The first index is the prime, the second is the power of that prime
fn prime_powers_below_n(n: u16, orientable_pieces: &[u16]) -> Vec<Vec<PrimePower>> {
    let mut primes: Vec<u16> = vec![2];

    // find all primes below n
    for possible_prime in (3..=n).step_by(2) {
        let mut is_prime = true;
        for p in &primes {
            if p * p > possible_prime {
                break;
            }

            if possible_prime % p == 0 {
                is_prime = false;
                break;
            }
        }

        if !is_prime {
            continue;
        }

        primes.push(possible_prime);
    }

    let mut prime_powers = vec![];

    //for each prime, find all of its powers and the minimum pieces needed
    for (p, &prime) in primes.iter().enumerate() {
        let mut orient_multiplier: u16 = 1;
        let mut piece_check;
        // handle if there is an orbit with an orient_count of the current prime
        if orientable_pieces.len() > prime as usize && orientable_pieces[prime as usize] > 0 {
            orient_multiplier = prime;
            prime_powers.push(vec![
                PrimePower {
                    value: 1,
                    min_pieces: 0, // excluding this prime uses no pieces
                },
                PrimePower {
                    value: prime,
                    min_pieces: 0, // the minimum pieces for this prime is 0 since we can use a cycle of different prime length and orient
                },
            ]);
            piece_check = prime;
        } else {
            prime_powers.push(vec![
                PrimePower {
                    value: 1,
                    min_pieces: 0,
                },
                PrimePower {
                    value: prime,
                    min_pieces: prime,
                },
            ]);
            piece_check = prime.pow(2);
        }

        // increase powers of this prime until one doesn't fit
        while piece_check <= n {
            prime_powers[p].push(PrimePower {
                value: orient_multiplier * piece_check,
                min_pieces: piece_check,
            });
            piece_check *= prime;

            // if the power exceeds the size of orientable orbit, remove the multiplier
            if orient_multiplier > 1 && piece_check > orientable_pieces[prime as usize] {
                piece_check *= orient_multiplier;
                orient_multiplier = 1;
            }
        }
    }

    prime_powers
}

/// get a list of all possible orders to fit within a given number of pieces and partitions
fn possible_order_list(
    total_pieces: u16,
    partition_max: u16,
    orientable_pieces: &[u16],
) -> Vec<PossibleOrder> {
    // get list of prime powers that fit within the largest partition
    let prime_powers = prime_powers_below_n(partition_max, orientable_pieces);

    let mut paths = vec![];
    // create a stack to handle recursive
    let mut stack: Vec<OrderIteration> = vec![OrderIteration {
        index: 0,
        piece_count: 0,
        product: Int::<U>::from(1_u16),
        powers: vec![],
        min_pieces: vec![],
    }];

    // loop through the prime powers, taking all combinations that will fit within total_pieces
    while let Some(s) = stack.pop() {
        // if all primes have been added or there's no room for the next prime, log this order
        if s.index == prime_powers.len()
            || prime_powers[s.index][1].min_pieces + s.piece_count > total_pieces
        {
            let prime_powers = if s.product == Int::<U>::from(1_u16) {
                vec![1]
            } else {
                s.powers.clone()
            };
            let min_piece_counts = if s.product == Int::<U>::from(1_u16) {
                vec![0]
            } else {
                s.min_pieces.clone()
            };

            paths.push(PossibleOrder {
                order: s.product,
                prime_powers,
                min_piece_counts,
            });
            continue;
        }

        // try adding all powers of the current prime
        for p in &prime_powers[s.index] {
            // the new piece count will add min_pieces for the current power, plus two if parity needs handling
            let new_piece_count = s.piece_count
                + p.min_pieces
                + if p.min_pieces > 0 && p.min_pieces % 2 == 0 {
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
                    powers: s.powers.clone(),
                    min_pieces: s.min_pieces.clone(),
                };

                if p.value > 1 {
                    order_iteraton.product *= Int::<U>::from(p.value);
                    order_iteraton.powers.push(p.value);
                    order_iteraton.min_pieces.push(p.min_pieces);
                }
                stack.push(order_iteraton);
            }
        }
    }

    paths.sort_by(|a: &PossibleOrder, b: &PossibleOrder| b.order.partial_cmp(&a.order).unwrap());

    paths
}

/// given some order, test if it will fit on the puzzle
fn possible_order_test(
    registers: &[PossibleOrder],
    cycle_cubie_counts: &[u16],
    puzzle: &[KSolveSet],
    available_pieces: u16,
    shared_pieces: &Vec<u16>,
) -> Option<Vec<Assignment>> {
    let mut shared_sum = 0;
    for orbit in puzzle {
        shared_sum += shared_pieces[orbit.orientation_count().get() as usize];
    }
    if shared_sum > available_pieces {
        return None;
    }
    let parity_covered = shared_pieces[2] == 2 || shared_pieces[3] == 2;

    // create a stack to recursively add cycles for prime powers from each register
    let mut stack: Vec<ComboIteration> = vec![ComboIteration {
        register: 0,                                   // current register to add
        power: registers[0].prime_powers.len(), // current prime power index to add (reversed)
        orbit_sums: vec![0; cycle_cubie_counts.len()], // pieces used in each orbit
        assignments: vec![vec![vec![]; cycle_cubie_counts.len()]; registers.len()],
        available_pieces: available_pieces - shared_sum, // extra pieces beyond the minimum
    }];

    let mut loops: u16 = 0;
    while let Some(mut s) = stack.pop() {
        loops += 1;
        if loops > 1000 {
            return None; // a fit is usually found quickly, so quit if the search takes a while
        }

        let mut seen = vec![]; // this is used to detect duplicates

        // if we've added the last prime power for this register, move to the next register
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
        for (o, orbit) in puzzle.iter().enumerate() {
            let orbit_orient = orbit.orientation_count().get() as u16;

            // orbits with no orientation and the same piece count act the same. we should only check the first one
            // continue if this is a duplicate of an orbit that was already checked.
            if orbit_orient == 1 {
                if seen.contains(&cycle_cubie_counts[o]) {
                    continue;
                }
                seen.push(cycle_cubie_counts[o]);
            }

            let mut new_cycle: u16;
            let new_available: u16;
            // if this orbit orients using the same prime as the power, add a cycle
            if orbit_orient > 1
                && registers[s.register].prime_powers[s.power].is_multiple_of(orbit_orient)
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
            // otherwise, we get no orientation multiplier, so the cycle will use the same number of pieces as the power itself
            // if there are enough available pieces to make this happen, add a cycle
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

            // assume that every even cycle needs a parity to go with it. TODO could be more efficient to share parity.
            let parity: u16 = if new_cycle.is_multiple_of(2) && new_cycle > 0 && !parity_covered {
                2
            } else {
                0
            };
            if parity > new_available {
                continue;
            }

            // if there is room for the new cycle in this orbit, add it and push to stack
            if new_cycle + parity + s.orbit_sums[o] + shared_pieces[orbit_orient as usize]
                <= cycle_cubie_counts[o]
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

/// once an order is found that fits on the cube, process into an output format
fn assignments_to_combo(
    assignments: &mut [Vec<Vec<u16>>],
    registers: &[PossibleOrder],
    cycle_cubie_counts: &[u16],
    puzzle: &[KSolveSet],
    shared_pieces: &Vec<u16>,
) -> CycleCombination {
    let mut cycle_combination: Vec<Cycle> = vec![];

    for (r, register) in registers.iter().rev().enumerate() {
        let mut partitions: Vec<Partition> = vec![];

        for (o, orbit) in puzzle.iter().enumerate() {
            let mut lcm: Int<U> = Int::<U>::from(1_u16);
            for &a in &assignments[registers.len() - 1 - r][o] {
                lcm = numbers::lcm(lcm, Int::<U>::from(a));
            }

            if orbit.orientation_count().get() > 1 {
                lcm *= Int::<U>::from(orbit.orientation_count().get());
                //assignments[r][o].push(1);
            }

            partitions.push(Partition {
                name: orbit.name().to_string(),
                partition: assignments[registers.len() - 1 - r][o].clone(),
                order: lcm,
            });
        }

        cycle_combination.push(Cycle {
            order: register.order,
            partitions,
        });
    }

    let order_product = registers.iter().map(|v| v.order).product();

    CycleCombination {
        used_cubie_counts: cycle_cubie_counts.to_vec(),
        order_product,
        cycles: cycle_combination,
        shared_pieces: shared_pieces.clone(),
    }
}

/// this is the main function. it returns a 'near optimal' combination such that all registers have equivalent order
/// it may not be the most optimal, since there are some assumptions made to help efficiency
fn optimal_equivalent_combination(
    puzzle: &[KSolveSet],
    num_registers: u16,
) -> Option<CycleCombination> {
    let mut cycle_cubie_counts: Vec<u16> = vec![0; puzzle.len()]; //the count of pieces in each orbit
    let mut orientable_pieces: Vec<u16> = vec![0; 4]; // the kth index stores the number of pieces in an orbit with orient_count k
    let mut total_cubies: u16 = 0;
    // get number of pieces in each orbit. if the orbit pieces can orient, set a shared piece aside to allow free orientation.
    for (o, orbit) in puzzle.iter().enumerate() {
        let orientation_count = orbit.orientation_count().get();
        let piece_count = orbit.piece_count().get();
        if orientation_count > 1 {
            orientable_pieces[orientation_count as usize] = piece_count - 1;
            total_cubies += piece_count - 1;
        } else {
            total_cubies += piece_count;
        }
        cycle_cubie_counts[o] = piece_count
    }

    let cubies_per_register = total_cubies / num_registers;

    // get a list of all orders that would fit within a cubies_per_register amount of pieces
    let possible_orders: Vec<PossibleOrder> = possible_order_list(
        cubies_per_register,
        cycle_cubie_counts
            .iter()
            .max()
            .copied()
            .unwrap()
            .min(cubies_per_register),
        &orientable_pieces,
    );

    // check the possible orders, descending, until one is found that fits
    for possible_order in possible_orders {
        println!("Testing Order {}", possible_order.order);

        // by default, prime_combo.piece_counts assumes all orientation efficiencies can be made
        // here we check if they can actually fit, or if they must be handled by non-orienting pieces
        let mut unorientable_excess: u16 = 0;
        for (p, prime_power) in possible_order.prime_powers.iter().enumerate() {
            if prime_power % 2 == 0 {
                // find the amount of registers that can't be oriented
                let orientable_registers = (orientable_pieces[2]
                    / 1.max(possible_order.min_piece_counts[p]))
                .min(num_registers);
                // each unorientable register will use 'value' pieces instead of 'prime_combo.piece_counts[v]' pieces
                // so we need to account for that difference
                unorientable_excess += (num_registers - orientable_registers)
                    * (prime_power - possible_order.min_piece_counts[p]);
            } else if prime_power % 3 == 0 {
                let orientable_registers = (orientable_pieces[3]
                    / 1.max(possible_order.min_piece_counts[p]))
                .min(num_registers);
                unorientable_excess += (num_registers - orientable_registers)
                    * (prime_power - possible_order.min_piece_counts[p]);
            }
        }

        let available_pieces = total_cubies
            - num_registers * (possible_order.min_piece_counts.iter().sum::<u16>())
            + 2;
        // if the excess exceeds the total number of cubies, the order won't fit so we skip to the next
        if unorientable_excess > available_pieces {
            continue;
        }

        let registers = vec![possible_order.clone(); num_registers as usize];
        let shared_pieces: Vec<u16> = vec![0, 0, 1, 1];
        if let Some(mut assignments) = possible_order_test(
            &registers,
            &cycle_cubie_counts,
            puzzle,
            available_pieces,
            &shared_pieces,
        ) {
            return Some(assignments_to_combo(
                &mut assignments,
                &registers,
                &cycle_cubie_counts,
                puzzle,
                &shared_pieces,
            ));
        }
    }

    None
}

fn add_order_to_registers(
    num_registers: &u16,
    registers: Vec<PossibleOrder>,
    possible_orders: &[PossibleOrder],
    cycle_cubie_counts: &[u16],
    puzzle: &[KSolveSet],
    available_pieces: u16,
    cycle_combos: &mut Vec<CycleCombination>,
    shared_piece_options: &Vec<Vec<u16>>,
) {
    let last_reg = registers.len() as i32 - 1;
    let last_order: Int<U> = if last_reg == -1 {
        possible_orders[0].order
    } else {
        registers[0].order
    };

    let mut max_redundant = Int::<U>::from(0_u16);
    for combo in &mut *cycle_combos {
        let mut overshadows = true;
        for reg_from_last in 0..registers.len() {
            if registers[last_reg as usize - reg_from_last].order
                > combo.cycles[reg_from_last].order
            {
                overshadows = false;
                break;
            }

            if overshadows {
                max_redundant = combo.cycles[(*num_registers - 1) as usize]
                    .order
                    .max(max_redundant);
            }
        }
    }

    for possible_order in possible_orders {
        //println!("possible_order At {:?}, {}", possible_order, last_order);
        if possible_order.order <= max_redundant {
            return;
        }

        if possible_order.min_piece_counts.iter().sum::<u16>() > available_pieces
            || possible_order.order > last_order
        {
            continue;
        }

        let mut registers_with_new: Vec<PossibleOrder> = vec![possible_order.clone()];
        registers_with_new.extend(registers.clone());

        if (last_reg + 2) as u16 == *num_registers {
            for shared_pieces in shared_piece_options {
                if let Some(mut assignments) = possible_order_test(
                    &registers_with_new,
                    cycle_cubie_counts,
                    puzzle,
                    available_pieces,
                    shared_pieces,
                ) {
                    cycle_combos.push(assignments_to_combo(
                        &mut assignments,
                        &registers_with_new,
                        cycle_cubie_counts,
                        puzzle,
                        shared_pieces,
                    ));
                    return;
                }
            }
        } else {
            add_order_to_registers(
                num_registers,
                registers_with_new,
                possible_orders,
                cycle_cubie_counts,
                puzzle,
                available_pieces - possible_order.min_piece_counts.iter().sum::<u16>(),
                cycle_combos,
                shared_piece_options,
            );
        }
    }
}

// this is the main function. it returns all non-redundant combinations
fn optimal_combinations(puzzle: &[KSolveSet], num_registers: u16) {
    let mut cycle_cubie_counts: Vec<u16> = vec![0; puzzle.len()]; //the count of pieces in each orbit
    let mut orientable_pieces: Vec<u16> = vec![0; 4]; // the kth index stores the number of pieces in an orbit with orient_count k

    // get number of pieces in each orbit. if the orbit pieces can orient, set a shared piece aside to allow free orientation.
    for (o, orbit) in puzzle.iter().enumerate() {
        let orientation_count = orbit.orientation_count().get();
        let piece_count = orbit.piece_count().get();
        if orientation_count > 1 {
            orientable_pieces[orientation_count as usize] = piece_count;
        }
        cycle_cubie_counts[o] = piece_count;
    }

    let total_cubies: u16 = cycle_cubie_counts.iter().sum();

    // get a list of all orders that would fit within a cubies_per_register amount of pieces
    let possible_orders: Vec<PossibleOrder> = possible_order_list(
        total_cubies,
        cycle_cubie_counts.iter().max().copied().unwrap(),
        &orientable_pieces,
    );

    let mut cycle_combos: Vec<CycleCombination> = vec![];
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

    add_order_to_registers(
        &num_registers,
        vec![],
        &possible_orders,
        &cycle_cubie_counts,
        puzzle,
        cycle_cubie_counts.iter().sum(),
        &mut cycle_combos,
        &shared_piece_options,
    );

    for combo in cycle_combos {
        //println!("Found Combo {:?}, {:?}", combo.cycles, combo.shared_pieces);
        println!("Found Combo {:?}", combo.cycles);
    }
}

fn main() {
    let ksolve = puzzle("3x3").ksolve();
    let puzzle = ksolve.sets();
    let cycle_combos: Option<CycleCombination> = optimal_equivalent_combination(puzzle, 3);

    println!(
        "Highest Equivalent Order: {}",
        cycle_combos.unwrap().cycles[0].order
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prime_powers_below_n() {
        let result = prime_powers_below_n(10, &[0, 0, 0, 0]);
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].len(), 4);
        assert_eq!(result[1].len(), 3);
        assert_eq!(result[2].len(), 2);
        assert_eq!(result[3].len(), 2);
    }

    // ... tests for each of your complicated math functions

    #[test]
    fn test_highest_equiv_order_3_registers_3x3() {
        let ksolve = puzzle("3x3").ksolve();
        let puzzle = ksolve.sets();
        let cycle_combos: Option<CycleCombination> = optimal_equivalent_combination(puzzle, 3);
        assert_eq!(
            cycle_combos.unwrap().cycles[0].order,
            Int::<U>::from(30_u16),
        );
    }

    #[test]
    fn test_highest_equiv_order_2_registers_3x3() {
        let ksolve = puzzle("3x3").ksolve();
        let puzzle = ksolve.sets();
        let cycle_combos: Option<CycleCombination> = optimal_equivalent_combination(puzzle, 2);
        assert_eq!(
            cycle_combos.unwrap().cycles[0].order,
            Int::<U>::from(90_u16),
        );
    }

    #[test]
    fn test_optimal_order_3_registers_3x3() {
        let ksolve = puzzle("3x3").ksolve();
        let puzzle = ksolve.sets();
        optimal_combinations(puzzle, 3);
    }

    #[test]
    fn test_optimal_order_2_registers_5X5() {
        let ksolve = puzzle("3x3").ksolve();
        let puzzle = ksolve.sets();
        optimal_combinations(puzzle, 2);
    }
}

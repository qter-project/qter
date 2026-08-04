use std::{
    fmt::Debug,
    num::{NonZeroU16, NonZeroUsize},
    simd::{Mask, Simd, cmp::SimdPartialEq},
};

use log::trace;

use crate::{
    FIRST_65_PRIMES,
    cycle_combinations_tree::DisjointRegisters,
    finder::PossibleOrder,
    puzzle::{OrbitDef, OrientationStatus, OrientationSumConstraint, PuzzleDef, orbit_index_cast},
};

enum DetailsCalculation {
    Existence(bool),
    Expansion(Option<CycleCombinationDetail>),
}

#[derive(Debug, Clone, Copy)]
pub struct Cycle {
    pub(crate) piece_count: u16,
    // we don't have to permute all ways to orient this way
    pub(crate) must_orient: bool,
}

#[derive(Debug, Default)]
pub struct CycleCombinationDetail {
    pub(crate) detail: Vec<(Box<[OrbitRemainingPieceCount]>, Box<[Vec<Cycle>]>)>,
}

#[derive(Debug)]
#[non_exhaustive]
pub struct CycleCombinationDetails<'a, 'b, const N: usize> {
    find_all: bool,
    possible_orders_except_one: &'a [PossibleOrder<N>],
    maybe_fitting_tries: Option<(u32, u32)>,
    puzzle_def: &'b PuzzleDef<N>,
    exact_register_count: NonZeroU16,

    detail: Option<CycleCombinationDetail>,

    /// Map of every register, to its cycles, to which orbit its prime power
    /// component is assigned to and bitmask
    register_assignments: Box<[RegisterCycleAssignments<N>]>,
    reg_to_orbits_constraints: Box<[OrbitConstraint]>,
    initial_reg_to_orbits_constraints: Box<[OrbitConstraint]>,
    /// Remaining piece count for every orbit
    orbit_remaining_piece_counts: Box<[OrbitRemainingPieceCount]>,
    initial_orbit_remaining_piece_counts: Box<[OrbitRemainingPieceCount]>,
    // /// Gives the best registers (register index, the exponent)
    // register_exponent_sorter: Vec<(u16, u8)>,
    // /// Gives the best orientation orders
    // best_orientations_queue: [BestOrientation; 9],
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug, Default)]
enum ShareState {
    #[default]
    None = 0,
    Orientation = 1,
    Parity = 2,
}

#[derive(Debug, Clone, Copy)]
pub struct OrbitRemainingPieceCount {
    unused: u16,
    ignored: u16,
}

#[derive(Debug, Clone, Copy)]
struct OrbitConstraint {
    share_state: ShareState,
    orientation_constraint: OrbitOrientationConstraint,
}

#[derive(Debug, Clone)]
struct RegisterCycleAssignments<const N: usize> {
    all_exponents_mask: u64,
    unassigned_exponents_mask: u64,
    // unassigned_exponents_mask: u64,
    cycle_assignments: [PPCycleAssignment; N],
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PPCycleAssignment {
    Orbit(u16, bool),
    Unassigned,
}

// #[derive(Debug, Clone, Copy)]
// enum BestOrientation {
//     Orbit(u16, SharingState),
//     Ambiguous,
//     Unassigned,
// }

// #[derive(Debug, Clone, Copy)]
// enum SaturatingOrbit {
//     Orbit(u16, u8, SharingState),
//     Ambiguous,
//     None,
// }

#[derive(Debug, PartialEq, Clone, Copy)]
enum OrbitOrientationConstraint {
    None,
    SatisfiedByRegisterCycle,
    SatisfiedByLeftoverPieces,
}

impl<'a, 'b, const N: usize> CycleCombinationDetails<'a, 'b, N> {
    #[must_use]
    pub fn new(
        exact_register_count: NonZeroU16,
        possible_orders_except_one: &'a [PossibleOrder<N>],
        max_fitting_tries: Option<u32>,
        puzzle_def: &'b PuzzleDef<N>,
    ) -> Self {
        let register_assignments = vec![
            RegisterCycleAssignments {
                all_exponents_mask: 0,
                unassigned_exponents_mask: !0,
                // unassigned_exponents_mask: !0,
                cycle_assignments: [PPCycleAssignment::Unassigned; N],
            };
            NonZeroUsize::from(exact_register_count).get()
        ]
        .into_boxed_slice();
        let orbit_defs = puzzle_def.orbit_defs();
        let reg_to_orbits_constraints = (0..NonZeroUsize::from(exact_register_count).get())
            .flat_map(|_| {
                orbit_defs.iter().map(|&orbit_def| {
                    let orientation_constraint = if matches!(
                        orbit_def.orientation,
                        OrientationStatus::CanOrient {
                            count: _,
                            sum_constraint: OrientationSumConstraint::Zero
                        }
                    ) {
                        OrbitOrientationConstraint::None
                    } else {
                        OrbitOrientationConstraint::SatisfiedByRegisterCycle
                    };
                    OrbitConstraint {
                        share_state: ShareState::default(),
                        orientation_constraint,
                    }
                })
            })
            .collect::<Box<[_]>>();
        let initial_reg_to_orbits_constraints = reg_to_orbits_constraints.clone();
        let orbit_remaining_piece_counts = puzzle_def
            .orbit_defs()
            .iter()
            .map(|orbit_def| OrbitRemainingPieceCount {
                unused: orbit_def.piece_count.get(),
                ignored: 0,
            })
            .collect::<Box<[_]>>();
        let initial_orbit_remaining_piece_counts = orbit_remaining_piece_counts.clone();
        // let register_exponent_sorter =
        //     Vec::with_capacity(NonZeroUsize::from(exact_register_count).get());
        // let best_orientations_queue = [BestOrientation::Unassigned; 9];
        Self {
            possible_orders_except_one,
            puzzle_def,
            detail: None,
            maybe_fitting_tries: max_fitting_tries.map(|i| (i, i)),
            exact_register_count,
            register_assignments,
            reg_to_orbits_constraints,
            initial_reg_to_orbits_constraints,
            orbit_remaining_piece_counts,
            initial_orbit_remaining_piece_counts,
            find_all: false,
            // register_exponent_sorter,
            // best_orientations_queue,
        }
    }

    fn recursive_backtrack(&mut self, registers: DisjointRegisters, register_index: u16) -> bool {
        if let Some(&mut (initial_fitting_tries, ref mut remaining_fitting_tries)) =
            self.maybe_fitting_tries.as_mut()
        {
            if let Some(next_remaining_fitting_tries) = remaining_fitting_tries.checked_sub(1) {
                *remaining_fitting_tries = next_remaining_fitting_tries;
            } else {
                *remaining_fitting_tries = initial_fitting_tries;
                return false;
            }
        }
        let register_index2 = usize::from(register_index);
        let unassigned_exponents_mask =
            self.register_assignments[register_index2].unassigned_exponents_mask;
        if unassigned_exponents_mask == 0 {
            // we do not have to care about the mod 2 case; when we have more than 1 cycle
            // we can always fit by just picking two to orient
            let next_register_index = register_index + 1;
            let next_register_index2 = usize::from(next_register_index);
            let leaf = next_register_index2 == self.register_assignments.len();

            // TODO: is this valid?
            //                                             cycle
            // .piece_count
            // .div_exact(u16::from(
            //     orbit_def.orientation_count().get()
            // ))
            // .unwrap_or(cycle.piece_count)

            let (orbits_constraints, next_orbits_constraints) = self.reg_to_orbits_constraints
                [register_index2 * self.puzzle_def.orbit_defs().len().get()..]
                .split_at_mut(self.puzzle_def.orbit_defs().len().get());
            for (
                orbit_index2,
                &mut OrbitConstraint {
                    ref mut share_state,
                    orientation_constraint: orbit_orientation_constraint,
                },
            ) in orbits_constraints.iter_mut().enumerate()
            {
                // Promote only if we have no other share rn
                if orbit_orientation_constraint
                    == OrbitOrientationConstraint::SatisfiedByLeftoverPieces
                    && *share_state == ShareState::None
                {
                    assert_ne!(self.orbit_remaining_piece_counts[orbit_index2].unused, 0);
                    *share_state = ShareState::Orientation;
                }
                if let Some(OrbitConstraint {
                    share_state: next_share_state,
                    ..
                }) = next_orbits_constraints.get_mut(orbit_index2)
                {
                    assert!(!leaf);
                    *next_share_state = *share_state;
                } else {
                    assert!(leaf);
                }
            }

            if !leaf {
                let found = self.recursive_backtrack(registers, next_register_index);

                if let Some(prev_register_index2) = register_index2.checked_sub(1) {
                    let (prev_orbits_constraints, orbits_constraints) = self
                        .reg_to_orbits_constraints
                        [prev_register_index2 * self.puzzle_def.orbit_defs().len().get()..]
                        .split_at_mut(self.puzzle_def.orbit_defs().len().get());
                    for (
                        orbit_index2,
                        &OrbitConstraint {
                            share_state: prev_share_state,
                            ..
                        },
                    ) in prev_orbits_constraints.iter().enumerate()
                    {
                        orbits_constraints[orbit_index2].share_state = prev_share_state;
                    }
                } else {
                    for OrbitConstraint { share_state, .. } in self
                        .reg_to_orbits_constraints
                        .iter_mut()
                        .take(self.puzzle_def.orbit_defs().len().get())
                    {
                        *share_state = ShareState::default();
                    }
                }

                return found;
            }

            if self.find_all {
                // TODO: allocator
                let mut reg_to_orbits_to_cycles =
                    vec![
                        vec![];
                        NonZeroUsize::from(self.exact_register_count).get()
                            * self.puzzle_def.orbit_defs().len().get()
                    ]
                    .into_boxed_slice();
                for register_index in 0..self.exact_register_count.get() {
                    let register_index2 = usize::from(register_index);
                    let register_assignment = &self.register_assignments[register_index2];
                    let mut all_exponents = register_assignment.all_exponents_mask;
                    while all_exponents != 0 {
                        let prime_index = all_exponents.trailing_zeros() as usize;
                        let prime = FIRST_65_PRIMES[prime_index];
                        let register_order_exp = registers
                            .get_order(register_index, self.possible_orders_except_one)
                            .unwrap()
                            .order
                            .prime_exponent(prime_index);
                        let cycle_piece_count = prime.pow(u32::from(register_order_exp));
                        // We can have a 7+ on edges to serve as following the 2 cycle and a 7 cycle, in the false case
                        if let PPCycleAssignment::Orbit(orbit_index, must_orient) =
                            register_assignment.cycle_assignments[prime_index]
                        {
                            let orbit_index2 = usize::from(orbit_index);
                            let reg_orbit_index = register_index2
                                * self.puzzle_def.orbit_defs().len().get()
                                + orbit_index2;
                            reg_to_orbits_to_cycles[reg_orbit_index].push(Cycle {
                                piece_count: cycle_piece_count,
                                must_orient,
                            });
                            // only the last register has the most recent share state propagation
                            if register_index == self.exact_register_count.get() - 1 {
                                self.orbit_remaining_piece_counts[orbit_index2].ignored =
                                    self.reg_to_orbits_constraints[reg_orbit_index].share_state
                                        as u16;
                            }
                        }
                        all_exponents ^= all_exponents.isolate_lowest_one();
                    }
                }
                self.detail.get_or_insert_default().detail.push((
                    self.orbit_remaining_piece_counts.clone(),
                    reg_to_orbits_to_cycles,
                ));
            }
            return true;
        }

        // pop the highest one so we fit large primes first; higher chance of reaching the fail state
        let prime_index2 = unassigned_exponents_mask.ilog2() as usize;
        let prime = FIRST_65_PRIMES[prime_index2];

        // Nonzero because it is in the unassigned mask
        let register_order = &registers
            .get_order(register_index, self.possible_orders_except_one)
            .unwrap()
            .order;
        let register_order_exp = register_order.prime_exponent(prime_index2);

        // TODO: smarter orbit ordering, ignore identical orbits
        let mut prev_idx_and_piece_count: Option<(usize, NonZeroU16)> = None;
        for _ in 0..self.puzzle_def.orbit_defs().len().get() {
            let (orbit_index2, piece_count) = self
                .puzzle_def
                .orbit_defs()
                .iter()
                .enumerate()
                .filter_map(|(orbit_index, &OrbitDef { piece_count, .. })| {
                    if prev_idx_and_piece_count.is_none_or(
                        |(prev_orbit_index, prev_piece_count)| {
                            piece_count > prev_piece_count
                                || (piece_count == prev_piece_count
                                    && orbit_index > prev_orbit_index)
                        },
                    ) {
                        Some((orbit_index, piece_count))
                    } else {
                        None
                    }
                })
                .min_by(
                    |&(orbit_index1, piece_count1), &(orbit_index2, piece_count2)| {
                        piece_count1
                            .cmp(&piece_count2)
                            .then_with(|| orbit_index1.cmp(&orbit_index2))
                    },
                )
                .expect("there are exactly <number of orbits> distinct (index, value) pairs");

            // TODO: optimization to not place same cycle in orbit with 1 orientation
            // TODO: min piece count pruning
            let orbit_index = orbit_index_cast(orbit_index2);
            let reg_orbit_constraint_index =
                register_index2 * self.puzzle_def.orbit_defs().len().get() + orbit_index2;

            let orbit_orientation_exps = &self.puzzle_def.orientations_exps()[orbit_index2];
            let orbit_orientation_contributing_prime_index = orbit_orientation_exps
                .0
                .simd_ne(Simd::splat(0))
                .to_bitmask()
                .trailing_zeros()
                as usize;
            let orbit_orientation_exp = orbit_orientation_exps.prime_exponent(prime_index2);
            let OrbitConstraint {
                share_state,
                orientation_constraint: _,
            } = self.reg_to_orbits_constraints[reg_orbit_constraint_index];
            let orbit_unused_piece_count = self.orbit_remaining_piece_counts[orbit_index2].unused;

            // FIXME: do parity by checking ParityConstraint::Even first in OrbitDef

            // Does the orbit have a non-zero exponent of the prime power we're fitting?
            let canonically_orients = if orbit_orientation_exp == 0 {
                &[false][..]
            } else {
                // true first so we have a greater chance of finding a solution earlier
                &[true, false][..]
            };
            for &canonically_orient in canonically_orients {
                let cycle_piece_count = if canonically_orient {
                    let exp = register_order_exp.saturating_sub(orbit_orientation_exp);
                    if exp == 0 {
                        0
                    } else {
                        prime.pow(u32::from(exp))
                    }
                } else {
                    prime.pow(u32::from(register_order_exp))
                };
                // TODO: figure out if prime powers are allowed (like 27, 81, etc)
                let Some(next_orbit_unused_piece_count) =
                    orbit_unused_piece_count.checked_sub(cycle_piece_count)
                else {
                    trace!(
                        "{register_index} {orbit_index} failed: {orbit_unused_piece_count} < \
                         {cycle_piece_count}; tried {prime}; must_orient {canonically_orient}",
                    );
                    continue;
                };
                if next_orbit_unused_piece_count < share_state as u16 {
                    continue;
                }

                // If not the current orbit
                let assign_cycle_orient = orbit_orientation_exp == 0
                    && orbit_orientation_contributing_prime_index != 64
                    && unassigned_exponents_mask
                        & (1 << orbit_orientation_contributing_prime_index)
                        != 0
                    && orbit_orientation_exps
                        .prime_exponent(orbit_orientation_contributing_prime_index)
                        >= register_order
                            .prime_exponent(orbit_orientation_contributing_prime_index);
                let must_orient = assign_cycle_orient || canonically_orient;

                // Do we already share something?
                let share_anything =
                    share_state == ShareState::Orientation || share_state == ShareState::Parity;

                let slot = &mut self.reg_to_orbits_constraints[reg_orbit_constraint_index]
                    .orientation_constraint;
                let old = *slot;
                match old {
                    OrbitOrientationConstraint::None => {
                        if must_orient && !share_anything {
                            // If we orient this cycle, and there is no room left, and the constraint is unsatisfied (i.e. we have a zero sum constraint), and we don't already have ignored pieces, then this is impossible.
                            if next_orbit_unused_piece_count == 0 {
                                continue;
                            }
                            *slot = OrbitOrientationConstraint::SatisfiedByLeftoverPieces;
                        } else {
                            *slot = OrbitOrientationConstraint::SatisfiedByRegisterCycle;
                        }
                    }
                    OrbitOrientationConstraint::SatisfiedByLeftoverPieces => {
                        *slot = OrbitOrientationConstraint::SatisfiedByRegisterCycle;
                    }
                    OrbitOrientationConstraint::SatisfiedByRegisterCycle => (),
                }
                trace!(
                    "{register_index} {orbit_index}: updated {old:?} -> {:?}; assigned {prime} \
                     (share state {share_state:?}) ({orbit_unused_piece_count} -> \
                     {next_orbit_unused_piece_count}); assign_cycle_orient {assign_cycle_orient}",
                    *slot
                );

                self.orbit_remaining_piece_counts[orbit_index2].unused =
                    next_orbit_unused_piece_count;
                self.register_assignments[register_index2].unassigned_exponents_mask ^=
                    1 << prime_index2;
                if assign_cycle_orient {
                    self.register_assignments[register_index2].unassigned_exponents_mask ^=
                        1 << orbit_orientation_contributing_prime_index;
                }
                self.register_assignments[register_index2].cycle_assignments[prime_index2] =
                    PPCycleAssignment::Orbit(orbit_index, must_orient);

                let exists = self.recursive_backtrack(registers, register_index);
                if exists && !self.find_all {
                    return true;
                }

                trace!(
                    "{register_index} {orbit_index}: undo {old:?} <- {:?}; unassigned {prime} \
                     (share state {share_state:?})",
                    self.reg_to_orbits_constraints[reg_orbit_constraint_index]
                        .orientation_constraint
                );

                self.orbit_remaining_piece_counts[orbit_index2].unused = orbit_unused_piece_count;
                self.register_assignments[register_index2].unassigned_exponents_mask |=
                    1 << prime_index2;
                if assign_cycle_orient {
                    self.register_assignments[register_index2].unassigned_exponents_mask |=
                        1 << orbit_orientation_contributing_prime_index;
                }
                // We need to do this now because we don't guarantee to assign every cycle anymore
                self.register_assignments[register_index2].cycle_assignments[prime_index2] =
                    PPCycleAssignment::Unassigned;
                self.reg_to_orbits_constraints[reg_orbit_constraint_index].orientation_constraint =
                    old;
            }
            prev_idx_and_piece_count = Some((orbit_index2, piece_count));
        }
        false
    }

    #[must_use]
    fn calculate(&mut self, registers: DisjointRegisters) -> DetailsCalculation {
        self.reg_to_orbits_constraints
            .clone_from_slice(&self.initial_reg_to_orbits_constraints);
        self.orbit_remaining_piece_counts
            .clone_from_slice(&self.initial_orbit_remaining_piece_counts);

        // Every prime used by the register orders
        let mut orienting_registers_prime_mask = Mask::splat(false);

        for (register_index, possible_order) in registers
            .iter_orders(self.possible_orders_except_one)
            .enumerate()
        {
            let all_exponents = possible_order.order.0.simd_ne(Simd::splat(0));
            self.register_assignments[register_index].all_exponents_mask =
                all_exponents.to_bitmask();
            self.register_assignments[register_index].unassigned_exponents_mask =
                all_exponents.to_bitmask();
            orienting_registers_prime_mask |= all_exponents;
        }
        // let orienting_registers_prime_mask = orienting_registers_prime_mask.to_bitmask();

        // let mut orienting_registers_prime_mask2 = orienting_registers_prime_mask;
        // while orienting_registers_prime_mask2 != 0 {
        //     let prime_index = orienting_registers_prime_mask2.trailing_zeros() as usize;
        //     let prime = FIRST_65_PRIMES[prime_index];
        //     self.best_orientations_queue
        //         .fill(BestOrientation::Unassigned);
        //     for (orbit_index, (orientation_exps, &orbit_def)) in self
        //         .puzzle_def
        //         .orientations_exps()
        //         .iter()
        //         .zip(self.puzzle_def.orbit_defs().iter())
        //         .enumerate()
        //     {
        //         let orbit_index = orbit_index_cast(orbit_index);
        //         // counterexample:
        //         // o1: 5 pieces 48 ori
        //         //
        //         // fit 576: 3 3 2 2 2 2 2 2
        //         //
        //         // if you go with 3 (worse); 9 cycle -> 3 cycle; saves 6 pieces
        //         // if you go with 2 (better); 64 cycle -> 4 cycle; saves 60 pieces
        //         let exactly_prime_factors =
        //             (orientation_exps.0.simd_ne(Simd::splat(0)).to_bitmask()
        //                 & orienting_registers_prime_mask)
        //                 == (1 << prime_index);
        //         if !exactly_prime_factors {
        //             continue;
        //         }
        //         let orbit_orientation_exp = orientation_exps.prime_exponent(prime_index);
        //         let required_extra_pieces = if prime_index == 0
        //             && (orbit_def.parity_constraint == ParityConstraint::Even
        //                 || orbit_def.parity_constraint == ParityConstraint::None)
        //         {
        //             // - 2^n is not necessarily valid with +1 of space because of parity
        //             // we COULD parity swap with another orbit; however we just focus on the
        //             // worst case
        //             SharingState::Parity
        //         } else if matches!(
        //             orbit_def.orientation,
        //             OrientationStatus::CanOrient {
        //                 count: _,
        //                 sum_constraint: OrientationSumConstraint::Zero
        //             }
        //         ) {
        //             // - x^n is not necessarily valid with +0 of space because of
        //             // orientation
        //             SharingState::Orientation
        //         } else {
        //             SharingState::None
        //         };

        //         // If there is an ambiguity among an exponent between two exponents,
        //         // we can assign a register to either; this violates the guarantee
        //         let slot = &mut self.best_orientations_queue[usize::from(orbit_orientation_exp)];
        //         match slot {
        //             BestOrientation::Orbit(..) => *slot = BestOrientation::Ambiguous,
        //             BestOrientation::Unassigned => {
        //                 *slot = BestOrientation::Orbit(orbit_index, required_extra_pieces);
        //             }
        //             BestOrientation::Ambiguous => (),
        //         }
        //     }

        //     // For the current prime index, iterate through every register and figure out
        //     // which registers have the largest power of this prime.
        //     self.register_exponent_sorter.extend(
        //         registers
        //             .iter_orders(self.possible_orders_except_one)
        //             .enumerate()
        //             .filter_map(|(register_index2, possible_order)| {
        //                 let register_index = register_index_cast(register_index2);
        //                 let register_order_exp = possible_order.order.prime_exponent(prime_index);
        //                 // - 2^1 is not always best
        //                 // at register_order_exp==0, we no longer have primes in this register
        //                 // order, so there is nothing to assign
        //                 if prime_index == 0 && register_order_exp == 1 || register_order_exp == 0 {
        //                     None
        //                 } else {
        //                     Some((register_index, register_order_exp))
        //                 }
        //             }),
        //     );
        //     self.register_exponent_sorter
        //         .sort_unstable_by_key(|&(_, register_order_exp)| {
        //             std::cmp::Reverse(register_order_exp)
        //         });

        //     // Try to fit a register's prime power cycle into an orbit such that it would
        //     // benefit the most from a share
        //     for (register_index, register_order_exp) in self.register_exponent_sorter.drain(..) {
        //         let register_index2 = usize::from(register_index);
        //         let slot = &mut self.register_assignments[register_index2];
        //         let mut try_assign_pp_to_orbit = |orbit_index: u16,
        //                                           orbit_orientation_exp: u8,
        //                                           required_extra_pieces: SharingState|
        //          -> bool {
        //             let orbit_index2 = usize::from(orbit_index);
        //             let orbit_remaining_piece_count =
        //                 &mut self.orbit_remaining_piece_counts[orbit_index2];
        //             let orbit_orientation_constraint = &mut self.orbit_orientation_constraints
        //                 [register_index2 * self.puzzle_def.orbit_defs().len().get() + orbit_index2];
        //             let exp = register_order_exp.saturating_sub(orbit_orientation_exp);
        //             let cycle_piece_count = if exp == 0 {
        //                 0
        //             } else {
        //                 prime.pow(u32::from(exp))
        //             };

        //             if let Some(next_orbit_remaining_piece_count) =
        //                 orbit_remaining_piece_count.checked_sub(cycle_piece_count)
        //             {
        //                 let component_remaining_piece_count = &mut self
        //                     .component_remaining_piece_counts[usize::from(
        //                     self.puzzle_def.orbit_index_to_component_index(orbit_index),
        //                 )];
        //                 let next_component_remaining_piece_count =
        //                     *component_remaining_piece_count - u32::from(cycle_piece_count);

        //                 if required_extra_pieces.enough_leftover_pieces(
        //                     next_orbit_remaining_piece_count,
        //                     next_component_remaining_piece_count,
        //                     // Assume worse case; it is otherwise not simple to keep track of when
        //                     // an orbit has an orienting cycle during this stage (it requires at
        //                     // least a pass of all primes)
        //                     false,
        //                 ) {
        //                     *orbit_remaining_piece_count = next_orbit_remaining_piece_count;
        //                     *component_remaining_piece_count = next_component_remaining_piece_count;
        //                     let orbit_def = self.puzzle_def.orbit_defs()[orbit_index2];
        //                     if matches!(
        //                         orbit_def.orientation,
        //                         OrientationStatus::CanOrient {
        //                             count: _,
        //                             sum_constraint: OrientationSumConstraint::Zero
        //                         }
        //                     ) {
        //                         *orbit_orientation_constraint =
        //                             OrbitOrientationConstraint::Unsatisfied;
        //                     }

        //                     slot.unassigned_exponents_mask ^= 1 << prime_index;
        //                     slot.cycle_assignments[prime_index] =
        //                         PPCycleAssignment::Orbit(orbit_index, orbit_orientation_exp);
        //                     return true;
        //                 }
        //             }
        //             false
        //         };
        //         // Descending exp order of available orientation-sharing cycles
        //         let mut saturated_orbit_found = SaturatingOrbit::None;
        //         for (orbit_index, orbit_orientation_exp, required_extra_pieces) in self
        //             .best_orientations_queue
        //             .iter()
        //             .enumerate()
        //             .filter_map(|(orbit_orientation_exp, &slot)| {
        //                 if let BestOrientation::Orbit(orbit_index, required_share) = slot {
        //                     // array is 9 elements long
        //                     #[allow(clippy::cast_possible_truncation)]
        //                     Some((orbit_index, orbit_orientation_exp as u8, required_share))
        //                 } else {
        //                     None
        //                 }
        //             })
        //             .rev()
        //         {
        //             // Orbit provides more orientation than needed for this register order. We may
        //             // still have the ambiguous case
        //             if orbit_orientation_exp >= register_order_exp {
        //                 trace!(
        //                     "prime={prime}; reg={register_index}; {orbit_orientation_exp:?} > \
        //                      {register_order_exp}"
        //                 );
        //                 if let SaturatingOrbit::Orbit(..) = saturated_orbit_found {
        //                     saturated_orbit_found = SaturatingOrbit::Ambiguous;
        //                 } else {
        //                     saturated_orbit_found = SaturatingOrbit::Orbit(
        //                         orbit_index,
        //                         orbit_orientation_exp,
        //                         required_extra_pieces,
        //                     );
        //                 }
        //             } else if try_assign_pp_to_orbit(
        //                 orbit_index,
        //                 orbit_orientation_exp,
        //                 required_extra_pieces,
        //             ) {
        //                 break;
        //             }
        //         }
        //         if let SaturatingOrbit::Orbit(
        //             orbit_index,
        //             orbit_orientation_exp,
        //             required_extra_pieces,
        //         ) = saturated_orbit_found
        //         {
        //             try_assign_pp_to_orbit(
        //                 orbit_index,
        //                 orbit_orientation_exp,
        //                 required_extra_pieces,
        //             );
        //         }
        //     }

        //     orienting_registers_prime_mask2 ^= orienting_registers_prime_mask2.isolate_lowest_one();
        // }

        // for (i, x) in self.reg_to_assignments.iter().enumerate() {
        //     #[allow(clippy::missing_panics_doc)]
        //     let order = &registers
        //         .get_order(i as u16, self.possible_orders_except_one)
        //         .unwrap()
        //         .order;
        //     println!(
        //         "reg {order:?}; all {:b}; unassigned {:b}; {:#?}",
        //         x.all_exponents_mask, x.unassigned_exponents_mask,
        // x.cycle_assignments     );
        // }
        // println!("{:?}", self.orbit_remaining_piece_counts);
        // TODO: if an orbit has at least the first highest cycle + second highest cycle
        // number of pieces, we will never not satisfy an orientation constraint
        if self.find_all {
            self.recursive_backtrack(registers, 0);
            DetailsCalculation::Expansion(self.detail.take())
        } else {
            DetailsCalculation::Existence(self.recursive_backtrack(registers, 0))
        }
    }

    pub fn calculate_existence(&mut self, registers: DisjointRegisters) -> bool {
        self.find_all = false;
        let DetailsCalculation::Existence(exists) = self.calculate(registers) else {
            unreachable!();
        };
        exists
    }

    pub fn calculate_all(
        &mut self,
        registers: DisjointRegisters,
    ) -> Option<CycleCombinationDetail> {
        self.find_all = true;
        let DetailsCalculation::Expansion(detail) = self.calculate(registers) else {
            unreachable!();
        };
        detail
    }
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU16, time::Instant};

    use humanize_duration::{Truncate, prelude::DurationExt};

    use crate::{
        cycle_combination_details::CycleCombinationDetails,
        cycle_combinations_tree::DisjointRegisters,
        finder::{PossibleOrder, mk_possible_orders_except_one},
        nonemptyvec::NonemptySlice,
        orderexps::OrderExps,
        puzzle::{
            EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef,
            PuzzleDef, cubeN::CUBE4, minxN::MINX3,
        },
    };

    #[test_log::test]
    fn foo3() {
        let crazy = PuzzleDef::<32>::new(
            vec![
                PartialOrbitDef {
                    piece_count: 5.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 27,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 5.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 9,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
            ],
            EvenParityConstraints(vec![vec![0, 1]]),
        )
        .unwrap();

        CycleCombinationDetails::new(
            NonZeroU16::new(1).unwrap(),
            &[PossibleOrder {
                order: OrderExps::try_from(NonZeroU16::new(3).unwrap()).unwrap(),
                min_piece_count: 1.try_into().unwrap(),
            }],
            None,
            &crazy,
        )
        .calculate_existence(DisjointRegisters::from(
            NonemptySlice::try_from(&[0][..]).unwrap(),
        ));
    }

    #[test_log::test]
    fn foo2() {
        let crazy = PuzzleDef::<32>::new(
            vec![
                PartialOrbitDef {
                    piece_count: 5.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 85,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 5.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 77,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 5.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 59,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 3.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 56,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 5.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 50,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 5.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 48,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 5.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 48,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 5.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 34,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 5.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 25,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 5.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 15,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
            ],
            EvenParityConstraints(vec![vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]]),
        )
        .unwrap();

        CycleCombinationDetails::new(
            NonZeroU16::new(6).unwrap(),
            &[
                PossibleOrder {
                    order: OrderExps::try_from(NonZeroU16::new(2).unwrap()).unwrap(),
                    min_piece_count: 1.try_into().unwrap(),
                },
                PossibleOrder {
                    order: OrderExps::try_from(NonZeroU16::new(4).unwrap()).unwrap(),
                    min_piece_count: 1.try_into().unwrap(),
                },
                PossibleOrder {
                    order: OrderExps::try_from(NonZeroU16::new(5).unwrap()).unwrap(),
                    min_piece_count: 1.try_into().unwrap(),
                },
                PossibleOrder {
                    order: OrderExps::try_from(NonZeroU16::new(25).unwrap()).unwrap(),
                    min_piece_count: 1.try_into().unwrap(),
                },
                PossibleOrder {
                    order: OrderExps::try_from(NonZeroU16::new(12).unwrap()).unwrap(),
                    min_piece_count: 1.try_into().unwrap(),
                },
                PossibleOrder {
                    order: OrderExps::try_from(NonZeroU16::new(16).unwrap()).unwrap(),
                    min_piece_count: 1.try_into().unwrap(),
                },
            ],
            None,
            &crazy,
        )
        .calculate_existence(DisjointRegisters::from(
            NonemptySlice::try_from(&[0, 1, 2, 3, 4, 5][..]).unwrap(),
        ));
    }

    #[test_log::test]
    fn foo1() {
        let minx3 = MINX3.clone();
        let possible_orders_except_one =
            mk_possible_orders_except_one(&minx3, minx3.possible_orders(None).unwrap());
        // 2520 630 420
        let mut detail = CycleCombinationDetails::new(
            NonZeroU16::new(3).unwrap(),
            &possible_orders_except_one,
            None,
            &minx3,
        );
        let now = Instant::now();
        detail.calculate_existence(DisjointRegisters::from(
            NonemptySlice::try_from(&[504, 251, 196][..]).unwrap(),
        ));
        println!("{}", now.elapsed().human(Truncate::Micro));

        // 2520 630 420
        //
        // 2 2 2 3 3 5 7 : 4e 3c
        // 2     3 3 5 7 : 3c
        // 2 2   3   5 7 : 2e
        //
        // 24 edges 5 5 7 7
        // 14 corners 7 5
        //
        // 2520:
        //
        // e: (4+, 5+); total 9/30
        // c: (3+, 7+); total 10/20
        //
        // 630:
        //
        // e: (5+, 7+); total 12/30
        // c: (3+); total 3/20
        //
        // 420:
        //
        // e: (2+, 7+); total 9/30
        // c: (5+); total 5/20
        //
        // parity share 2 edges or corners
        //
        // 30/30
        // 18/20

        println!("{detail:#?}");
        panic!();
    }

    #[test_log::test]
    fn foo4() {
        let cube4 = CUBE4.clone();
        let possible_orders_except_one =
            mk_possible_orders_except_one(&cube4, cube4.possible_orders(None).unwrap());
        let mut detail = CycleCombinationDetails::new(
            NonZeroU16::new(2).unwrap(),
            &possible_orders_except_one,
            None,
            &cube4,
        );
        detail.calculate_existence(DisjointRegisters::from(
            NonemptySlice::try_from(&[875, 1][..]).unwrap(),
        ));

        // 2520 630 420
        //
        // 2 2 2 3 3 5 7 : 4e 3c
        // 2     3 3 5 7 : 3c
        // 2 2   3   5 7 : 2e
        //
        // 24 edges 5 5 7 7
        // 14 corners 7 5
        //
        // 2520:
        //
        // e: (4+, 5+); total 9/30
        // c: (3+, 7+); total 10/20
        //
        // 630:
        //
        // e: (5+, 7+); total 12/30
        // c: (3+); total 3/20
        //
        // 420:
        //
        // e: (2+, 7+); total 9/30
        // c: (5+); total 5/20
        //
        // parity share 2 edges or corners
        //
        // 30/30
        // 18/20

        println!("{detail:#?}");
        panic!();
    }
}

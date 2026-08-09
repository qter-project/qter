use std::{
    fmt::Debug,
    num::{NonZeroU16, NonZeroUsize},
    simd::{Mask, Simd, cmp::SimdPartialEq},
};

use log::trace;

use crate::{
    FIRST_65_PRIMES,
    cycle_combinations_tree::DisjointRegisters,
    finder::{PossibleOrder, SolutionExpansion},
    orderexps::OrderExps,
    puzzle::{OrbitDef, OrientationStatus, OrientationSumConstraint, PuzzleDef, orbit_index_cast},
};

enum SolutionsCalculation {
    Existence(bool),
    MaybeExpansion(Option<CycleCombinationSolutions>),
}

#[derive(Debug, Clone, Copy)]
pub struct Cycle {
    pub(crate) piece_count: u16,
    // we don't have to permute all ways to orient this way
    pub(crate) must_orient: bool,
}

#[derive(Debug, Default)]
pub struct CycleCombinationSolutions(pub(crate) Vec<CycleCombinationSolution>);

#[derive(Debug, Default)]
pub struct CycleCombinationSolution {
    pub(crate) orbit_remaining_pieces: Box<[OrbitRemainingPieces]>,
    // TODO: we cannot guarantee that every cycle belongs to exactly one orbit, in the expansion
    pub(crate) register_orbit_cycles: Box<[Vec<Cycle>]>,
}

#[derive(Debug)]
#[non_exhaustive]
pub struct CycleCombinationSolutionsCalculator<'a, const N: usize> {
    expansion: bool,
    possible_orders_except_one: &'a [PossibleOrder<N>],
    maybe_fitting_tries: Option<(u32, u32)>,
    solution_expansion: SolutionExpansion,
    puzzle_def: &'a PuzzleDef<N>,
    exact_register_count: NonZeroU16,

    maybe_solutions: Option<CycleCombinationSolutions>,

    /// Map of every register, to its cycles, to which orbit its prime power
    /// component is assigned to and bitmask
    register_assignments: Box<[RegisterCycleAssignments<N>]>,
    register_orbit_constraints: Box<[RegisterOrbitConstraint]>,
    initial_register_orbit_constraints: Box<[RegisterOrbitConstraint]>,
    /// Remaining piece count for every orbit
    orbit_remaining_pieces: Box<[OrbitRemainingPieces]>,
    initial_orbit_remaining_piece_counts: Box<[OrbitRemainingPieces]>,
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

#[derive(Clone, Copy, Debug, PartialEq)]
enum OrientationSatisfiedBy {
    CycleAndLeftoverPiece,
    LeftoverPiece,
    Satisfied,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CycleOrientState {
    None,
    Canonical,
    Noncanonical,
}

#[derive(Debug, Clone, Copy)]
pub struct OrbitRemainingPieces {
    pub(crate) unused: u16,
    pub(crate) ignored: u16,
}

#[derive(Debug, Clone, Copy)]
struct RegisterOrbitConstraint {
    known_share_state: ShareState,
    orientation_satisfied_by: OrientationSatisfiedBy,
}

#[derive(Debug, Clone)]
struct RegisterCycleAssignments<const N: usize> {
    all_exponents_mask: u64,
    unassigned_exponents_mask: u64,
    // unassigned_exponents_mask: u64,
    cycle_assignments: [PrimePowerCycleAssignment; N],
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PrimePowerCycleAssignment {
    Orbit(u16, CycleOrientState),
    Unassigned,
}

#[derive(Debug, Clone, Copy)]
struct OrbitTraversalState<'a, const N: usize> {
    orbit_index2: usize,
    piece_count: NonZeroU16,
    orientation_exps: &'a OrderExps<N>,
    orientation_prime_index: usize,
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

impl<'a, const N: usize> CycleCombinationSolutionsCalculator<'a, N> {
    #[must_use]
    pub fn new(
        puzzle_def: &'a PuzzleDef<N>,
        possible_orders_except_one: &'a [PossibleOrder<N>],
        exact_register_count: NonZeroU16,
        solution_expansion: SolutionExpansion,
        maybe_max_fitting_tries: Option<u32>,
    ) -> Self {
        let register_assignments = vec![
            RegisterCycleAssignments {
                all_exponents_mask: 0,
                unassigned_exponents_mask: !0,
                // unassigned_exponents_mask: !0,
                cycle_assignments: [PrimePowerCycleAssignment::Unassigned; N],
            };
            NonZeroUsize::from(exact_register_count).get()
        ]
        .into_boxed_slice();
        let orbit_defs = puzzle_def.orbit_defs();
        let register_orbit_constraints = (0..NonZeroUsize::from(exact_register_count).get())
            .flat_map(|_| {
                orbit_defs.iter().map(|&orbit_def| {
                    let orientation_satisfied_by = if matches!(
                        orbit_def.orientation,
                        OrientationStatus::CanOrient {
                            count: _,
                            sum_constraint: OrientationSumConstraint::Zero
                        }
                    ) {
                        OrientationSatisfiedBy::CycleAndLeftoverPiece
                    } else {
                        OrientationSatisfiedBy::Satisfied
                    };
                    RegisterOrbitConstraint {
                        known_share_state: ShareState::default(),
                        orientation_satisfied_by,
                    }
                })
            })
            .collect::<Box<[_]>>();
        let initial_register_orbit_constraints = register_orbit_constraints.clone();
        let orbit_remaining_pieces = puzzle_def
            .orbit_defs()
            .iter()
            .map(|orbit_def| OrbitRemainingPieces {
                unused: orbit_def.piece_count.get(),
                ignored: 0,
            })
            .collect::<Box<[_]>>();
        let initial_orbit_remaining_piece_counts = orbit_remaining_pieces.clone();
        // let register_exponent_sorter =
        //     Vec::with_capacity(NonZeroUsize::from(exact_register_count).get());
        // let best_orientations_queue = [BestOrientation::Unassigned; 9];
        Self {
            possible_orders_except_one,
            puzzle_def,
            maybe_solutions: None,
            maybe_fitting_tries: maybe_max_fitting_tries
                .map(|max_fitting_tries| (max_fitting_tries, max_fitting_tries)),
            solution_expansion,
            exact_register_count,
            register_assignments,
            register_orbit_constraints,
            initial_register_orbit_constraints,
            orbit_remaining_pieces,
            initial_orbit_remaining_piece_counts,
            expansion: false,
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

            trace!(
                "before: {:?} {:?}",
                self.orbit_remaining_pieces, self.register_orbit_constraints,
            );

            let (orbits_constraints, next_orbits_constraints) = self.register_orbit_constraints
                [register_index2 * self.puzzle_def.orbit_defs().len().get()..]
                .split_at_mut(self.puzzle_def.orbit_defs().len().get());
            for (
                orbit_index2,
                &mut RegisterOrbitConstraint {
                    known_share_state: ref mut share_state,
                    orientation_satisfied_by: running_share_state_satisfies,
                },
            ) in orbits_constraints.iter_mut().enumerate()
            {
                // Promote only if we have no other share rn
                // TODO: parity
                if running_share_state_satisfies == OrientationSatisfiedBy::LeftoverPiece
                    && *share_state == ShareState::None
                {
                    assert_ne!(self.orbit_remaining_pieces[orbit_index2].unused, 0);
                    *share_state = ShareState::Orientation;
                }
                if let Some(RegisterOrbitConstraint {
                    known_share_state: next_share_state,
                    ..
                }) = next_orbits_constraints.get_mut(orbit_index2)
                {
                    debug_assert!(!leaf);
                    *next_share_state = *share_state;
                } else {
                    debug_assert!(leaf);
                }
            }

            if !leaf {
                let found = self.recursive_backtrack(registers, next_register_index);

                if let Some(prev_register_index2) = register_index2.checked_sub(1) {
                    let (prev_orbits_constraints, orbits_constraints) = self
                        .register_orbit_constraints
                        [prev_register_index2 * self.puzzle_def.orbit_defs().len().get()..]
                        .split_at_mut(self.puzzle_def.orbit_defs().len().get());
                    for (
                        orbit_index2,
                        &RegisterOrbitConstraint {
                            known_share_state: prev_share_state,
                            ..
                        },
                    ) in prev_orbits_constraints.iter().enumerate()
                    {
                        orbits_constraints[orbit_index2].known_share_state = prev_share_state;
                    }
                } else {
                    for RegisterOrbitConstraint {
                        known_share_state: share_state,
                        ..
                    } in self
                        .register_orbit_constraints
                        .iter_mut()
                        .take(self.puzzle_def.orbit_defs().len().get())
                    {
                        *share_state = ShareState::default();
                    }
                }

                return found;
            }

            if self.expansion {
                // TODO: allocator
                let mut register_orbit_cycles = vec![
                    vec![];
                    NonZeroUsize::from(self.exact_register_count)
                        .get()
                        * self.puzzle_def.orbit_defs().len().get()
                ]
                .into_boxed_slice();
                for register_index in 0..self.exact_register_count.get() {
                    let register_index2 = usize::from(register_index);
                    let register_order = &registers
                        .get_order(register_index, self.possible_orders_except_one)
                        .unwrap()
                        .order;
                    let register_assignment = &self.register_assignments[register_index2];
                    let mut all_exponents = register_assignment.all_exponents_mask;
                    while all_exponents != 0 {
                        let prime_index = all_exponents.trailing_zeros() as usize;
                        let prime = FIRST_65_PRIMES[prime_index];
                        let register_order_exp = register_order.prime_exponent(prime_index);
                        // We can have a 7+ on edges to serve as following the 2 cycle and a 7 cycle, in the false case
                        if let PrimePowerCycleAssignment::Orbit(orbit_index, orient_state) =
                            register_assignment.cycle_assignments[prime_index]
                        {
                            let orbit_index2 = usize::from(orbit_index);
                            let orientation_exps =
                                &self.puzzle_def.orientations_exps()[orbit_index2];
                            let exp = if orient_state == CycleOrientState::Canonical {
                                let orientation_exp = orientation_exps.prime_exponent(prime_index);
                                register_order_exp.saturating_sub(orientation_exp)
                            } else {
                                register_order_exp
                            };
                            let register_orbit_index = register_index2
                                * self.puzzle_def.orbit_defs().len().get()
                                + orbit_index2;
                            let cycle_piece_count = prime.pow(u32::from(exp));
                            register_orbit_cycles[register_orbit_index].push(Cycle {
                                piece_count: cycle_piece_count,
                                must_orient: orient_state == CycleOrientState::Canonical
                                    || orient_state == CycleOrientState::Noncanonical,
                            });
                            // only the last register has the most recent share state propagation
                            if register_index == self.exact_register_count.get() - 1 {
                                self.orbit_remaining_pieces[orbit_index2].ignored = self
                                    .register_orbit_constraints[register_orbit_index]
                                    .known_share_state
                                    as u16;
                            }
                        }
                        all_exponents ^= all_exponents.isolate_lowest_one();
                    }
                    for register_orbit_cycle in register_orbit_cycles
                        .iter_mut()
                        .skip(register_index2 * self.puzzle_def.orbit_defs().len().get())
                        .take(self.puzzle_def.orbit_defs().len().get())
                    {
                        register_orbit_cycle
                            .sort_unstable_by_key(|&Cycle { piece_count, .. }| piece_count);
                    }
                }

                self.maybe_solutions
                    .get_or_insert_default()
                    .0
                    .push(CycleCombinationSolution {
                        orbit_remaining_pieces: self.orbit_remaining_pieces.clone(),
                        register_orbit_cycles,
                    });
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

        let mut traverse_canonically_orients = false;
        let mut maybe_prev_traversal_state: Option<OrbitTraversalState<N>> = None;
        let orientations_exps = self.puzzle_def.orientations_exps();
        // if p is 3, visit oris 8 7 5 4 2 9 3
        // if p is 7, visit oris 8 7 5 4 2 9 3
        //
        // if p is 7, visit oris 2 3
        loop {
            // TODO: smarter orbit ordering, ignore identical orbits
            // optimization to not place same cycle in orbit with same orientations
            let Some(orbit_traversal_state) = self
                .puzzle_def
                .orbit_defs()
                .iter()
                .zip(orientations_exps.iter())
                .enumerate()
                .filter_map(
                    |(orbit_index2, (&OrbitDef { piece_count, .. }, orientation_exps))| {
                        let orientation_prime_index = orientation_exps
                            .0
                            .simd_ne(Simd::splat(0))
                            .to_bitmask()
                            .trailing_zeros()
                            as usize;
                        let orbit_can_canonically_orient = orientation_prime_index == prime_index2;
                        if traverse_canonically_orients == orbit_can_canonically_orient
                            && maybe_prev_traversal_state.is_none_or(
                                |OrbitTraversalState {
                                     orbit_index2: prev_orbit_index2,
                                     piece_count: prev_piece_count,
                                     ..
                                 }| {
                                    piece_count
                                        .cmp(&prev_piece_count)
                                        .then_with(|| orbit_index2.cmp(&prev_orbit_index2))
                                        .is_gt()
                                },
                            )
                        {
                            Some(OrbitTraversalState {
                                orbit_index2,
                                piece_count,
                                orientation_exps,
                                orientation_prime_index,
                            })
                        } else {
                            None
                        }
                    },
                )
                // TODO: reduce instead of min_by to avoid extra loop
                .min_by(|a, b| {
                    a.piece_count
                        .cmp(&b.piece_count)
                        .then_with(|| a.orbit_index2.cmp(&b.orbit_index2))
                })
            else {
                if traverse_canonically_orients {
                    break;
                }
                traverse_canonically_orients = true;
                maybe_prev_traversal_state = None;
                continue;
            };
            maybe_prev_traversal_state = Some(orbit_traversal_state);

            let OrbitTraversalState {
                orbit_index2,
                orientation_exps,
                orientation_prime_index,
                ..
            } = orbit_traversal_state;

            // TODO: min piece count pruning
            let orbit_index = orbit_index_cast(orbit_index2);
            let register_orbit_constraint_index =
                register_index2 * self.puzzle_def.orbit_defs().len().get() + orbit_index2;

            let orientation_exp = orientation_exps.prime_exponent(prime_index2);
            let orbit_unused_piece_count = self.orbit_remaining_pieces[orbit_index2].unused;

            // TODO: do parity by checking ParityConstraint::Even first in OrbitDef

            // Does the orbit have a non-zero exponent of the prime power we're fitting?
            let orient_states = if traverse_canonically_orients {
                // true first so we have a greater chance of finding a solution earlier
                &[CycleOrientState::Canonical, CycleOrientState::None][..]
            } else {
                &[CycleOrientState::None][..]
            };
            for &(mut orient_state) in orient_states {
                let RegisterOrbitConstraint {
                    known_share_state,
                    orientation_satisfied_by,
                } = &mut self.register_orbit_constraints[register_orbit_constraint_index];
                let cycle_piece_count = if orient_state == CycleOrientState::Canonical {
                    let exp = register_order_exp.saturating_sub(orientation_exp);
                    if exp == 0 {
                        match *orientation_satisfied_by {
                            OrientationSatisfiedBy::CycleAndLeftoverPiece => 2,
                            OrientationSatisfiedBy::LeftoverPiece => 1,
                            OrientationSatisfiedBy::Satisfied => 0,
                        }
                    } else {
                        // TODO: test case with a singular 4 cycle on edges. it should fail
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
                         {cycle_piece_count}; tried {prime}; orient state {orient_state:?}",
                    );
                    continue;
                };
                if next_orbit_unused_piece_count < *known_share_state as u16 {
                    continue;
                }

                // order 9
                // 3 ori
                //
                // we have another order 3, we get it for free by
                //
                // 3+
                //
                // but now lcm(9, 3) = 9??
                //
                //
                //
                // order 7
                // 2^1 ori
                //
                // we have exponent 2^1, we get it for free by
                //
                // 2+
                // x
                //
                // 2^2 ori , 2^2 exponent is good
                // 2^3 ori , 2^2 exponent is good
                // 2^1 ori , 2^2 exponent is bad
                // claim: traverse_canonically_orients and assign_cycle_orient together cannot be true
                // 5 then 2
                // let must_noncanonically_orient = !traverse_canonically_orients
                //     && orientation_prime_index != 64
                //     && unassigned_exponents_mask & (1 << orientation_prime_index) != 0
                //     && orientation_exps.prime_exponent(orientation_prime_index)
                //         >= register_order.prime_exponent(orientation_prime_index);
                // let must_orient = must_noncanonically_orient || must_canonically_orient;

                // Do we already share something?
                // TODO: do we need to visit 2s first, or last, or it doesnt matter?
                let share_anything = *known_share_state == ShareState::Orientation
                    || *known_share_state == ShareState::Parity;

                let old_orientation_satisfied_by = std::mem::replace(
                    orientation_satisfied_by,
                    match *orientation_satisfied_by {
                        OrientationSatisfiedBy::CycleAndLeftoverPiece => {
                            match (orient_state == CycleOrientState::Canonical, share_anything) {
                                // We have a non-orienting cycle. This cycle can orient to satisfy any future orienting cycles.
                                (false, _) => {
                                    // 7+ on edges
                                    // (7+, 2+) on edges
                                    // We won't have a `true` canonical orient later
                                    // TODO^ does that make sense
                                    if !traverse_canonically_orients
                                        && orientation_prime_index != 64
                                        && unassigned_exponents_mask
                                            & (1 << orientation_prime_index)
                                            != 0
                                        && orientation_exps.prime_exponent(orientation_prime_index)
                                            >= register_order
                                                .prime_exponent(orientation_prime_index)
                                    {
                                        orient_state = CycleOrientState::Noncanonical;
                                    }
                                    OrientationSatisfiedBy::Satisfied
                                }
                                // We have an orienting cycle and no shared piece in this orbit. We need leftover pieces.
                                (true, false) => {
                                    // If we have no pieces left to be used as leftover then this is impossible. Note that this is satisfied when the orbit has no orientation sum constraint.
                                    if next_orbit_unused_piece_count == 0 {
                                        continue;
                                    }
                                    OrientationSatisfiedBy::LeftoverPiece
                                }
                                // We have an orienting cycle and a shared piece in this orbit. This satisfies any future orienting cycles.
                                (true, true) => OrientationSatisfiedBy::Satisfied,
                            }
                        }
                        OrientationSatisfiedBy::LeftoverPiece
                        | OrientationSatisfiedBy::Satisfied => OrientationSatisfiedBy::Satisfied,
                    },
                );
                trace!(
                    "{register_index} {orbit_index}: updated {old_orientation_satisfied_by:?} -> \
                     {:?}; assigned {prime} ({orbit_unused_piece_count} -> \
                     {next_orbit_unused_piece_count}); assign_cycle_orient {orient_state:?}",
                    *orientation_satisfied_by,
                );

                self.orbit_remaining_pieces[orbit_index2].unused = next_orbit_unused_piece_count;
                self.register_assignments[register_index2].unassigned_exponents_mask ^=
                    1 << prime_index2;
                if orient_state == CycleOrientState::Noncanonical {
                    self.register_assignments[register_index2].unassigned_exponents_mask ^=
                        1 << orientation_prime_index;
                }
                self.register_assignments[register_index2].cycle_assignments[prime_index2] =
                    PrimePowerCycleAssignment::Orbit(orbit_index, orient_state);

                let exists = self.recursive_backtrack(registers, register_index);
                if exists {
                    if !self.expansion {
                        return true;
                    } else if let SolutionExpansion::Limit(limit) = self.solution_expansion
                        && self.maybe_solutions.as_ref().is_some_and(
                            |CycleCombinationSolutions(solutions)| solutions.len() >= limit.get(),
                        )
                    {
                        return true;
                    }
                }
                trace!(
                    "{register_index} {orbit_index}: undo {old_orientation_satisfied_by:?} <- \
                     {:?}; unassigned {prime} (share state {:?})",
                    self.register_orbit_constraints[register_orbit_constraint_index]
                        .orientation_satisfied_by,
                    self.register_orbit_constraints[register_orbit_constraint_index]
                        .known_share_state
                );

                self.orbit_remaining_pieces[orbit_index2].unused = orbit_unused_piece_count;
                self.register_assignments[register_index2].unassigned_exponents_mask |=
                    1 << prime_index2;
                if orient_state == CycleOrientState::Noncanonical {
                    self.register_assignments[register_index2].unassigned_exponents_mask |=
                        1 << orientation_prime_index;
                }
                // We need to do this now because we don't guarantee to assign every cycle anymore
                self.register_assignments[register_index2].cycle_assignments[prime_index2] =
                    PrimePowerCycleAssignment::Unassigned;
                self.register_orbit_constraints[register_orbit_constraint_index]
                    .orientation_satisfied_by = old_orientation_satisfied_by;
            }
        }
        false
    }

    #[must_use]
    fn calculate(&mut self, registers: DisjointRegisters) -> SolutionsCalculation {
        self.register_orbit_constraints
            .clone_from_slice(&self.initial_register_orbit_constraints);
        self.orbit_remaining_pieces
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
        if self.expansion {
            self.recursive_backtrack(registers, 0);
            SolutionsCalculation::MaybeExpansion(self.maybe_solutions.take())
        } else {
            SolutionsCalculation::Existence(self.recursive_backtrack(registers, 0))
        }
    }

    pub fn existence(&mut self, registers: DisjointRegisters) -> bool {
        self.expansion = false;
        let SolutionsCalculation::Existence(exists) = self.calculate(registers) else {
            unreachable!();
        };
        exists
    }

    pub fn expansion(&mut self, registers: DisjointRegisters) -> Option<CycleCombinationSolutions> {
        self.expansion = true;
        let SolutionsCalculation::MaybeExpansion(maybe_solutions) = self.calculate(registers)
        else {
            unreachable!();
        };
        maybe_solutions
    }
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU16, sync::Arc, time::Instant};

    use humanize_duration::{Truncate, prelude::DurationExt};

    use crate::{
        cycle_combination_solutions::CycleCombinationSolutionsCalculator,
        cycle_combinations_tree::DisjointRegisters,
        finder::{
            CycleCombination, PossibleOrder, SolutionExpansion, mk_possible_orders_except_one,
        },
        nonemptyvec::NonemptySlice,
        orderexps::OrderExps,
        puzzle::{
            EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef,
            PuzzleDef, minxN::MINX3, possible_orders_len_cast,
        },
    };

    fn do_test<const N: usize>(
        mut solutions_calculator: CycleCombinationSolutionsCalculator<N>,
        puzzle_def: &PuzzleDef<N>,
        possible_orders_except_one: &[PossibleOrder<N>],
        register_orders: Vec<u64>,
        expected: &'static str,
    ) {
        let mut registers = register_orders
            .into_iter()
            .map(|register_order| {
                possible_orders_len_cast(
                    possible_orders_except_one
                        .iter()
                        .position(|possible_order| {
                            u64::try_from(possible_order.order.as_bigint()).unwrap()
                                == register_order
                        })
                        .unwrap(),
                )
            })
            .collect::<Box<[_]>>();
        registers.sort_by_key(|&r| std::cmp::Reverse(r));
        let registers = Arc::from(registers);
        let now = Instant::now();
        let solutions = solutions_calculator
            .expansion(DisjointRegisters::from(
                NonemptySlice::try_from(&*registers).unwrap(),
            ))
            .unwrap();
        println!("{}", now.elapsed().human(Truncate::Micro));
        let cycle_combination = CycleCombination {
            registers: Arc::clone(&registers),
            solutions,
        };

        let mut expected = expected.to_string();
        expected.retain(|c| !c.is_whitespace());
        let mut actual = cycle_combination.display_fmt(possible_orders_except_one, puzzle_def);
        let actual_copy = actual.clone();
        actual.retain(|c| !c.is_whitespace());

        assert_eq!(expected, actual, "\n{actual_copy}");
    }

    #[test_log::test]
    fn preassignment_1() {
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

        CycleCombinationSolutionsCalculator::new(
            &crazy,
            &[PossibleOrder {
                order: OrderExps::try_from(NonZeroU16::new(3).unwrap()).unwrap(),
                min_piece_count: 1.try_into().unwrap(),
            }],
            NonZeroU16::new(1).unwrap(),
            SolutionExpansion::All,
            None,
        )
        .existence(DisjointRegisters::from(
            NonemptySlice::try_from(&[0][..]).unwrap(),
        ));
    }

    #[test_log::test]
    fn minx3_optimal_3() {
        let minx3 = MINX3.clone();
        let possible_orders_except_one =
            mk_possible_orders_except_one(&minx3, minx3.possible_orders(None).unwrap());
        let solutions_calculator = CycleCombinationSolutionsCalculator::new(
            &minx3,
            &possible_orders_except_one,
            NonZeroU16::new(3).unwrap(),
            SolutionExpansion::All,
            None,
        );
        // 2520 630 420
        //
        // 2 2 2 3 3 5 7 : 4e 3c
        // 2     3 3 5 7 : 3c
        // 2 2   3   5 7 : 2e
        let register_orders = vec![2520, 630, 420];

        let expected = "
            2520: 0: (3+, 7) 1: (4+, 5)
             630: 0: (3+) 1: (5, 7+)
             420: 0: (5+) 1: (2+, 7)

            0: 1 ignored, 1 unused
            1: 0 ignored, 0 unused

            2520: 0: (3+, 5) 1: (4+, 7)
             630: 0: (3+) 1: (5, 7+)
             420: 0: (7+) 1: (2+, 5)

            0: 1 ignored, 1 unused
            1: 0 ignored, 0 unused

            2520: 0: (3+) 1: (4+, 5, 7)
             630: 0: (3+, 7) 1: (5+)
             420: 0: (5+) 1: (2+, 7)

            0: 1 ignored, 1 unused
            1: 0 ignored, 0 unused

            2520: 0: (3+) 1: (4+, 5, 7)
             630: 0: (3+, 5) 1: (7+)
             420: 0: (7+) 1: (2+, 5)

            0: 1 ignored, 1 unused
            1: 0 ignored, 0 unused
        ";

        do_test(
            solutions_calculator,
            &minx3,
            &possible_orders_except_one,
            register_orders,
            expected,
        );
    }

    #[test_log::test]
    fn minx3_equivalent_3() {
        let minx3 = MINX3.clone();
        let possible_orders_except_one =
            mk_possible_orders_except_one(&minx3, minx3.possible_orders(None).unwrap());
        let solutions_calculator = CycleCombinationSolutionsCalculator::new(
            &minx3,
            &possible_orders_except_one,
            NonZeroU16::new(3).unwrap(),
            SolutionExpansion::All,
            None,
        );
        // 840: 2 2 2 3 5 7
        let register_orders = vec![840, 840, 840];

        let expected = "
            840: 0: (7+) 1: (4+, 5)
            840: 0: (7+) 1: (4+, 5)
            840: 0: (5+) 1: (4+, 7)

            0: 0 ignored, 1 unused
            1: 0 ignored, 1 unused

            840: 0: (7+) 1: (4+, 5)
            840: 0: (5+) 1: (4+, 7)
            840: 0: (7+) 1: (4+, 5)

            0: 0 ignored, 1 unused
            1: 0 ignored, 1 unused

            840: 0: (5+) 1: (4+, 7)
            840: 0: (7+) 1: (4+, 5)
            840: 0: (7+) 1: (4+, 5)

            0: 0 ignored, 1 unused
            1: 0 ignored, 1 unused
        ";

        do_test(
            solutions_calculator,
            &minx3,
            &possible_orders_except_one,
            register_orders,
            expected,
        );
    }

    #[test_log::test]
    fn orienting_3_cycle() {
        let crazy = PuzzleDef::<64>::new(
            vec![PartialOrbitDef {
                piece_count: 4.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 2,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            }],
            EvenParityConstraints(vec![vec![]]),
        )
        .unwrap();
        let possible_orders_except_one =
            mk_possible_orders_except_one(&crazy, crazy.possible_orders(None).unwrap());
        let solutions_calculator = CycleCombinationSolutionsCalculator::new(
            &crazy,
            &possible_orders_except_one,
            NonZeroU16::new(1).unwrap(),
            SolutionExpansion::All,
            None,
        );
        // 840: 2 2 2 3 5 7
        let register_orders = vec![6];

        let expected = "
            6: 0: (3+)

            0: 0 ignored, 1 unused
        ";

        do_test(
            solutions_calculator,
            &crazy,
            &possible_orders_except_one,
            register_orders,
            expected,
        );
    }
}

use std::{
    cmp::Ordering,
    fmt::Debug,
    num::NonZeroUsize,
    simd::{Mask, Simd, cmp::SimdPartialEq},
    sync::{
        Arc,
        atomic::{self, AtomicUsize},
        nonpoison::Mutex,
    },
};

use log::{Level, debug, log_enabled, trace};

use crate::{
    FIRST_65_PRIMES,
    cycle_combinations_tree::{DisjointRegisters, dbg_registers},
    finder::{
        CycleCombination, PossibleOrder, ValidatedCycleCombinationFinder,
        ValidatedSolutionExpansion,
    },
    nonemptyvec::NonemptySlice,
    orderexps::OrderExps,
    puzzle::{OrientationStatus, OrientationSumConstraint, orbit_index_cast},
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

#[non_exhaustive]
pub struct CycleCombinationSolutionsCalculator<'a, const N: usize> {
    expansion: bool,
    register_index: u16,

    maybe_solutions: Option<CycleCombinationSolutions>,
    fitting_tries: u32,

    /// Map of every register, to its cycles, to which orbit its prime power
    /// component is assigned to and bitmask
    register_assignments: Box<[RegisterCycleAssignments<N>]>,
    register_orbit_constraints: Box<[RegisterOrbitConstraint]>,
    /// Remaining piece count for every orbit
    orbit_remaining_pieces: Box<[OrbitRemainingPieces]>,
    // /// Gives the best registers (register index, the exponent)
    // register_exponent_sorter: Vec<(u16, u8)>,
    // /// Gives the best orientation orders
    // best_orientations_queue: [BestOrientation; 9],
    ccf: &'a ValidatedCycleCombinationFinder<'a, N>,
    immutable: CycleCombinationSolutionsCalculatorImmutable<'a, N>,
}

#[derive(Debug)]
#[non_exhaustive]
pub struct CycleCombinationSolutionsCalculatorImmutable<'a, const N: usize> {
    // TODO: make this one orbit defs long
    initial_register_orbit_constraints: Box<[RegisterOrbitConstraint]>,
    initial_orbit_remaining_piece_counts: Box<[OrbitRemainingPieces]>,
    possible_orders_except_one: &'a [PossibleOrder<N>],
    orientations_exps_mask: u64,
}

#[derive(PartialEq, Clone, Copy, Debug, Default)]
enum ShareState {
    #[default]
    None,
    Orientation,
    Parity,
}

// The Ord implementation is not semantically meaningful; we just need some
// total order
#[derive(Clone, Copy, Debug, Ord, Eq, PartialEq, PartialOrd)]
enum OrientationSatisfiedBy {
    NoConstraint,
    LeftoverPiece(Option<u8>),
    SharedPieces(Option<u8>),
    RegisterCycle,
}

#[derive(Clone, Copy, Debug, Ord, Eq, PartialEq, PartialOrd)]
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct RegisterOrbitConstraint {
    known_share_state: ShareState,
    orientation_satisfied_by: OrientationSatisfiedBy,
    foo: bool,
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
    unused_piece_count: u16,
    orientation_exps: &'a OrderExps<N>,
    register_orbit_constraint: RegisterOrbitConstraint,
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

impl ShareState {
    fn required_ignored_pieces(self) -> u16 {
        match self {
            ShareState::None => 0,
            ShareState::Orientation => 1,
            ShareState::Parity => 2,
        }
    }
}

impl CycleOrientState {
    fn must_orient(self) -> bool {
        match self {
            CycleOrientState::None => false,
            CycleOrientState::Canonical | CycleOrientState::Noncanonical => true,
        }
    }
}

impl<const N: usize> OrbitTraversalState<'_, N> {
    fn cmp(&self, other: &Self) -> Ordering {
        let &OrbitTraversalState {
            unused_piece_count,
            orientation_exps,
            register_orbit_constraint,
        } = self;
        let &OrbitTraversalState {
            unused_piece_count: unused_piece_count_1,
            orientation_exps: orientation_exps_1,
            register_orbit_constraint: register_orbit_constraint_1,
        } = other;
        unused_piece_count
            .cmp(&unused_piece_count_1)
            .then(orientation_exps.cmp(orientation_exps_1))
            .then(
                register_orbit_constraint
                    .known_share_state
                    .required_ignored_pieces()
                    .cmp(
                        &register_orbit_constraint_1
                            .known_share_state
                            .required_ignored_pieces(),
                    ),
            )
        // TODO: NO WE DONT??????/
        // // We need this. Assume both orbits share nothing
        // //
        // // o1:
        // // 10 pieces
        // // 2+ cycle
        // //
        // // o2:
        // // 8 pieces
        // //
        // // We can't treat them the same. Placing a 3 cycle would require a
        // leftover piece for // the second orbit but not the first
        // .then(
        //     register_orbit_constraint
        //         .orientation_satisfied_by
        //         .cmp(&register_orbit_constraint_1.orientation_satisfied_by),
        // )
    }
}

impl<'a, const N: usize> ValidatedCycleCombinationFinder<'a, N> {
    #[must_use]
    pub(crate) fn solutions_calculator(
        &'a self,
        possible_orders_except_one: &'a [PossibleOrder<N>],
    ) -> CycleCombinationSolutionsCalculator<'a, N> {
        let register_assignments = vec![
            RegisterCycleAssignments {
                all_exponents_mask: 0,
                unassigned_exponents_mask: !0,
                // unassigned_exponents_mask: !0,
                cycle_assignments: [PrimePowerCycleAssignment::Unassigned; N],
            };
            NonZeroUsize::from(self.register_count).get()
        ]
        .into_boxed_slice();
        let orbit_defs = self.puzzle_def.orbit_defs();
        let register_orbit_constraints = (0..NonZeroUsize::from(self.register_count).get())
            .flat_map(|_| {
                orbit_defs.iter().map(|&orbit_def| {
                    let orientation_satisfied_by = if matches!(
                        orbit_def.orientation,
                        OrientationStatus::CanOrient {
                            count: _,
                            sum_constraint: OrientationSumConstraint::Zero
                        }
                    ) {
                        OrientationSatisfiedBy::NoConstraint
                    } else {
                        OrientationSatisfiedBy::RegisterCycle
                    };
                    RegisterOrbitConstraint {
                        known_share_state: ShareState::default(),
                        orientation_satisfied_by,
                        foo: false,
                    }
                })
            })
            .collect::<Box<[_]>>();
        let initial_register_orbit_constraints = Box::clone_from_ref(
            &register_orbit_constraints[..self.puzzle_def.orbit_defs().len().get()],
        );
        let orbit_remaining_pieces = orbit_defs
            .iter()
            .map(|orbit_def| OrbitRemainingPieces {
                unused: orbit_def.piece_count.get(),
                ignored: 0,
            })
            .collect::<Box<[_]>>();
        let initial_orbit_remaining_piece_counts = orbit_remaining_pieces.clone();
        let orientations_exps_mask = self
            .puzzle_def
            .orientations_exps_lcm()
            .0
            .simd_ne(Simd::splat(0))
            .to_bitmask();
        // let register_exponent_sorter =
        //     Vec::with_capacity(NonZeroUsize::from(exact_register_count).get());
        // let best_orientations_queue = [BestOrientation::Unassigned; 9];
        CycleCombinationSolutionsCalculator {
            register_index: 0,
            maybe_solutions: None,
            register_assignments,
            register_orbit_constraints,
            orbit_remaining_pieces,
            expansion: false,
            fitting_tries: 0,
            ccf: self,
            // register_exponent_sorter,
            // best_orientations_queue,
            immutable: CycleCombinationSolutionsCalculatorImmutable {
                initial_register_orbit_constraints,
                initial_orbit_remaining_piece_counts,
                possible_orders_except_one,
                orientations_exps_mask,
            },
        }
    }
}

impl<const N: usize> CycleCombinationSolutionsCalculator<'_, N> {
    // TODO: inline this more for previous calls
    fn recursive_backtrack(&mut self, registers: DisjointRegisters) -> bool {
        if let Some(max_fitting_tries) = self.ccf.maybe_max_fitting_tries {
            if self.fitting_tries == max_fitting_tries {
                return false;
            }
            self.fitting_tries += 1;
        } else if log_enabled!(Level::Debug) {
            self.fitting_tries += 1;
        }
        let register_index2 = usize::from(self.register_index);
        let unassigned_exponents_mask =
            self.register_assignments[register_index2].unassigned_exponents_mask;
        if unassigned_exponents_mask == 0 {
            // we do not have to care about the mod 2 case; when we have more than 1 cycle
            // we can always fit by just picking two to orient
            let leaf = register_index2 == self.register_assignments.len() - 1;

            trace!(
                "before: {:?} {:?}",
                self.orbit_remaining_pieces, self.register_orbit_constraints,
            );

            let (orbits_constraints, next_orbits_constraints) = self.register_orbit_constraints
                [register_index2 * self.ccf.puzzle_def.orbit_defs().len().get()..]
                .split_at_mut(self.ccf.puzzle_def.orbit_defs().len().get());
            let mut orbits_unused_piece_count_sum = 0;
            let mut invalid = false;
            for (
                orbit_index2,
                (
                    &mut RegisterOrbitConstraint {
                        ref mut known_share_state,
                        orientation_satisfied_by,
                        ref mut foo,
                    },
                    orbit_remaining_piece,
                ),
            ) in orbits_constraints
                .iter_mut()
                .zip(self.orbit_remaining_pieces.iter_mut())
                .enumerate()
            {
                let prev_known_share_state = *known_share_state;
                // Promote only if we have no other share rn
                match orientation_satisfied_by {
                    OrientationSatisfiedBy::LeftoverPiece(
                        maybe_noncanonically_orienting_prime_index,
                    ) => {
                        if *known_share_state == ShareState::None {
                            *known_share_state = ShareState::Orientation;
                        }
                        if let Some(noncanonically_orienting_prime_index) =
                            maybe_noncanonically_orienting_prime_index
                        {
                            trace!("(leftover) assigning piece to orbit {orbit_index2}");

                            self.register_assignments[register_index2].cycle_assignments
                                [usize::from(noncanonically_orienting_prime_index)] =
                                PrimePowerCycleAssignment::Orbit(
                                    orbit_index_cast(orbit_index2),
                                    CycleOrientState::Canonical,
                                );
                            if let Some(next_orbit_unused_piece_count) =
                                orbit_remaining_piece.unused.checked_sub(1)
                            {
                                *foo = true;
                                orbit_remaining_piece.unused = next_orbit_unused_piece_count;
                            } else if leaf {
                                return false;
                            } else {
                                invalid = true;
                            }
                        }
                    }
                    OrientationSatisfiedBy::SharedPieces(Some(
                        noncanonically_orienting_prime_index,
                    )) => {
                        trace!("(shared) assigning piece to orbit {orbit_index2}");

                        self.register_assignments[register_index2].cycle_assignments
                            [usize::from(noncanonically_orienting_prime_index)] =
                            PrimePowerCycleAssignment::Orbit(
                                orbit_index_cast(orbit_index2),
                                CycleOrientState::Canonical,
                            );
                        if let Some(next_orbit_unused_piece_count) =
                            orbit_remaining_piece.unused.checked_sub(1)
                        {
                            *foo = true;
                            orbit_remaining_piece.unused = next_orbit_unused_piece_count;
                        } else if leaf {
                            return false;
                        } else {
                            invalid = true;
                        }
                    }
                    _ => (),
                }
                orbit_remaining_piece.unused -= known_share_state.required_ignored_pieces()
                    - prev_known_share_state.required_ignored_pieces();
                if let Some(RegisterOrbitConstraint {
                    known_share_state: next_share_state,
                    ..
                }) = next_orbits_constraints.get_mut(orbit_index2)
                {
                    *next_share_state = *known_share_state;
                }
                orbits_unused_piece_count_sum += u32::from(orbit_remaining_piece.unused);
            }

            trace!(
                "{register_index2} after: {:?} {:?}",
                self.orbit_remaining_pieces, self.register_orbit_constraints,
            );

            if !leaf {
                self.register_index += 1;
                let next_register = registers
                    .get_order(
                        self.register_index,
                        self.immutable.possible_orders_except_one,
                    )
                    .unwrap();
                let found = if u32::from(next_register.min_piece_count.get())
                    > orbits_unused_piece_count_sum
                    || invalid
                {
                    false
                } else {
                    self.recursive_backtrack(registers)
                };
                self.register_index -= 1;

                if let Some(prev_register_index2) = register_index2.checked_sub(1) {
                    let (prev_orbits_constraints, orbits_constraints) = self
                        .register_orbit_constraints
                        [prev_register_index2 * self.ccf.puzzle_def.orbit_defs().len().get()..]
                        .split_at_mut(self.ccf.puzzle_def.orbit_defs().len().get());
                    for (
                        (
                            &RegisterOrbitConstraint {
                                known_share_state: prev_known_share_state,
                                orientation_satisfied_by: _,
                                foo: _,
                            },
                            &mut RegisterOrbitConstraint {
                                ref mut known_share_state,
                                orientation_satisfied_by,
                                ref mut foo,
                            },
                        ),
                        orbit_remaining_piece,
                    ) in prev_orbits_constraints
                        .iter()
                        .zip(orbits_constraints)
                        .zip(self.orbit_remaining_pieces.iter_mut())
                    {
                        orbit_remaining_piece.unused += known_share_state.required_ignored_pieces()
                            - prev_known_share_state.required_ignored_pieces();
                        *known_share_state = prev_known_share_state;

                        match orientation_satisfied_by {
                            OrientationSatisfiedBy::LeftoverPiece(Some(
                                noncanonically_orienting_prime_index,
                            ))
                            | OrientationSatisfiedBy::SharedPieces(Some(
                                noncanonically_orienting_prime_index,
                            )) => {
                                self.register_assignments[register_index2].cycle_assignments
                                    [usize::from(noncanonically_orienting_prime_index)] =
                                    PrimePowerCycleAssignment::Unassigned;
                                if *foo {
                                    orbit_remaining_piece.unused += 1;
                                    *foo = false;
                                }
                            }
                            _ => (),
                        }
                    }
                } else {
                    for (
                        &mut RegisterOrbitConstraint {
                            ref mut known_share_state,
                            orientation_satisfied_by,
                            ref mut foo,
                        },
                        orbit_remaining_piece,
                    ) in self
                        .register_orbit_constraints
                        .iter_mut()
                        .take(self.ccf.puzzle_def.orbit_defs().len().get())
                        .zip(self.orbit_remaining_pieces.iter_mut())
                    {
                        orbit_remaining_piece.unused += known_share_state.required_ignored_pieces();
                        *known_share_state = ShareState::default();
                        match orientation_satisfied_by {
                            OrientationSatisfiedBy::LeftoverPiece(Some(
                                noncanonically_orienting_prime_index,
                            ))
                            | OrientationSatisfiedBy::SharedPieces(Some(
                                noncanonically_orienting_prime_index,
                            )) => {
                                self.register_assignments[register_index2].cycle_assignments
                                    [usize::from(noncanonically_orienting_prime_index)] =
                                    PrimePowerCycleAssignment::Unassigned;
                                if *foo {
                                    orbit_remaining_piece.unused += 1;
                                    *foo = false;
                                }
                            }
                            _ => (),
                        }
                    }
                }

                return found;
            }

            if self.expansion {
                trace!("{:#?}", self.register_assignments);
                // TODO: allocator
                let mut register_orbit_cycles =
                    vec![
                        vec![];
                        NonZeroUsize::from(self.ccf.register_count).get()
                            * self.ccf.puzzle_def.orbit_defs().len().get()
                    ]
                    .into_boxed_slice();
                for register_index in 0..self.ccf.register_count.get() {
                    let register_index2 = usize::from(register_index);
                    let register_order = &registers
                        .get_order(register_index, self.immutable.possible_orders_except_one)
                        .unwrap()
                        .order;
                    let register_assignment = &self.register_assignments[register_index2];
                    let mut all_exponents = register_assignment.all_exponents_mask;
                    while all_exponents != 0 {
                        let prime_index = all_exponents.trailing_zeros() as usize;
                        let prime = FIRST_65_PRIMES[prime_index];
                        let register_order_exp = register_order.prime_exponent(prime_index);
                        // We can have a 7+ on edges to serve as following the 2 cycle and a 7
                        // cycle, in the false case
                        if let PrimePowerCycleAssignment::Orbit(orbit_index, orient_state) =
                            register_assignment.cycle_assignments[prime_index]
                        {
                            let orbit_index2 = usize::from(orbit_index);
                            let orientation_exps =
                                &self.ccf.puzzle_def.orientations_exps()[orbit_index2];
                            let exp = if orient_state == CycleOrientState::Canonical {
                                let orientation_exp = orientation_exps.prime_exponent(prime_index);
                                register_order_exp.saturating_sub(orientation_exp)
                            } else {
                                register_order_exp
                            };
                            let register_orbit_index = register_index2
                                * self.ccf.puzzle_def.orbit_defs().len().get()
                                + orbit_index2;
                            let cycle_piece_count = prime.pow(u32::from(exp));
                            register_orbit_cycles[register_orbit_index].push(Cycle {
                                piece_count: cycle_piece_count,
                                must_orient: orient_state.must_orient(),
                            });
                        }
                        all_exponents ^= all_exponents.isolate_lowest_one();
                    }
                    for (orbit_index2, register_orbit_cycle) in register_orbit_cycles
                        .iter_mut()
                        .skip(register_index2 * self.ccf.puzzle_def.orbit_defs().len().get())
                        .take(self.ccf.puzzle_def.orbit_defs().len().get())
                        .enumerate()
                    {
                        register_orbit_cycle
                            .sort_unstable_by_key(|&Cycle { piece_count, .. }| piece_count);
                        // only the last register has the most recent share state propagation
                        if register_index == self.ccf.register_count.get() - 1 {
                            let register_orbit_index = register_index2
                                * self.ccf.puzzle_def.orbit_defs().len().get()
                                + orbit_index2;
                            self.orbit_remaining_pieces[orbit_index2].ignored = self
                                .register_orbit_constraints[register_orbit_index]
                                .known_share_state
                                .required_ignored_pieces();
                        }
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
            trace!("Solution!");
            return true;
        }

        // pop the highest one so we fit large primes first; higher chance of reaching
        // the fail state
        let orienting_unassigned_exponents_mask =
            unassigned_exponents_mask & self.immutable.orientations_exps_mask;
        // There is a good reason to visit primes with an orbit with that orientation
        // factor first. Recall how every canonically orienting cycle also has an
        // iteration where it does not canonically orient. Also recall how, in the case
        // we want to optimize for, we are just checking for existence. If we have an
        // orienting cycle at the root node, we never have to make it unorient which can
        // be unoptimal
        let prime_index2 = if orienting_unassigned_exponents_mask == 0 {
            unassigned_exponents_mask.ilog2() as usize
        } else {
            orienting_unassigned_exponents_mask.ilog2() as usize
        };
        let prime = FIRST_65_PRIMES[prime_index2];

        // Nonzero because it is in the unassigned mask
        let register_order = &registers
            .get_order(
                self.register_index,
                self.immutable.possible_orders_except_one,
            )
            .unwrap()
            .order;
        let register_order_exp = register_order.prime_exponent(prime_index2);

        let mut traverse_orients = orienting_unassigned_exponents_mask != 0;
        let mut maybe_prev_traversal_state: Option<OrbitTraversalState<N>> = None;
        let orientations_exps = self.ccf.puzzle_def.orientations_exps();
        loop {
            let Some((orbit_index2, orbit_traversal_state)) = orientations_exps
                .iter()
                .zip(&self.orbit_remaining_pieces)
                .enumerate()
                .fold(
                    None,
                    |acc, (orbit_index2, (orientation_exps, &orbit_remaining_pieces))| {
                        let orientation_prime_index = orientation_exps
                            .0
                            .simd_ne(Simd::splat(0))
                            .to_bitmask()
                            .trailing_zeros()
                            as usize;
                        let orients = orientation_prime_index == prime_index2;
                        trace!(
                            "orients={orients}; orientation_prime={}; p={prime}",
                            FIRST_65_PRIMES[orientation_prime_index]
                        );
                        if traverse_orients != orients {
                            return acc;
                        }
                        let register_orbit_constraint = self.register_orbit_constraints
                            [register_index2 * self.ccf.puzzle_def.orbit_defs().len().get()
                                + orbit_index2];
                        // fast check to make sure the nonorienting prime fits into this orbit
                        if !traverse_orients
                            && f64::from(register_order_exp) * f64::from(prime).ln()
                                > f64::from(orbit_remaining_pieces.unused).ln()
                        {
                            return acc;
                        }
                        let curr = OrbitTraversalState {
                            unused_piece_count: orbit_remaining_pieces.unused,
                            orientation_exps,
                            register_orbit_constraint,
                        };
                        // Filter out everything <= the previous traversal state
                        if maybe_prev_traversal_state.is_some_and(|prev| curr.cmp(&prev).is_le()) {
                            acc
                        // curr is > the previous traversal state. If we have a
                        // previous best and curr < min, or if there was no
                        // previous best, swap it out with curr.
                        } else if acc.as_ref().is_none_or(|(_, min)| curr.cmp(min).is_lt()) {
                            Some((orbit_index2, curr))
                        } else {
                            acc
                        }
                    },
                )
            else {
                if !traverse_orients {
                    trace!("reg {register_index2} finished traversing");
                    break;
                }
                trace!("reg {register_index2} traversing nonorienting");
                traverse_orients = false;
                maybe_prev_traversal_state = None;
                continue;
            };
            maybe_prev_traversal_state = Some(orbit_traversal_state);
            trace!("{register_index2} {orbit_index2}; p={prime}");

            let OrbitTraversalState {
                orientation_exps, ..
            } = orbit_traversal_state;

            let orbit_index = orbit_index_cast(orbit_index2);
            let register_orbit_constraint_index =
                register_index2 * self.ccf.puzzle_def.orbit_defs().len().get() + orbit_index2;

            let orientation_exp = orientation_exps.prime_exponent(prime_index2);
            let orbit_unused_piece_count = self.orbit_remaining_pieces[orbit_index2].unused;

            // TODO: do parity by checking ParityConstraint::Even first in OrbitDef
            // put in the 2 cycle case here
            let orient_states = if traverse_orients {
                &[CycleOrientState::Canonical][..]
            } else {
                &[CycleOrientState::None][..]
            };
            for &(mut orient_state) in orient_states {
                let RegisterOrbitConstraint {
                    known_share_state,
                    orientation_satisfied_by,
                    foo: _,
                } = &mut self.register_orbit_constraints[register_orbit_constraint_index];
                let mut exp = register_order_exp;
                let cycle_piece_count = if orient_state == CycleOrientState::Canonical {
                    exp = exp.saturating_sub(orientation_exp);
                    if exp == 0 {
                        0
                    } else {
                        prime.pow(u32::from(exp))
                    }
                } else {
                    // exp unused if not canonical
                    prime.pow(u32::from(register_order_exp))
                };
                let Some(next_orbit_unused_piece_count) =
                    orbit_unused_piece_count.checked_sub(cycle_piece_count)
                else {
                    trace!(
                        "{register_index2} {orbit_index} failed: {orbit_unused_piece_count} < \
                         {cycle_piece_count}; tried {prime}; orient state {orient_state:?}",
                    );
                    continue;
                };

                // Do we already share something?
                // TODO: do we need to visit 2s first, or last, or it doesnt matter?
                let share_anything = known_share_state.required_ignored_pieces() > 0;

                let old_orientation_satisfied_by = *orientation_satisfied_by;
                let old_known_share_state = *known_share_state;
                *orientation_satisfied_by = match old_orientation_satisfied_by {
                    OrientationSatisfiedBy::NoConstraint => {
                        match (orient_state == CycleOrientState::Canonical, share_anything) {
                            // We have a non-orienting cycle and no shared pieces in this orbit. We
                            // must have added an orienting cycle if we have one. We need to
                            // indicate that we no longer need leftover pieces.
                            // We have a non-orienting cycle and a shared piece in this orbit. We
                            // must have added an orienting cycle if we have one. We need to
                            // indicate that we no longer need leftover pieces.
                            (false, _) => OrientationSatisfiedBy::RegisterCycle,
                            // We have an orienting cycle and no shared piece in this orbit. We need
                            // leftover pieces.
                            (true, false) => {
                                // If we have no pieces left to be used as leftover then this is
                                // impossible. Note that this is satisfied when the orbit has no
                                // orientation sum constraint.
                                if next_orbit_unused_piece_count == 0 {
                                    continue;
                                }
                                let maybe_noncanonically_orienting_prime_index = if exp == 0 {
                                    // prime indicies are <=255
                                    #[allow(clippy::cast_possible_truncation)]
                                    Some(prime_index2 as u8)
                                } else {
                                    None
                                };
                                OrientationSatisfiedBy::LeftoverPiece(
                                    maybe_noncanonically_orienting_prime_index,
                                )
                            }
                            // We have an orienting cycle and a shared piece in this orbit. The
                            // shared piece satisfies any future orienting cycles.
                            (true, true) => {
                                let maybe_noncanonically_orienting_prime_index = if exp == 0 {
                                    // prime indicies are <=255
                                    #[allow(clippy::cast_possible_truncation)]
                                    Some(prime_index2 as u8)
                                } else {
                                    None
                                };
                                OrientationSatisfiedBy::SharedPieces(
                                    maybe_noncanonically_orienting_prime_index,
                                )
                            }
                        }
                    }
                    OrientationSatisfiedBy::LeftoverPiece(Some(_)) => {
                        // The noncanonically orienting cycle takes up  we have no pieces left to be
                        // used as leftover then this is impossible. Note that this is satisfied
                        // when the orbit has no orientation sum constraint.
                        // There is no shared piece in the LeftoverPiece case so we can use unused
                        if next_orbit_unused_piece_count == 0 {
                            continue;
                        }
                        orient_state = CycleOrientState::Noncanonical;
                        OrientationSatisfiedBy::LeftoverPiece(None)
                    }
                    OrientationSatisfiedBy::SharedPieces(Some(_)) => {
                        orient_state = CycleOrientState::Noncanonical;
                        OrientationSatisfiedBy::RegisterCycle
                    }
                    OrientationSatisfiedBy::LeftoverPiece(None)
                    | OrientationSatisfiedBy::SharedPieces(None)
                    | OrientationSatisfiedBy::RegisterCycle => {
                        OrientationSatisfiedBy::RegisterCycle
                    }
                };

                trace!(
                    "{register_index2} {orbit_index}: updated {old_orientation_satisfied_by:?} -> \
                     {:?}; assigned {prime} ({orbit_unused_piece_count} -> \
                     {next_orbit_unused_piece_count}); orient_state {orient_state:?}",
                    *orientation_satisfied_by,
                );

                self.orbit_remaining_pieces[orbit_index2].unused = next_orbit_unused_piece_count;
                self.register_assignments[register_index2].unassigned_exponents_mask ^=
                    1 << prime_index2;
                if orient_state != CycleOrientState::Canonical || exp != 0 {
                    self.register_assignments[register_index2].cycle_assignments[prime_index2] =
                        PrimePowerCycleAssignment::Orbit(orbit_index, orient_state);
                }

                let exists = self.recursive_backtrack(registers);
                if exists {
                    if !self.expansion {
                        return true;
                    } else if let ValidatedSolutionExpansion::Limit(limit) =
                        self.ccf.solution_expansion
                        && self.maybe_solutions.as_ref().is_some_and(
                            |CycleCombinationSolutions(solutions)| solutions.len() >= limit.get(),
                        )
                    {
                        return true;
                    }
                }
                trace!(
                    "{register_index2} {orbit_index}: undo {old_orientation_satisfied_by:?} <- \
                     {:?}; unassigned {prime} ({} -> {}) (share state {:?})",
                    self.register_orbit_constraints[register_orbit_constraint_index]
                        .orientation_satisfied_by,
                    self.orbit_remaining_pieces[orbit_index2].unused,
                    orbit_unused_piece_count,
                    self.register_orbit_constraints[register_orbit_constraint_index]
                        .known_share_state
                );

                self.orbit_remaining_pieces[orbit_index2].unused = orbit_unused_piece_count;
                self.register_assignments[register_index2].unassigned_exponents_mask |=
                    1 << prime_index2;
                // We need to do this now because we don't guarantee to assign every cycle
                // anymore
                if orient_state != CycleOrientState::Canonical || exp != 0 {
                    self.register_assignments[register_index2].cycle_assignments[prime_index2] =
                        PrimePowerCycleAssignment::Unassigned;
                }
                self.register_orbit_constraints[register_orbit_constraint_index]
                    .orientation_satisfied_by = old_orientation_satisfied_by;
                self.register_orbit_constraints[register_orbit_constraint_index]
                    .known_share_state = old_known_share_state;
            }
        }
        false
    }

    #[must_use]
    fn calculate(&mut self, registers: DisjointRegisters) -> SolutionsCalculation {
        self.register_orbit_constraints[..self.ccf.puzzle_def.orbit_defs().len().get()]
            .clone_from_slice(&self.immutable.initial_register_orbit_constraints);
        self.orbit_remaining_pieces
            .clone_from_slice(&self.immutable.initial_orbit_remaining_piece_counts);

        // Every prime used by the register orders
        let mut orienting_registers_prime_mask = Mask::splat(false);

        for (register_index, possible_order) in registers
            .iter_orders(self.immutable.possible_orders_except_one)
            .enumerate()
        {
            let all_exponents = possible_order.order.0.simd_ne(Simd::splat(0));
            self.register_assignments[register_index].all_exponents_mask =
                all_exponents.to_bitmask();
            self.register_assignments[register_index].unassigned_exponents_mask =
                all_exponents.to_bitmask();
            orienting_registers_prime_mask |= all_exponents;
        }
        // TODO: if an orbit has at least the first highest cycle + second highest cycle
        // number of pieces, we will never not satisfy an orientation constraint
        self.fitting_tries = 0;
        if self.expansion {
            self.recursive_backtrack(registers);
            debug!(
                "Solution for {} in {} tries",
                dbg_registers(registers.iter(), self.immutable.possible_orders_except_one),
                self.fitting_tries
            );

            SolutionsCalculation::MaybeExpansion(self.maybe_solutions.take())
        } else {
            let existence = self.recursive_backtrack(registers);
            if existence {
                debug!(
                    "Solution for {} in {} tries",
                    dbg_registers(registers.iter(), self.immutable.possible_orders_except_one),
                    self.fitting_tries
                );
            }
            SolutionsCalculation::Existence(existence)
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

pub fn expand_possible_register<const N: usize>(
    solutions_calculator: &mut CycleCombinationSolutionsCalculator<N>,
    possible_registers: Arc<[u32]>,
    expansion_percent_done: &AtomicUsize,
    logged_bucket: &Mutex<usize>,
    possible_registers_len: usize,
) -> CycleCombination {
    #[allow(clippy::missing_panics_doc)]
    let possible_registers2 = DisjointRegisters::from(
        NonemptySlice::try_from(&*possible_registers).expect("The number of registers is non-zero"),
    );
    #[allow(clippy::missing_panics_doc)]
    let solutions = solutions_calculator
        .expansion(possible_registers2)
        .expect("This solution is in the front and therefore exists");
    let cycle_combination = CycleCombination {
        registers: possible_registers,
        solutions,
    };

    if log_enabled!(Level::Debug) {
        const PERCENT: usize = 1;

        let done = expansion_percent_done.fetch_add(1, atomic::Ordering::Relaxed) + 1;
        let new_bucket = done * 100 / (PERCENT * possible_registers_len);
        let mut bucket = logged_bucket.lock();
        if new_bucket > *bucket {
            *bucket = new_bucket;
            debug!("Expansion: {}%", done * 100 / possible_registers_len);
        }
    }

    cycle_combination
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU16, sync::Arc, time::Instant};

    use humanize_duration::{Truncate, prelude::DurationExt};

    use crate::{
        cycle_combination_solutions::CycleCombinationSolutionsCalculator,
        cycle_combinations_tree::{DisjointRegisters, dbg_registers},
        finder::{
            CycleCombination, CycleCombinationFinder, PossibleOrder, SolutionExpansion,
            mk_possible_orders_except_one,
        },
        nonemptyvec::NonemptySlice,
        orderexps::OrderExps,
        puzzle::{
            EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef,
            PuzzleDef,
            minxN::{MINX3, MINX5},
            possible_orders_len_cast,
        },
    };

    fn do_test<const N: usize>(
        mut solutions_calculator: CycleCombinationSolutionsCalculator<N>,
        register_orders: Vec<u64>,
        expected: &'static str,
    ) {
        let mut registers = register_orders
            .into_iter()
            .map(|register_order| {
                possible_orders_len_cast(
                    solutions_calculator
                        .immutable
                        .possible_orders_except_one
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
        let solutions = solutions_calculator.expansion(DisjointRegisters::from(
            NonemptySlice::try_from(&*registers).unwrap(),
        ));
        println!("{}", now.elapsed().human(Truncate::Micro));
        let cycle_combination = CycleCombination {
            registers: Arc::clone(&registers),
            solutions: solutions.unwrap(),
        };

        let mut expected = expected.to_string();
        expected.retain(|c| !c.is_whitespace());
        let mut actual = cycle_combination.solutions_fmt(
            solutions_calculator.immutable.possible_orders_except_one,
            solutions_calculator.ccf.puzzle_def,
        );
        let actual_copy = actual.clone();
        actual.retain(|c| !c.is_whitespace());

        assert_eq!(expected, actual, "\n{actual_copy}");
    }

    #[test_log::test]
    fn preassignment_1() {
        let crazy = PuzzleDef::<32>::new((
            vec![
                PartialOrbitDef {
                    name: None,
                    piece_count: 5.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 27,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    name: None,
                    piece_count: 5.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 9,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
            ],
            EvenParityConstraints(vec![vec![0, 1]]),
        ))
        .unwrap();

        let ccf = CycleCombinationFinder::builder()
            .with_puzzle_def(&crazy)
            .with_register_count(1)
            .with_solution_expansion(SolutionExpansion::All)
            .with_max_fitting_tries(None)
            .validate()
            .unwrap();
        ccf.solutions_calculator(&[PossibleOrder {
            order: OrderExps::try_from(NonZeroU16::new(3).unwrap()).unwrap(),
            min_piece_count: 1.try_into().unwrap(),
        }])
        .existence(DisjointRegisters::from(
            NonemptySlice::try_from(&[0][..]).unwrap(),
        ));
    }

    #[test_log::test]
    fn minx3_optimal_3() {
        let minx3 = MINX3.clone();
        let ccf = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx3)
            .with_register_count(3)
            .with_solution_expansion(SolutionExpansion::All)
            .with_max_fitting_tries(None)
            .validate()
            .unwrap();
        let possible_orders_except_one =
            mk_possible_orders_except_one(&minx3, minx3.possible_orders(None).unwrap());
        let solutions_calculator = ccf.solutions_calculator(&possible_orders_except_one);
        // 2520 630 420
        //
        // 2 2 2 3 3 5 7 : 4e 3c
        // 2     3 3 5 7 : 3c
        // 2 2   3   5 7 : 2e
        let register_orders = vec![2520, 630, 420];

        let expected = "
            2520: c: (3+, 7) e: (4+, 5)
             630: c: (3+) e: (5, 7+)
             420: c: (5+) e: (2+, 7)

            c: 1 ignored, 1 unused
            e: 0 ignored, 0 unused

            2520: c: (3+, 5) e: (4+, 7)
             630: c: (3+) e: (5, 7+)
             420: c: (7+) e: (2+, 5)

            c: 1 ignored, 1 unused
            e: 0 ignored, 0 unused
        ";

        do_test(solutions_calculator, register_orders, expected);
    }

    #[test_log::test]
    fn minx3_equivalent_3() {
        let minx3 = MINX3.clone();
        let possible_orders_except_one =
            mk_possible_orders_except_one(&minx3, minx3.possible_orders(None).unwrap());
        let ccf = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx3)
            .with_register_count(3)
            .with_solution_expansion(SolutionExpansion::All)
            .with_max_fitting_tries(None)
            .validate()
            .unwrap();
        let solutions_calculator = ccf.solutions_calculator(&possible_orders_except_one);
        // 840: 2 2 2 3 5 7
        let register_orders = vec![840, 840, 840];

        let expected = "
            840: c: (7+) e: (4+, 5)
            840: c: (7+) e: (4+, 5)
            840: c: (5+) e: (4+, 7)

            c: 1 ignored, 0 unused
            e: 0 ignored, 1 unused

            840: c: (7+) e: (4+, 5)
            840: c: (5+) e: (4+, 7)
            840: c: (7+) e: (4+, 5)

            c: 1 ignored, 0 unused
            e: 0 ignored, 1 unused

            840: c: (5+) e: (4+, 7)
            840: c: (7+) e: (4+, 5)
            840: c: (7+) e: (4+, 5)

            c: 1 ignored, 0 unused
            e: 0 ignored, 1 unused
        ";

        do_test(solutions_calculator, register_orders, expected);
    }

    #[test_log::test]
    fn orienting_3_cycle() {
        let ccf_base = CycleCombinationFinder::builder()
            .with_register_count(1)
            .with_solution_expansion(SolutionExpansion::All)
            .with_max_fitting_tries(None);

        let crazy = PuzzleDef::<64>::new((
            vec![PartialOrbitDef {
                name: None,
                piece_count: 4.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 2,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            }],
            EvenParityConstraints(vec![vec![]]),
        ))
        .unwrap();
        let possible_orders_except_one =
            mk_possible_orders_except_one(&crazy, crazy.possible_orders(None).unwrap());
        let ccf = ccf_base.clone().with_puzzle_def(&crazy).validate().unwrap();
        let solutions_calculator = ccf.solutions_calculator(&possible_orders_except_one);
        let register_orders = vec![6];

        let expected = "
            6: 0: (3+)

            0: 1 ignored, 0 unused
        ";

        do_test(solutions_calculator, register_orders, expected);

        let crazy = PuzzleDef::<64>::new((
            vec![PartialOrbitDef {
                name: None,
                piece_count: 2.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 3,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            }],
            EvenParityConstraints(vec![vec![]]),
        ))
        .unwrap();
        let possible_orders_except_one =
            mk_possible_orders_except_one(&crazy, crazy.possible_orders(None).unwrap());
        let ccf = ccf_base.with_puzzle_def(&crazy).validate().unwrap();
        let solutions_calculator = ccf.solutions_calculator(&possible_orders_except_one);
        let register_orders = vec![3];

        let expected = "
            3: 0: (1+)

            0: 1 ignored, 0 unused
        ";

        do_test(solutions_calculator, register_orders, expected);
    }

    #[test_log::test]
    fn noncanonical_edge_case() {
        panic!();
        let minx5 = MINX5.clone();
        let possible_orders_except_one =
            mk_possible_orders_except_one(&minx5, minx5.possible_orders(None).unwrap());
        let ccf = CycleCombinationFinder::builder()
            .with_register_count(4)
            .with_solution_expansion(SolutionExpansion::FIRST)
            .with_puzzle_def(&minx5)
            .validate()
            .unwrap();
        let mut solutions_calculator = ccf.solutions_calculator(&possible_orders_except_one);
        let register_orders = vec![2217072, 1420848, 1081080, 240240];

        let mut registers = register_orders
            .into_iter()
            .map(|register_order| {
                possible_orders_len_cast(
                    solutions_calculator
                        .immutable
                        .possible_orders_except_one
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
        let len = solutions.0.len();
        let cycle_combination = CycleCombination {
            registers: Arc::clone(&registers),
            solutions,
        };

        println!(
            "Found {len} solutions in {}:\n{}",
            now.elapsed().human(Truncate::Micro),
            cycle_combination.solutions_fmt(
                solutions_calculator.immutable.possible_orders_except_one,
                solutions_calculator.ccf.puzzle_def,
            )
        );
        panic!();
    }
}

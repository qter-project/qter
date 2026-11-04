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
    finder::{CycleCombination, CycleCombinationFinder, ValidatedSolutionExpansion},
    nonemptyvec::NonemptySlice,
    orderexps::OrderExps,
    puzzle::{OrientationStatus, OrientationSumConstraint, orbit_index_cast, register_index_cast},
};

const MAX_ORIENTATION_EXP: usize = 8;

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
#[derive(Debug)]
pub struct CycleCombinationSolutionsCalculator<'pd, 'a, const N: usize> {
    expansion: bool,
    register_index: u16,

    maybe_solutions: Option<CycleCombinationSolutions>,
    fitting_tries: u32,

    /// Gives the best registers (register index, the exponent)
    register_exponent_sorter: Vec<(u16, u8)>,
    /// Gives the best orientation orders
    best_orientations_queue: [BestOrientation; MAX_ORIENTATION_EXP],

    /// Map of every register, to its cycles, to which orbit its prime power
    /// component is assigned to and bitmask
    register_assignments: Box<[RegisterCycleAssignments<N>]>,
    register_orbit_constraints: Box<[RegisterOrbitConstraint]>,
    /// Remaining piece count for every orbit
    orbit_remaining_pieces: Box<[OrbitRemainingPieces]>,
    ccf: &'a CycleCombinationFinder<'pd, N>,
    immutable: CycleCombinationSolutionsCalculatorImmutable<N>,
}

#[derive(Debug)]
#[non_exhaustive]
pub struct CycleCombinationSolutionsCalculatorImmutable<const N: usize> {
    initial_register_orbit_constraints: Box<[RegisterOrbitConstraint]>,
    initial_orbit_remaining_piece_counts: Box<[OrbitRemainingPieces]>,
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
    RemainingPiece(Option<u8>, RemainingPieceType),
    RegisterCycle,
}

#[derive(Clone, Copy, Debug, Ord, Eq, PartialEq, PartialOrd)]
enum RemainingPieceType {
    Leftover,
    Shared,
    Unknown,
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
    borrowed_noncanonical_piece: bool,
}

#[derive(Debug, Clone)]
struct RegisterCycleAssignments<const N: usize> {
    all_exponents_mask: u64,
    unassigned_exponents_mask: u64,
    cycle_assignments: [PrimePowerCycleAssignment; N],
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PrimePowerCycleAssignment {
    Orbit(u16, CycleOrientState),
    Unassigned,
}

/// Orbits are visited one pass at a time, in declaration order
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum OrbitPass {
    /// The orbit orients by the cycle's prime, so the cycle orients
    Orienting,
    /// The orbit does not orient
    NonOrienting,
    /// The orbit orients by a different prime
    OtherOrienting,
}

#[derive(Debug, Clone, Copy)]
struct OrbitTraversalKey<'a, const N: usize> {
    orbit_index2: usize,
    orbit_pass: OrbitPass,
    unused_piece_count: u16,
    orientation_exps: &'a OrderExps<N>,
    register_orbit_constraint: RegisterOrbitConstraint,
}

#[derive(Debug, Clone, Copy)]
enum BestOrientation {
    Orbit(u16),
    Ambiguous,
    Unassigned,
}

#[derive(Debug, Clone, Copy)]
enum SaturatingOrbit {
    Orbit(u16, u8),
    Ambiguous,
    None,
}

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

impl<'pd, const N: usize> CycleCombinationFinder<'pd, N> {
    #[must_use]
    pub(crate) fn solutions_calculator(&self) -> CycleCombinationSolutionsCalculator<'pd, '_, N> {
        let register_assignments = vec![
            RegisterCycleAssignments {
                all_exponents_mask: 0,
                unassigned_exponents_mask: !0,
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
                        borrowed_noncanonical_piece: false,
                    }
                })
            })
            .collect::<Box<[_]>>();
        let initial_register_orbit_constraints = register_orbit_constraints.clone();
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
        let register_exponent_sorter =
            Vec::with_capacity(NonZeroUsize::from(self.register_count).get());
        let best_orientations_queue = [BestOrientation::Unassigned; MAX_ORIENTATION_EXP];
        CycleCombinationSolutionsCalculator {
            register_index: 0,
            maybe_solutions: None,
            register_assignments,
            register_orbit_constraints,
            orbit_remaining_pieces,
            expansion: false,
            fitting_tries: 0,
            ccf: self,
            register_exponent_sorter,
            best_orientations_queue,
            immutable: CycleCombinationSolutionsCalculatorImmutable {
                initial_register_orbit_constraints,
                initial_orbit_remaining_piece_counts,
                orientations_exps_mask,
            },
        }
    }
}

/// (`pass`, `unused_piece_count` `orientation_exps`,
/// `known_share_state`, `orientation_satisfied_by`, `orbit_index2` if
/// expansion)
fn orbit_traversal_key_cmp<const N: usize>(
    state: &OrbitTraversalKey<N>,
    other_state: &OrbitTraversalKey<N>,
    expansion: bool,
) -> Ordering {
    let &OrbitTraversalKey {
        orbit_index2,
        orbit_pass: pass,
        unused_piece_count,
        orientation_exps,
        register_orbit_constraint:
            RegisterOrbitConstraint {
                known_share_state,
                orientation_satisfied_by,
                borrowed_noncanonical_piece: _,
            },
    } = state;
    let &OrbitTraversalKey {
        orbit_index2: orbit_index2_1,
        orbit_pass: pass_1,
        unused_piece_count: unused_piece_count_1,
        orientation_exps: orientation_exps_1,
        register_orbit_constraint:
            RegisterOrbitConstraint {
                known_share_state: known_share_state_1,
                orientation_satisfied_by: orientation_satisfied_by_1,
                borrowed_noncanonical_piece: _,
            },
    } = other_state;
    let mut ret = pass
        .cmp(&pass_1)
        .then(unused_piece_count.cmp(&unused_piece_count_1))
        .then(orientation_exps.cmp(orientation_exps_1))
        .then(
            known_share_state
                .required_ignored_pieces()
                .cmp(&known_share_state_1.required_ignored_pieces()),
        )
        // We need this. Assume both orbits share nothing
        //
        // o1:
        // 10 pieces
        // 2+ cycle
        //
        // o2:
        // 8 pieces
        //
        // We can't treat them the same. Placing a 3 cycle would require a leftover piece
        // for the second orbit but not the first
        .then(orientation_satisfied_by.cmp(&orientation_satisfied_by_1));
    // Orbits with equal traversal states have identical remaining searches, so
    // existence visits only one of them. Expansion must visit each because
    // the solutions differ.
    if expansion {
        ret = ret.then(orbit_index2.cmp(&orbit_index2_1));
    }
    ret
}

impl<const N: usize> CycleCombinationSolutionsCalculator<'_, '_, N> {
    fn existence_mode(&self) -> bool {
        !self.expansion
    }

    // TODO: inline this more for previous calls
    fn recursive_backtrack(&mut self, registers: DisjointRegisters) -> bool {
        if self.existence_mode()
            && let Some(max_fitting_tries) = self.ccf.maybe_max_fitting_tries
        {
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

            let (orbits_unused_piece_count_sum, invalid) =
                self.finalize_register_orbits(register_index2);

            trace!(
                "{register_index2} after: {:?} {:?}",
                self.orbit_remaining_pieces, self.register_orbit_constraints,
            );

            let ret = if leaf {
                if invalid {
                    false
                } else {
                    self.emit_solution(registers);
                    true
                }
            } else {
                self.register_index += 1;
                let next_register = registers
                    .get_order(self.register_index, &self.ccf.possible_orders_except_one)
                    .unwrap();
                let found = if u32::from(next_register.min_piece_count_naive)
                    > orbits_unused_piece_count_sum
                {
                    trace!("pruned by min piece count");
                    false
                } else if invalid {
                    trace!("pruned by invalid");
                    false
                // debugging
                // } else if self.register_index == 2 {
                //     self.emit_solution(registers);
                //     true
                } else {
                    self.recursive_backtrack(registers)
                };
                self.register_index -= 1;
                found
            };

            trace!("post {register_index2}");
            self.undo_register_frame(register_index2);
            return ret;
        }

        // pop the highest one so we fit large primes first; higher chance of reaching
        // the fail state
        let orienting_unassigned_exponents_mask =
            unassigned_exponents_mask & self.immutable.orientations_exps_mask;
        let any_orienting_unassigned_exponent = orienting_unassigned_exponents_mask != 0;
        // There is a good reason to visit primes with an orbit with that orientation
        // factor first. Recall how every canonically orienting cycle also has an
        // iteration where it does not canonically orient. Also recall how, in the case
        // we want to optimize for, we are just checking for existence. If we have an
        // orienting cycle at the root node, we never have to make it unorient which can
        // be unoptimal
        let prime_index = if any_orienting_unassigned_exponent {
            orienting_unassigned_exponents_mask.ilog2() as usize
        } else {
            unassigned_exponents_mask.ilog2() as usize
        };
        let prime = FIRST_65_PRIMES[prime_index];

        // Nonzero because it is in the unassigned mask
        let register_order = &registers
            .get_order(self.register_index, &self.ccf.possible_orders_except_one)
            .unwrap()
            .order;
        let register_order_exp = register_order.prime_exponent(prime_index);

        let mut maybe_prev_traversal_state: Option<OrbitTraversalKey<N>> = None;
        let orientations_exps = self.ccf.puzzle_def.orientations_exps();
        // Visit orbits in ascending `orbit_traversal_key_cmp` order, skipping equal
        // elements
        while let Some(orbit_traversal_key) = orientations_exps
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
                    // To help understand we examine the two cases:
                    //
                    // 1) There exist orbits with orientation factors that divide this cycle
                    //
                    // We put the cycle into that those orbits *first* in hopes of finding a
                    // solution quickly in existence mode. In the rare case we don't find a solution
                    // after doing so we put the orienting cycle in unorienting orbits.
                    //
                    // 2) There does not exist an orbit with an orientation factor that divides this
                    //    cycle
                    //
                    // It appears that we don't need to care about the orbit traversal order, but
                    // this intuition turns out to be slightly misguided. One case the CCF used to
                    // struggle with looks like:
                    //
                    // 13693680:
                    //
                    // c: (3+)
                    // e: (8+)
                    // +: (5, 7, 11, 13, 19)
                    // w: ()
                    // x: ()
                    //
                    // Here every piece in every orbit is used, and consequently we have
                    // noncanonically orient on edges. If we follow the large cycles into small
                    // orbits ordering then we cram all of the cycles into the edges orbit which
                    // makes including noncanonical cycles difficult or impossible. We first visit
                    // orbits that cannot orient before orbits that orient to favor future
                    // noncanonical cycles.
                    let orbit_pass = if orientation_prime_index == prime_index {
                        OrbitPass::Orienting
                    } else if orientation_prime_index == 64 {
                        OrbitPass::NonOrienting
                    } else {
                        OrbitPass::OtherOrienting
                    };
                    trace!(
                        "pass={orbit_pass:?}; orientation_prime={}; p={prime}",
                        FIRST_65_PRIMES[orientation_prime_index]
                    );
                    // fast check to make sure the nonorienting prime fits into this orbit. we
                    // cannot use logarithms because it misbehaves when the
                    // orbit size exactly fits the prime power
                    if orbit_pass != OrbitPass::Orienting
                        && prime
                            .checked_pow(u32::from(register_order_exp))
                            .is_none_or(|pp| pp > orbit_remaining_pieces.unused)
                    {
                        return acc;
                    }
                    let register_orbit_constraint = self.register_orbit_constraints[register_index2
                        * self.ccf.puzzle_def.orbit_defs().len().get()
                        + orbit_index2];
                    let curr = OrbitTraversalKey {
                        orbit_index2,
                        orbit_pass,
                        unused_piece_count: orbit_remaining_pieces.unused,
                        orientation_exps,
                        register_orbit_constraint,
                    };
                    // Filter out everything <= the previous traversal state
                    if maybe_prev_traversal_state.as_ref().is_some_and(|prev| {
                        orbit_traversal_key_cmp(&curr, prev, self.expansion).is_le()
                    }) {
                        acc
                    // curr is > the previous traversal state. If we have a
                    // previous best `min` and curr < min, or if there was no
                    // previous best, swap out the best with curr.
                    } else if acc.as_ref().is_none_or(|min| {
                        orbit_traversal_key_cmp(&curr, min, self.expansion).is_lt()
                    }) {
                        Some(curr)
                    } else {
                        acc
                    }
                },
            )
        {
            maybe_prev_traversal_state = Some(orbit_traversal_key);
            let OrbitTraversalKey {
                orbit_index2,
                orbit_pass,
                orientation_exps,
                ..
            } = orbit_traversal_key;
            let cycle_orients = orbit_pass == OrbitPass::Orienting;
            trace!("{register_index2} {orbit_index2}; p={prime}");

            let orbit_index = orbit_index_cast(orbit_index2);
            let register_orbit_constraint_index =
                register_index2 * self.ccf.puzzle_def.orbit_defs().len().get() + orbit_index2;

            let orientation_exp = orientation_exps.prime_exponent(prime_index);
            let orbit_unused_piece_count = self.orbit_remaining_pieces[orbit_index2].unused;

            // TODO: do parity by checking ParityConstraint::Even first in OrbitDef
            // put in the 2 cycle case here
            let orient_states = if cycle_orients {
                if self.ccf.fast_assumptions {
                    &[CycleOrientState::Canonical][..]
                } else {
                    // orbit 1: 3 orientations, 20 pieces, zero orientation sum
                    // orbit 2: 1 orientation, 10 pieces, zero orientation sum
                    //
                    // fit architecture 297/3
                    //
                    // the only way to do so is
                    //
                    // orbit 1: 9+ 11+
                    // orbit 2: 3
                    // had we assigned the 3 to orbit 1 in the form of 1+, then 11 could not fit
                    // anywhere
                    //
                    // canonical first so we find a solution faster
                    &[CycleOrientState::Canonical, CycleOrientState::None][..]
                }
            } else {
                &[CycleOrientState::None][..]
            };
            for &(mut orient_state) in orient_states {
                //
                // Keep this synchronized with the preassignment
                //
                let RegisterOrbitConstraint {
                    known_share_state,
                    orientation_satisfied_by,
                    borrowed_noncanonical_piece: _,
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
                    OrientationSatisfiedBy::NoConstraint
                        if orient_state != CycleOrientState::Canonical =>
                    {
                        // We have a non-orienting cycle and no shared pieces in this orbit. We
                        // must have added an orienting cycle if we have one. We need to
                        // indicate that we no longer need leftover pieces.
                        // We have a non-orienting cycle and a shared piece in this orbit. We
                        // must have added an orienting cycle if we have one. We need to
                        // indicate that we no longer need leftover pieces.
                        OrientationSatisfiedBy::RegisterCycle
                    }
                    OrientationSatisfiedBy::NoConstraint => {
                        let remaining_piece_type = if share_anything {
                            RemainingPieceType::Shared
                        } else {
                            RemainingPieceType::Leftover
                        };
                        // If we have no pieces left to be used as leftover then this is
                        // impossible. Note that this is satisfied when the orbit has no
                        // orientation sum constraint.
                        if remaining_piece_type == RemainingPieceType::Leftover
                            && next_orbit_unused_piece_count == 0
                        {
                            continue;
                        }
                        let maybe_noncanonically_orienting_prime_index = if exp == 0 {
                            // prime indicies are <=255
                            #[allow(clippy::cast_possible_truncation)]
                            Some(prime_index as u8)
                        } else {
                            None
                        };
                        OrientationSatisfiedBy::RemainingPiece(
                            maybe_noncanonically_orienting_prime_index,
                            remaining_piece_type,
                        )
                    }
                    OrientationSatisfiedBy::RemainingPiece(
                        Some(_),
                        remaining_piece_type @ (RemainingPieceType::Leftover
                        | RemainingPieceType::Shared
                        | RemainingPieceType::Unknown),
                    ) => {
                        // The noncanonically orienting cycle takes up  we have no pieces left to be
                        // used as leftover then this is impossible. Note that this is satisfied
                        // when the orbit has no orientation sum constraint.
                        // There is no shared piece in the LeftoverPiece case so we can use unused
                        if remaining_piece_type == RemainingPieceType::Leftover
                            && next_orbit_unused_piece_count == 0
                        {
                            continue;
                        }
                        orient_state = CycleOrientState::Noncanonical;
                        OrientationSatisfiedBy::RemainingPiece(None, remaining_piece_type)
                    }
                    OrientationSatisfiedBy::RemainingPiece(
                        None,
                        RemainingPieceType::Leftover
                        | RemainingPieceType::Shared
                        | RemainingPieceType::Unknown,
                    )
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
                    1 << prime_index;
                // Do not announce the cycle assignment if the orbit orients but the `exp`
                // factor is 0.
                if orient_state != CycleOrientState::Canonical || exp != 0 {
                    self.register_assignments[register_index2].cycle_assignments[prime_index] =
                        PrimePowerCycleAssignment::Orbit(orbit_index, orient_state);
                }

                let exists = self.recursive_backtrack(registers);
                if exists {
                    if self.existence_mode() {
                        return true;
                    } else if let Some(max) = self.ccf.solution_expansion.max_count
                        && self.maybe_solutions.as_ref().is_some_and(
                            |CycleCombinationSolutions(solutions)| solutions.len() >= max.get(),
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
                    1 << prime_index;
                // We need to do this now because we don't guarantee to assign every cycle
                // anymore
                if orient_state != CycleOrientState::Canonical || exp != 0 {
                    self.register_assignments[register_index2].cycle_assignments[prime_index] =
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

    #[inline]
    fn finalize_register_orbits(&mut self, register_index2: usize) -> (u32, bool) {
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
                    ref mut borrowed_noncanonical_piece,
                },
                orbit_remaining_piece,
            ),
        ) in orbits_constraints
            .iter_mut()
            .zip(self.orbit_remaining_pieces.iter_mut())
            .enumerate()
        {
            // Promote only if we have no other share rn
            if let OrientationSatisfiedBy::RemainingPiece(
                maybe_noncanonically_orienting_prime_index,
                remaining_piece_type,
            ) = orientation_satisfied_by
            {
                if *known_share_state == ShareState::None {
                    // NOTE: we must make sure we immediately use an unused piece. If we
                    // do so at the end of this match
                    // statement, the checked_sub will mistake
                    // the unused piece as a canonically orienting cycle.
                    match remaining_piece_type {
                        RemainingPieceType::Leftover => {
                            *known_share_state = ShareState::Orientation;
                            orbit_remaining_piece.unused -= 1;
                        }
                        // In this branch we don't guarantee from the recursive step that we
                        // have space.
                        RemainingPieceType::Unknown => {
                            if orbit_remaining_piece.unused == 0 {
                                invalid = true;
                            } else {
                                *known_share_state = ShareState::Orientation;
                                orbit_remaining_piece.unused -= 1;
                            }
                        }
                        RemainingPieceType::Shared => (),
                    }
                }
                if let Some(noncanonically_orienting_prime_index) =
                    maybe_noncanonically_orienting_prime_index
                {
                    trace!("assigning piece to orbit {orbit_index2}");

                    self.register_assignments[register_index2].cycle_assignments
                        [usize::from(noncanonically_orienting_prime_index)] =
                        PrimePowerCycleAssignment::Orbit(
                            orbit_index_cast(orbit_index2),
                            CycleOrientState::Canonical,
                        );
                    if let Some(next_orbit_unused_piece_count) =
                        orbit_remaining_piece.unused.checked_sub(1)
                    {
                        *borrowed_noncanonical_piece = true;
                        orbit_remaining_piece.unused = next_orbit_unused_piece_count;
                    } else {
                        invalid = true;
                    }
                }
            }
            if let Some(RegisterOrbitConstraint {
                known_share_state: next_share_state,
                orientation_satisfied_by: _,
                borrowed_noncanonical_piece: _,
            }) = next_orbits_constraints.get_mut(orbit_index2)
            {
                *next_share_state = *known_share_state;
            }
            orbits_unused_piece_count_sum += u32::from(orbit_remaining_piece.unused);
        }
        (orbits_unused_piece_count_sum, invalid)
    }

    #[inline]
    fn emit_solution(&mut self, registers: DisjointRegisters) {
        if self.expansion {
            trace!("{:#?}", self.register_assignments);
            // TODO: allocator
            let mut register_orbit_cycles = vec![
                vec![];
                NonZeroUsize::from(self.ccf.register_count).get()
                    * self.ccf.puzzle_def.orbit_defs().len().get()
            ]
            .into_boxed_slice();
            for register_index in 0..self.ccf.register_count.get() {
                let register_index2 = usize::from(register_index);
                let register_order = &registers
                    .get_order(register_index, &self.ccf.possible_orders_except_one)
                    .unwrap()
                    .order;
                let register_assignment = &self.register_assignments[register_index2];
                let mut all_exponents = register_assignment.all_exponents_mask;
                while all_exponents != 0 {
                    let prime_index = all_exponents.trailing_zeros() as usize;
                    let prime = FIRST_65_PRIMES[prime_index];
                    let register_order_exp = register_order.prime_exponent(prime_index);
                    // We can have a 7+ on edges to serve as following the 2 cycle and a
                    // 7 cycle, in the false case
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
                    // only the last register has the most recent share state
                    // propagation
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
    }

    #[inline]
    fn undo_register_frame(&mut self, register_index2: usize) {
        let orbit_defs_len = self.ccf.puzzle_def.orbit_defs().len().get();
        let (prev_orbits_constraints, orbits_constraints) = match register_index2.checked_sub(1) {
            Some(prev_register_index2) => {
                let (prev_orbits_constraints, orbits_constraints) = self.register_orbit_constraints
                    [prev_register_index2 * orbit_defs_len..]
                    .split_at_mut(orbit_defs_len);
                (&*prev_orbits_constraints, orbits_constraints)
            }
            None => (
                &self.immutable.initial_register_orbit_constraints[..],
                &mut self.register_orbit_constraints[..],
            ),
        };
        for (
            (
                &mut RegisterOrbitConstraint {
                    ref mut known_share_state,
                    orientation_satisfied_by,
                    ref mut borrowed_noncanonical_piece,
                },
                orbit_remaining_piece,
            ),
            &RegisterOrbitConstraint {
                known_share_state: prev_known_share_state,
                ..
            },
        ) in orbits_constraints
            .iter_mut()
            .zip(self.orbit_remaining_pieces.iter_mut())
            .zip(prev_orbits_constraints)
        {
            orbit_remaining_piece.unused += known_share_state.required_ignored_pieces()
                - prev_known_share_state.required_ignored_pieces();
            *known_share_state = prev_known_share_state;

            if let OrientationSatisfiedBy::RemainingPiece(
                Some(noncanonically_orienting_prime_index),
                _,
            ) = orientation_satisfied_by
            {
                self.register_assignments[register_index2].cycle_assignments
                    [usize::from(noncanonically_orienting_prime_index)] =
                    PrimePowerCycleAssignment::Unassigned;
                trace!("unassigned piece from orbit");
                if *borrowed_noncanonical_piece {
                    orbit_remaining_piece.unused += 1;
                    *borrowed_noncanonical_piece = false;
                }
            }
        }
    }

    #[must_use]
    fn calculate(&mut self, registers: DisjointRegisters) -> SolutionsCalculation {
        // Although we just need the first register orbits, we copy the initial state
        // for all of the other registers because we need *all* the
        // `orientation_satisfied_by` fields to be in the initial state. We copy the
        // entire slice even though we don't need to populate `known_share_state`. This
        // is because it is faster to utilize `clone_from_slice` because `memcpy` is
        // optimized.
        self.register_orbit_constraints
            .clone_from_slice(&self.immutable.initial_register_orbit_constraints);
        self.orbit_remaining_pieces
            .clone_from_slice(&self.immutable.initial_orbit_remaining_piece_counts);

        // Mask of every prime used by all register orders
        let mut orienting_registers_prime_mask = Mask::splat(false);

        for (register_index, possible_order) in registers
            .iter_orders(&self.ccf.possible_orders_except_one)
            .enumerate()
        {
            let all_exponents = possible_order.order.0.simd_ne(Simd::splat(0));
            self.register_assignments[register_index].all_exponents_mask =
                all_exponents.to_bitmask();
            self.register_assignments[register_index].unassigned_exponents_mask =
                all_exponents.to_bitmask();
            orienting_registers_prime_mask |= all_exponents;
        }
        if self.ccf.fast_assumptions {
            let orienting_registers_prime_mask = orienting_registers_prime_mask.to_bitmask();
            // TODO: if an orbit has at least the first highest cycle + second highest cycle
            // number of pieces, we will never not satisfy an orientation constraint

            // Iterate through the set of all primes that divide any register order. The
            // iteration order does not matter because we guarantee every orbit has only
            // a single prime number of orientations.
            let mut orienting_registers_prime_mask2 = orienting_registers_prime_mask;
            while orienting_registers_prime_mask2 != 0 {
                let prime_index = orienting_registers_prime_mask2.trailing_zeros() as usize;
                let prime = FIRST_65_PRIMES[prime_index];
                // We first reset `self.best_orientations_queue`
                self.best_orientations_queue
                    .fill(BestOrientation::Unassigned);
                for (orbit_index, orientation_exps) in
                    self.ccf.puzzle_def.orientations_exps().iter().enumerate()
                {
                    let orbit_index = orbit_index_cast(orbit_index);
                    // Keep orbits that have an orientation of a power of the prime. Note that
                    // this becomes problematic when we remove the constraint that every
                    // orientation count must be a prime power:
                    //
                    // given an orbit of: 5 pieces 48 ori
                    //
                    // fit 576: 3 3 2 2 2 2 2 2
                    //
                    // if you go with 3 (worse); 9 cycle -> 3 cycle; saves 6 pieces
                    // if you go with 2 (better); 64 cycle -> 4 cycle; saves 60 pieces
                    if (orientation_exps.0.simd_ne(Simd::splat(0)).to_bitmask()
                        & orienting_registers_prime_mask)
                        != 1 << prime_index
                    {
                        continue;
                    }
                    let orientation_exp = orientation_exps.prime_exponent(prime_index);

                    // Use the power of the orientation as a hash. We then want to iterate
                    // through the orbits which would provide the best orientation power for
                    // this prime.
                    let slot = &mut self.best_orientations_queue[usize::from(orientation_exp)];
                    match slot {
                        // The slot is occupied; we have another orbit with this power.
                        // If there is an ambiguity among an exponent between two exponents,
                        // we can assign a register to either; this violates the preassignment
                        // guarantee.
                        BestOrientation::Orbit(..) => *slot = BestOrientation::Ambiguous,
                        // First time populate
                        BestOrientation::Unassigned => {
                            *slot = BestOrientation::Orbit(orbit_index);
                        }
                        // Already ambiguous; no work needed
                        BestOrientation::Ambiguous => (),
                    }
                }

                // At this point `self.best_orientations_queue` contains a list of orbit
                // indicies indexed by that orbit's orientation's prime's power.
                // If p=3 o10=3 o11=9, then we would have [Unassigned,
                // Orbit(10), Orbit(11), ...]

                // For the current prime index, iterate through every register and figure out
                // which register orders have the largest power of this prime.
                self.register_exponent_sorter.extend(
                    registers
                        .iter_orders(&self.ccf.possible_orders_except_one)
                        .enumerate()
                        .filter_map(|(register_index2, possible_order)| {
                            let register_index = register_index_cast(register_index2);
                            let register_order_exp =
                                possible_order.order.prime_exponent(prime_index);
                            // - 2^1 is not always best
                            // - at register_order_exp==0, we no longer have primes in this register
                            // order, so there is nothing to assign
                            if prime_index == 0 && register_order_exp == 1
                                || register_order_exp == 0
                            {
                                None
                            } else {
                                Some((register_index, register_order_exp))
                            }
                        }),
                );
                // And sort them descending
                self.register_exponent_sorter
                    .sort_unstable_by_key(|&(_, register_order_exp)| {
                        std::cmp::Reverse(register_order_exp)
                    });

                // At this point it iterates `register_order_exp` in descending order. We now
                // try to match registers to orienting orbits such that it would
                // benefit the most from the orienting cycle
                'next_reg: for (register_index, register_order_exp) in
                    self.register_exponent_sorter.drain(..)
                {
                    let register_index2 = usize::from(register_index);
                    let mut try_assign_pp_to_orbit = |orbit_index: u16,
                                                      orientation_exp: u8|
                     -> bool {
                        let orbit_index2 = usize::from(orbit_index);
                        let orbit_unused_piece_count =
                            self.orbit_remaining_pieces[orbit_index2].unused;
                        let register_orbit_constraint_index = register_index2
                            * self.ccf.puzzle_def.orbit_defs().len().get()
                            + orbit_index2;

                        //
                        // Keep this synchronized with the backtracking assignment
                        //
                        let RegisterOrbitConstraint {
                            known_share_state: _,
                            orientation_satisfied_by,
                            borrowed_noncanonical_piece: _,
                        } = &mut self.register_orbit_constraints[register_orbit_constraint_index];

                        let exp = register_order_exp.saturating_sub(orientation_exp);
                        let cycle_piece_count = if exp == 0 {
                            0
                        } else {
                            prime.pow(u32::from(exp))
                        };

                        let Some(next_orbit_unused_piece_count) =
                            orbit_unused_piece_count.checked_sub(cycle_piece_count)
                        else {
                            trace!(
                                "preassignment: {register_index2} {orbit_index} failed: \
                                 {orbit_unused_piece_count} < {cycle_piece_count}; tried {prime}",
                            );
                            return false;
                        };
                        *orientation_satisfied_by = match *orientation_satisfied_by {
                            OrientationSatisfiedBy::NoConstraint => {
                                // We do not include the ==0 check because we need to be
                                // conservative. It could be that we have a shared piece and
                                // therefore do not need the leftover piece.
                                let maybe_noncanonically_orienting_prime_index = if exp == 0 {
                                    // prime indicies are <=255
                                    #[allow(clippy::cast_possible_truncation)]
                                    Some(prime_index as u8)
                                } else {
                                    None
                                };
                                OrientationSatisfiedBy::RemainingPiece(
                                    maybe_noncanonically_orienting_prime_index,
                                    RemainingPieceType::Unknown,
                                )
                            }
                            OrientationSatisfiedBy::RemainingPiece(..) => {
                                // One orientation assumption
                                unreachable!("We already have an orienting cycle in this orbit");
                            }
                            OrientationSatisfiedBy::RegisterCycle => {
                                OrientationSatisfiedBy::RegisterCycle
                            }
                        };

                        trace!(
                            "preassignment: {register_index2} {orbit_index}: updated {:?}; \
                             assigned {prime} ({orbit_unused_piece_count} -> \
                             {next_orbit_unused_piece_count})",
                            *orientation_satisfied_by,
                        );

                        self.orbit_remaining_pieces[orbit_index2].unused =
                            next_orbit_unused_piece_count;
                        self.register_assignments[register_index2].unassigned_exponents_mask ^=
                            1 << prime_index;
                        if exp != 0 {
                            self.register_assignments[register_index2].cycle_assignments
                                [prime_index] = PrimePowerCycleAssignment::Orbit(
                                orbit_index,
                                CycleOrientState::Canonical,
                            );
                        }

                        true
                    };
                    // Descending exp order of available orienting cycles
                    let mut saturated_orbit_found = SaturatingOrbit::None;
                    for (orbit_index, orientation_exp) in self
                        .best_orientations_queue
                        .iter()
                        .enumerate()
                        .filter_map(|(orientation_exp, &slot)| {
                            if let BestOrientation::Orbit(orbit_index) = slot {
                                // array is 8 elements long
                                #[allow(clippy::cast_possible_truncation)]
                                Some((orbit_index, orientation_exp as u8))
                            } else {
                                None
                            }
                        })
                        .rev()
                    {
                        // Orbit provides more orientation than needed for this register order.
                        // If this happens twice, we have the ambiguous case
                        // i.e: o1=27 o2=81 r1=9
                        if orientation_exp >= register_order_exp {
                            trace!(
                                "prime={prime}; reg={register_index}; {orientation_exp:?} >= \
                                 {register_order_exp}"
                            );
                            if let SaturatingOrbit::Orbit(..) = saturated_orbit_found {
                                saturated_orbit_found = SaturatingOrbit::Ambiguous;
                            } else {
                                saturated_orbit_found =
                                    SaturatingOrbit::Orbit(orbit_index, orientation_exp);
                            }
                        // If we have one saturating orbit and we reach this
                        // branch, we will no longer have any saturating orbits
                        // since we traverse exps in descending order. We want
                        // to saturate the cycle instead of not saturating it
                        // because it saves the most pieces.
                        } else if let SaturatingOrbit::Orbit(..) = saturated_orbit_found {
                            break;
                        // If nothing was saturated at all then preassign the
                        // orienting cycle into the orbit
                        } else if try_assign_pp_to_orbit(orbit_index, orientation_exp) {
                            continue 'next_reg;
                        }
                    }
                    // If this only happens once, then it's not technically ambiguous
                    if let SaturatingOrbit::Orbit(orbit_index, orientation_exp) =
                        saturated_orbit_found
                    {
                        try_assign_pp_to_orbit(orbit_index, orientation_exp);
                    }
                }

                orienting_registers_prime_mask2 ^=
                    orienting_registers_prime_mask2.isolate_lowest_one();
            }
        }

        self.fitting_tries = 0;
        if self.expansion {
            self.recursive_backtrack(registers);
            if self.maybe_solutions.is_some() {
                debug!(
                    "Expansion for {} in {} steps",
                    dbg_registers(registers.iter(), &self.ccf.possible_orders_except_one),
                    self.fitting_tries
                );
            }

            SolutionsCalculation::MaybeExpansion(self.maybe_solutions.take())
        } else {
            let existence = self.recursive_backtrack(registers);
            if existence {
                debug!(
                    "Solution for {} in {} tries",
                    dbg_registers(registers.iter(), &self.ccf.possible_orders_except_one),
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

pub fn expand_possible_registers<const N: usize>(
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
        cycle_combinations_tree::DisjointRegisters,
        finder::{
            CycleCombination, CycleCombinationFinderBuilder, NumCores, PossibleOrder,
            PuzzleAnalysis, Register, SolutionExpansion,
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
                        .ccf
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
            &solutions_calculator.ccf.possible_orders_except_one,
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

        let analysis = PuzzleAnalysis {
            puzzle_def: &crazy,
            possible_orders_except_one: Arc::from(vec![PossibleOrder {
                order: OrderExps::try_from(NonZeroU16::new(3).unwrap()).unwrap(),
                min_piece_count: 1.try_into().unwrap(),
                min_piece_count_naive: 1,
            }]),
        };
        let ccf = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(1))
            .with_solution_expansion(SolutionExpansion::All)
            .build()
            .unwrap();
        let mut solutions_calculator = ccf.solutions_calculator();
        solutions_calculator.existence(DisjointRegisters::from(
            NonemptySlice::try_from(&[0][..]).unwrap(),
        ));
    }

    #[test_log::test]
    fn preassignment_single_saturating_orbit() {
        let crazy = PuzzleDef::<64>::new((
            vec![
                PartialOrbitDef {
                    name: None,
                    piece_count: 4.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 9,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    name: None,
                    piece_count: 4.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 3,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
            ],
            EvenParityConstraints(vec![vec![]]),
        ))
        .unwrap();
        let analysis = PuzzleAnalysis::new(&crazy, NumCores::AllCores).unwrap();
        let ccf_base = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(1))
            .with_solution_expansion(SolutionExpansion::All);
        let register_orders = vec![9];

        let ccf = ccf_base
            .clone()
            .with_fast_assumptions(false)
            .build()
            .unwrap();
        let expected = "
            9: 0: () 1: (3+)

            0: 0 ignored, 4 unused
            1: 1 ignored, 0 unused

            9: 0: (1+) 1: ()

            0: 1 ignored, 2 unused
            1: 0 ignored, 4 unused
        ";
        do_test(
            ccf.solutions_calculator(),
            register_orders.clone(),
            expected,
        );

        let ccf = ccf_base.with_fast_assumptions(true).build().unwrap();
        let expected = "
            9: 0: (1+) 1: ()

            0: 1 ignored, 2 unused
            1: 0 ignored, 4 unused
        ";
        do_test(ccf.solutions_calculator(), register_orders, expected);
    }

    #[test_log::test]
    fn expansion_coincident_unused_counts() {
        let crazy = PuzzleDef::<64>::new((
            vec![
                PartialOrbitDef {
                    name: None,
                    piece_count: 8.try_into().unwrap(),
                    orientation: OrientationStatus::CannotOrient,
                },
                PartialOrbitDef {
                    name: None,
                    piece_count: 5.try_into().unwrap(),
                    orientation: OrientationStatus::CannotOrient,
                },
            ],
            EvenParityConstraints(vec![]),
        ))
        .unwrap();
        let analysis = PuzzleAnalysis::new(&crazy, NumCores::AllCores).unwrap();
        let ccf = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(2))
            .with_solution_expansion(SolutionExpansion::All)
            .build()
            .unwrap();
        let register_orders = vec![3, 2];
        let expected = "
            3: 0: () 1: (3)
            2: 0: () 1: (2)

            0: 0 ignored, 8 unused
            1: 0 ignored, 0 unused

            3: 0: () 1: (3)
            2: 0: (2) 1: ()

            0: 0 ignored, 6 unused
            1: 0 ignored, 2 unused

            3: 0: (3) 1: ()
            2: 0: (2) 1: ()

            0: 0 ignored, 3 unused
            1: 0 ignored, 5 unused

            3: 0: (3) 1: ()
            2: 0: () 1: (2)

            0: 0 ignored, 5 unused
            1: 0 ignored, 3 unused
        ";
        do_test(ccf.solutions_calculator(), register_orders, expected);
    }

    #[test_log::test]
    fn nonorienting_prime_in_orienting_orbit() {
        let crazy = PuzzleDef::<64>::new((
            vec![PartialOrbitDef {
                name: None,
                piece_count: 3.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 2,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
            }],
            EvenParityConstraints(vec![]),
        ))
        .unwrap();
        let analysis = PuzzleAnalysis::new(&crazy, NumCores::AllCores).unwrap();
        let ccf = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(1))
            .with_solution_expansion(SolutionExpansion::All)
            .build()
            .unwrap();
        let solutions_calculator = ccf.solutions_calculator();
        let register_orders = vec![3];

        let expected = "
            3: 0: (3)

            0: 0 ignored, 0 unused
        ";

        do_test(solutions_calculator, register_orders, expected);
    }

    #[test_log::test]
    fn minx3_optimal_3() {
        let minx3 = MINX3.clone();
        let analysis = PuzzleAnalysis::new(&minx3, NumCores::AllCores).unwrap();
        let ccf = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(3))
            .with_solution_expansion(SolutionExpansion::All)
            .build()
            .unwrap();
        let solutions_calculator = ccf.solutions_calculator();
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
        let analysis = PuzzleAnalysis::new(&minx3, NumCores::AllCores).unwrap();
        let ccf = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(3))
            .with_solution_expansion(SolutionExpansion::All)
            .build()
            .unwrap();
        let solutions_calculator = ccf.solutions_calculator();
        // 840: 2 2 2 3 5 7
        let register_orders = vec![840, 840, 840];

        let expected = "
            840: c: (5+) e: (4+, 7)
            840: c: (7+) e: (4+, 5)
            840: c: (7+) e: (4+, 5)

            c: 1 ignored, 0 unused
            e: 0 ignored, 1 unused

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
        ";

        do_test(solutions_calculator, register_orders, expected);
    }

    #[test_log::test]
    fn orienting_3_cycle() {
        let ccf_base = CycleCombinationFinderBuilder::default()
            .with_solution_expansion(SolutionExpansion::All);

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
        let analysis = PuzzleAnalysis::new(&crazy, NumCores::AllCores).unwrap();
        let ccf = ccf_base
            .clone()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(1))
            .build()
            .unwrap();
        let solutions_calculator = ccf.solutions_calculator();
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
        let analysis = PuzzleAnalysis::new(&crazy, NumCores::AllCores).unwrap();
        let ccf = ccf_base
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(1))
            .build()
            .unwrap();
        let solutions_calculator = ccf.solutions_calculator();
        let register_orders = vec![3];

        let expected = "
            3: 0: (1+)

            0: 1 ignored, 0 unused
        ";

        do_test(solutions_calculator, register_orders, expected);
    }

    #[test_log::test]
    #[ignore = "long"]
    fn main1() {
        let minx5 = MINX5.clone();
        let analysis = PuzzleAnalysis::new(&minx5, NumCores::AllCores).unwrap();
        let ccf = CycleCombinationFinderBuilder::default()
            .with_solution_expansion(SolutionExpansion::FIRST)
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(4))
            .build()
            .unwrap();
        let mut solutions_calculator = ccf.solutions_calculator();
        // 8288280, 1531530, 1531530, 1081080
        let register_orders = vec![8_288_280, 1_531_530, 1_531_530, 1_081_080];

        let mut registers = register_orders
            .into_iter()
            .map(|register_order| {
                possible_orders_len_cast(
                    solutions_calculator
                        .ccf
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
                &solutions_calculator.ccf.possible_orders_except_one,
                solutions_calculator.ccf.puzzle_def,
            )
        );
        panic!();
    }
}

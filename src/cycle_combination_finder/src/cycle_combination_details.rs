use std::{
    num::{NonZeroU16, NonZeroU32, NonZeroUsize},
    simd::{Mask, Simd, cmp::SimdPartialEq},
};

use log::trace;

use crate::{
    FIRST_65_PRIMES, cycle_combinations_tree::DisjointRegisters, finder::PossibleOrder, puzzle::{OrientationStatus, OrientationSumConstraint, ParityConstraint, PuzzleDef, orbit_index_cast, register_index_cast},
};

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
enum SharingState {
    None,
    Orientation,
    Parity,
}

#[derive(Debug)]
pub struct Cycles(Box<[u16]>);

#[derive(Debug)]
pub struct CycleCombinationDetail {
    reg_to_cycles: Box<[Cycles]>,
}

#[derive(Debug)]
#[non_exhaustive]
pub struct CycleCombinationDetails<'a, 'b, const N: usize> {
    possible_orders_except_one: &'a [PossibleOrder<N>],
    puzzle_def: &'b PuzzleDef<N>,
    /// Map of every register, to its cycles, to which orbit its prime power
    /// component is assigned to and bitmask
    reg_to_cycle_orbit: Box<[[PPCycleAssignment; N]]>,
    /// Remaining piece count for every orbit
    orbit_remaining_piece_counts: Box<[u16]>,
    /// Remaining piece count for every connected orbit; used for parity
    /// constraints
    component_remaining_piece_counts: Box<[u32]>,
    /// Read-only
    orbit_remaining_piece_counts2: Box<[u16]>,
    /// Read-only
    component_remaining_piece_counts2: Box<[u32]>,
    /// Gives the best registers
    register_exponent_sorter: Vec<(u16, u8)>,
    /// Gives the best orientation orders
    best_orientations_queue: [BestOrientation; 9],
}

#[derive(Debug, Clone, Copy)]
enum PPCycleAssignment {
    Orbit(u16, SharingState),
    Unused,
    Unassigned,
}

#[derive(Debug, Clone, Copy)]
enum BestOrientation {
    Orbit(u16, SharingState),
    Ambiguous,
    Unassigned,
}

#[derive(Debug, Clone, Copy)]
enum SaturatingOrbit {
    Orbit(u16, u8, SharingState),
    Ambiguous,
    None,
}

impl SharingState {
    fn required_pieces(self, have_orienting_sharing_cycle: bool) -> u16 {
        match self {
            SharingState::None => 0,
            SharingState::Orientation if have_orienting_sharing_cycle => 0,
            SharingState::Orientation => 1,
            SharingState::Parity => 2,
        }
    }

    fn enough_leftover_pieces(
        self,
        orbit_remaining_piece_count: u16,
        component_remaining_piece_count: u32,
        have_orienting_sharing_cycle: bool,
    ) -> bool {
        let required_pieces = self.required_pieces(have_orienting_sharing_cycle);
        match self {
            SharingState::None => true,
            SharingState::Orientation => orbit_remaining_piece_count >= required_pieces,
            SharingState::Parity => component_remaining_piece_count >= u32::from(required_pieces),
        }
    }
}

impl<'a, 'b, const N: usize> CycleCombinationDetails<'a, 'b, N> {
    #[must_use]
    pub fn new(
        exact_register_count: NonZeroU16,
        possible_orders_except_one: &'a [PossibleOrder<N>],
        puzzle_def: &'b PuzzleDef<N>,
    ) -> Self {
        // TODO: allocator
        let reg_to_cycle_orbit =
            vec![[PPCycleAssignment::Unused; N]; NonZeroUsize::from(exact_register_count).get()]
                .into_boxed_slice();
        let orbit_remaining_piece_counts = puzzle_def
            .orbit_defs()
            .iter()
            .map(|orbit_def| orbit_def.piece_count.get())
            .collect::<Box<[_]>>();
        let orbit_remaining_piece_counts2 = orbit_remaining_piece_counts.clone();
        let component_remaining_piece_counts = puzzle_def
            .connected_components()
            .iter()
            .map(|connected_component| {
                connected_component
                    .iter()
                    .map(|&orbit_index| {
                        NonZeroU32::from(
                            puzzle_def.orbit_defs()[usize::from(orbit_index)].piece_count,
                        )
                        .get()
                    })
                    .sum()
            })
            .collect::<Box<[_]>>();
        let component_remaining_piece_counts2 = component_remaining_piece_counts.clone();
        let register_exponent_sorter =
            Vec::with_capacity(NonZeroUsize::from(exact_register_count).get());
        let best_orientations_queue = [BestOrientation::Unassigned; 9];
        Self {
            possible_orders_except_one,
            puzzle_def,
            reg_to_cycle_orbit,
            orbit_remaining_piece_counts,
            component_remaining_piece_counts,
            orbit_remaining_piece_counts2,
            component_remaining_piece_counts2,
            register_exponent_sorter,
            best_orientations_queue,
        }
    }

    #[must_use]
    pub fn calculate(&mut self, registers: DisjointRegisters) -> Option<CycleCombinationDetail> {
        self.reg_to_cycle_orbit.fill([PPCycleAssignment::Unused; N]);
        self.orbit_remaining_piece_counts
            .clone_from_slice(&self.orbit_remaining_piece_counts2);
        self.component_remaining_piece_counts
            .clone_from_slice(&self.component_remaining_piece_counts2);

        // Every prime used by the register orders
        let mut orienting_registers_prime_mask = Mask::splat(false);

        for (register_index, possible_order) in registers
            .iter_orders(self.possible_orders_except_one)
            .enumerate()
        {
            let all_exponents = possible_order.order.0.simd_ne(Simd::splat(0));
            orienting_registers_prime_mask |= all_exponents;
            // Set which
            let mut all_exponents_mask = all_exponents.to_bitmask();
            while all_exponents_mask != 0 {
                let prime_index = all_exponents_mask.trailing_zeros() as usize;
                self.reg_to_cycle_orbit[register_index][prime_index] =
                    PPCycleAssignment::Unassigned;
                all_exponents_mask ^= all_exponents_mask.isolate_lowest_one();
            }
        }
        let orienting_registers_prime_mask = orienting_registers_prime_mask.to_bitmask();

        let mut orienting_registers_prime_mask2 = orienting_registers_prime_mask;
        while orienting_registers_prime_mask2 != 0 {
            let prime_index = orienting_registers_prime_mask2.trailing_zeros() as usize;
            let prime = FIRST_65_PRIMES[prime_index];
            self.best_orientations_queue
                .fill(BestOrientation::Unassigned);
            for (orbit_index, (orientation_exps, &orbit_def)) in self
                .puzzle_def
                .orientations_exps()
                .iter()
                .zip(self.puzzle_def.orbit_defs().iter())
                .enumerate()
            {
                let orbit_index = orbit_index_cast(orbit_index);
                // counterexample:
                // o1: 5 pieces 48 ori
                //
                // fit 576: 3 3 2 2 2 2 2 2
                //
                // if you go with 3 (worse); 9 cycle -> 3 cycle; saves 6 pieces
                // if you go with 2 (better); 64 cycle -> 4 cycle; saves 60 pieces
                let exactly_prime_factors =
                    (orientation_exps.0.simd_ne(Simd::splat(0)).to_bitmask()
                        & orienting_registers_prime_mask)
                        == (1 << prime_index);
                if !exactly_prime_factors {
                    continue;
                }
                let orbit_orientation_exp = orientation_exps.prime_exponent(prime_index);
                let required_extra_pieces = if prime_index == 0
                    && (orbit_def.parity_constraint == ParityConstraint::Even
                        || orbit_def.parity_constraint == ParityConstraint::None)
                {
                    // - 2^n is not necessarily valid with +1 of space because of parity
                    // we COULD parity swap with another orbit; however we just focus on the
                    // worst case
                    SharingState::Parity
                } else if matches!(
                    orbit_def.orientation,
                    OrientationStatus::CanOrient {
                        count: _,
                        sum_constraint: OrientationSumConstraint::Zero
                    }
                ) {
                    // - x^n is not necessarily valid with +0 of space because of
                    // orientation
                    SharingState::Orientation
                } else {
                    SharingState::None
                };

                // If there is an ambiguity among an exponent between two exponents,
                // we can assign a register to either; this violates the guarantee
                let slot = &mut self.best_orientations_queue[usize::from(orbit_orientation_exp)];
                match slot {
                    BestOrientation::Orbit(..) => *slot = BestOrientation::Ambiguous,
                    BestOrientation::Unassigned => {
                        *slot = BestOrientation::Orbit(orbit_index, required_extra_pieces);
                    }
                    BestOrientation::Ambiguous => (),
                }
            }

            // For the current prime index, iterate through every register and figure out
            // which registers have the largest power of this prime.
            self.register_exponent_sorter.extend(
                registers
                    .iter_orders(self.possible_orders_except_one)
                    .enumerate()
                    .filter_map(|(register_index, possible_order)| {
                        let register_index = register_index_cast(register_index);
                        let register_order_exp = possible_order.order.prime_exponent(prime_index);
                        // - 2^1 is not always best
                        // at register_order_exp==0, we no longer have primes in this register
                        // order, so there is nothing to assign
                        if prime_index == 0 && register_order_exp == 1 || register_order_exp == 0 {
                            None
                        } else {
                            Some((register_index, register_order_exp))
                        }
                    }),
            );
            self.register_exponent_sorter
                .sort_unstable_by_key(|&(_, register_order_exp)| {
                    std::cmp::Reverse(register_order_exp)
                });

            // Try to fit a register's prime power cycle into an orbit such that it would
            // benefit the most from a share
            for (register_index, register_order_exp) in self.register_exponent_sorter.drain(..) {
                let slot @ PPCycleAssignment::Unassigned =
                    &mut self.reg_to_cycle_orbit[usize::from(register_index)][prime_index]
                else {
                    unreachable!();
                };
                let mut try_assign_pp_to_orbit = |orbit_index: u16,
                                                  orbit_orientation_exp: u8,
                                                  required_extra_pieces: SharingState|
                 -> bool {
                    let orbit_remaining_piece_count =
                        &mut self.orbit_remaining_piece_counts[usize::from(orbit_index)];
                    let exp = register_order_exp.saturating_sub(orbit_orientation_exp);
                    let cycle_piece_count = if exp == 0 {
                        0
                    } else {
                        prime.pow(u32::from(exp))
                    };

                    if let Some(next_orbit_remaining_piece_count) =
                        orbit_remaining_piece_count.checked_sub(cycle_piece_count)
                    {
                        let component_remaining_piece_count = &mut self
                            .component_remaining_piece_counts[usize::from(
                            self.puzzle_def.orbit_index_to_component_index(orbit_index),
                        )];
                        let next_component_remaining_piece_count =
                            *component_remaining_piece_count - u32::from(cycle_piece_count);

                        if required_extra_pieces.enough_leftover_pieces(
                            next_orbit_remaining_piece_count,
                            next_component_remaining_piece_count,
                            // Assume worse case; it is otherwise not simple to keep track of when
                            // an orbit has an orienting cycle during this stage (it requires at
                            // least a pass of all primes)
                            false,
                        ) {
                            *orbit_remaining_piece_count = next_orbit_remaining_piece_count;
                            *component_remaining_piece_count = next_component_remaining_piece_count;

                            *slot = PPCycleAssignment::Orbit(orbit_index, required_extra_pieces);
                            return true;
                        }
                    }
                    false
                };
                // Descending exp order of available orientation-sharing cycles
                let mut saturated_orbit_found = SaturatingOrbit::None;
                for (orbit_index, orbit_orientation_exp, required_extra_pieces) in self
                    .best_orientations_queue
                    .iter()
                    .enumerate()
                    .filter_map(|(orbit_orientation_exp, &slot)| {
                        if let BestOrientation::Orbit(orbit_index, required_share) = slot {
                            // array is 9 elements long
                            #[allow(clippy::cast_possible_truncation)]
                            Some((orbit_index, orbit_orientation_exp as u8, required_share))
                        } else {
                            None
                        }
                    })
                    .rev()
                {
                    // Orbit provides more orientation than needed for this register order. We may
                    // still have the ambiguous case
                    if orbit_orientation_exp >= register_order_exp {
                        trace!(
                            "prime={prime}; reg={register_index}; {orbit_orientation_exp:?} > \
                             {register_order_exp}"
                        );
                        if let SaturatingOrbit::Orbit(..) = saturated_orbit_found {
                            saturated_orbit_found = SaturatingOrbit::Ambiguous;
                        } else {
                            saturated_orbit_found = SaturatingOrbit::Orbit(
                                orbit_index,
                                orbit_orientation_exp,
                                required_extra_pieces,
                            );
                        }
                    } else if try_assign_pp_to_orbit(
                        orbit_index,
                        orbit_orientation_exp,
                        required_extra_pieces,
                    ) {
                        break;
                    }
                }
                if let SaturatingOrbit::Orbit(
                    orbit_index,
                    orbit_orientation_exp,
                    required_extra_pieces,
                ) = saturated_orbit_found
                {
                    try_assign_pp_to_orbit(
                        orbit_index,
                        orbit_orientation_exp,
                        required_extra_pieces,
                    );
                }
            }

            orienting_registers_prime_mask2 ^= orienting_registers_prime_mask2.isolate_lowest_one();
        }

        for (i, r) in self.reg_to_cycle_orbit.iter().enumerate() {
            println!(
                "reg: {:?}: {r:#?}",
                registers.get_order(i, self.possible_orders_except_one)
            );
        }
        println!("{:?}", self.orbit_remaining_piece_counts);

        // select cycle
        // assign cycle to every orbit, dfs
        todo!()
    }
}

impl CycleCombinationDetail {
    #[must_use]
    pub fn cycles(&self) -> &[Cycles] {
        &self.reg_to_cycles
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU16;

    use crate::{
        cycle_combination_details::CycleCombinationDetails,
        cycle_combinations_tree::DisjointRegisters,
        finder::{PossibleOrder, mk_possible_orders_except_one},
        nonemptyvec::NonemptySlice,
        orderexps::OrderExps,
        puzzle::{
            EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef,
            PuzzleDef, minxN::MINX3,
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

        let detail = CycleCombinationDetails::new(
            NonZeroU16::new(1).unwrap(),
            &[PossibleOrder {
                order: OrderExps::try_from(NonZeroU16::new(3).unwrap()).unwrap(),
                min_piece_count: 1.try_into().unwrap(),
            }],
            &crazy,
        )
        .calculate(DisjointRegisters::from(
            NonemptySlice::try_from(&[0][..]).unwrap(),
        ))
        .unwrap();
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

        let detail = CycleCombinationDetails::new(
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
            &crazy,
        )
        .calculate(DisjointRegisters::from(
            NonemptySlice::try_from(&[0, 1, 2, 3, 4, 5][..]).unwrap(),
        ))
        .unwrap();
    }

    #[test_log::test]
    fn foo() {
        let minx3 = MINX3.clone();
        let possible_orders_except_one =
            mk_possible_orders_except_one(&minx3, minx3.possible_orders(None, true).unwrap());
        // 2520 630 420
        let detail = CycleCombinationDetails::new(
            NonZeroU16::new(3).unwrap(),
            &possible_orders_except_one,
            &minx3,
        )
        .calculate(DisjointRegisters::from(
            NonemptySlice::try_from(&[504, 251, 196][..]).unwrap(),
        ))
        .unwrap();

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
        // e: (5+, 7+); total 10/30
        // c: (3+); total 3/20
        //
        // 420:
        //
        // e: (2+, 7+); total 9/30
        // c: (5+); total 5/20
        //
        // parity share 2 edges or corners
        //
        // 28/30
        // 18/20

        println!("{detail:?}");
        panic!();
    }
}

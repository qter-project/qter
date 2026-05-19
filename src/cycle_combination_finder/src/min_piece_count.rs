use std::{
    num::{NonZeroU16, NonZeroU32},
    simd::{Simd, cmp::SimdPartialEq},
};

use crate::{
    FIRST_129_PRIMES,
    orderexps::OrderExps,
    puzzle::{OrbitDef, OrientationStatus, OrientationSumConstraint, PuzzleDef},
};

#[derive(Debug)]
pub struct MinPieceCount<'a, const N: usize> {
    orientations_exps: Vec<OrderExps<N>>,
    orbit_orientation_contributions: Vec<OrderExps<N>>,
    orbit_defs: &'a [OrbitDef],
    leftover_prime_powers_mask: u64,
    orientations_exps_lcm: OrderExps<N>,
    has_even_parity_constraint: Vec<bool>,
}

impl<'a, const N: usize> From<&'a PuzzleDef<N>> for MinPieceCount<'a, N> {
    fn from(puzzle_def: &'a PuzzleDef<N>) -> Self {
        let orientations_exps = puzzle_def
            .orbit_defs()
            .iter()
            .map(|orbit_def| {
                OrderExps::<N>::try_from(NonZeroU16::from(orbit_def.orientation_count())).unwrap()
            })
            .collect::<Vec<_>>();
        let orientations_exps_lcm = OrderExps::lcms(orientations_exps.iter().cloned()).unwrap();
        let leftover_prime_powers_mask =
            orientations_exps_lcm.0.simd_eq(Simd::splat(0)).to_bitmask();
        let orbit_orientation_contributions = vec![OrderExps::one(); orientations_exps.len()];

        let mut has_even_parity_constraint = vec![false; puzzle_def.orbit_defs().len()];
        for orbit_index in (0..puzzle_def.even_parity_constraints().rows()).flat_map(|i| {
            puzzle_def
                .even_parity_constraints()
                .row(i)
                .iter()
                .enumerate()
                .filter_map(|(orbit_index, x)| if x { Some(orbit_index) } else { None })
        }) {
            has_even_parity_constraint[orbit_index] = true;
        }

        Self {
            orientations_exps,
            orbit_orientation_contributions,
            leftover_prime_powers_mask,
            orientations_exps_lcm,
            has_even_parity_constraint,
            orbit_defs: puzzle_def.orbit_defs(),
        }
    }
}

fn prime_power_cycle_piece_count(prime: u16, exp: u8) -> u32 {
    if exp == 0 {
        0
    } else {
        u32::from(prime).pow(u32::from(exp))
    }
}

impl<const N: usize> MinPieceCount<'_, N> {
    /// Compute a lower bound of the minimum number of pieces to construct a
    /// twisty puzzle group element with *known* possible order. This bound is
    /// close to being tight.
    ///
    /// # Panics
    ///
    /// This method panics if one is the possible order, since the minimum
    /// number of piece is zero, and it would be preferrable for this function
    /// to always return a [`NonZero`] type.
    // TODO: devise a scheme to make this incorporate piece counts
    pub fn calculate(&mut self, possible_order: &OrderExps<N>) -> NonZeroU32 {
        assert_ne!(possible_order, &OrderExps::one());

        let mut leftover_prime_powers_sum = 0;
        let mut leftover_prime_powers_count = 0;
        let mut leftover_prime_powers_mask = self.leftover_prime_powers_mask;
        while leftover_prime_powers_mask != 0 {
            let prime_power_index = leftover_prime_powers_mask.trailing_zeros() as usize;
            let exp = possible_order.0[prime_power_index];
            let prime = FIRST_129_PRIMES[prime_power_index];

            let leftover_prime_power = prime_power_cycle_piece_count(prime, exp);
            if leftover_prime_power != 0 {
                leftover_prime_powers_sum += leftover_prime_power;
                leftover_prime_powers_count += 1;
            }
            leftover_prime_powers_mask ^= leftover_prime_powers_mask.isolate_lowest_one();
        }

        let required_cycle_prime_powers =
            possible_order.remove_factors(&self.orientations_exps_lcm);
        let mut prime_power_to_orbit: [Option<(usize, u32)>; N] = [None; N];
        for (orbit_index, orientation_exps) in self.orientations_exps.iter().enumerate() {
            let mut eq = (possible_order
                .remove_factors(orientation_exps)
                .0
                .simd_eq(required_cycle_prime_powers.0)
                & possible_order.0.simd_ne(Simd::splat(0)))
            .to_bitmask();
            let eq_count = eq.count_ones();
            while eq != 0 {
                #[allow(clippy::cast_possible_truncation)]
                let prime_power_index = usize::from(eq.trailing_zeros() as u16);
                if (self.leftover_prime_powers_mask >> prime_power_index) & 1 == 0 {
                    match &mut prime_power_to_orbit[prime_power_index] {
                        Some(dominating_orbit) => {
                            if eq_count > dominating_orbit.1 {
                                *dominating_orbit = (orbit_index, eq_count);
                            }
                        }
                        to_dominate @ None => {
                            *to_dominate = Some((orbit_index, eq_count));
                        }
                    }
                }
                eq ^= eq.isolate_lowest_one();
            }
        }

        self.orbit_orientation_contributions.fill(OrderExps::one());

        let mut maybe_two_orientation_contribution_orbit_index = None;
        for (prime_power_index, orbit) in prime_power_to_orbit.into_iter().enumerate() {
            let Some((orbit_index, _)) = orbit else {
                continue;
            };
            if prime_power_index == 0 && required_cycle_prime_powers.two_exponent() != 0 {
                maybe_two_orientation_contribution_orbit_index = Some(orbit_index);
            }
            self.orbit_orientation_contributions[orbit_index].0[prime_power_index] =
                self.orientations_exps_lcm.0[prime_power_index];
        }

        // The maximum number of contributing orbits is the max N, one for every prime.
        // Thus this fits into a u32.
        let mut needing_orientation_cycles_count = 0u32;
        let mut min_piece_count = leftover_prime_powers_sum;
        let mut transfer_extra_two_cycle = false;
        let mut receive_extra_two_cycle = false;
        for ((orbit_orientation_contribution, orientation_exps), orbit_def) in self
            .orbit_orientation_contributions
            .iter()
            .zip(&self.orientations_exps)
            .zip(self.orbit_defs)
        {
            let mut contributing_prime_powers = orbit_orientation_contribution
                .0
                .simd_ne(Simd::splat(0))
                .to_bitmask();
            let orientation_contribution_is_two = match contributing_prime_powers.count_ones() {
                0 => continue,
                1 => orbit_orientation_contribution.two_exponent() != 0,
                _ => false,
            };
            let mut cycles_count = 0u32;
            while contributing_prime_powers != 0 {
                let prime_power_index = contributing_prime_powers.trailing_zeros() as usize;
                let exp = required_cycle_prime_powers.0[prime_power_index];
                let prime = FIRST_129_PRIMES[prime_power_index];
                let cycle_piece_count = prime_power_cycle_piece_count(prime, exp);
                if cycle_piece_count != 0 {
                    min_piece_count += cycle_piece_count;
                    cycles_count += 1;
                }
                contributing_prime_powers ^= contributing_prime_powers.isolate_lowest_one();
            }
            if orientation_contribution_is_two {
                transfer_extra_two_cycle = cycles_count == 0;
            } else {
                // we include the first condition because we have ensure the cycle with the
                // extra piece is the cycle receiving it
                receive_extra_two_cycle |= cycles_count < 2
                    && u16::from(possible_order.two_exponent())
                        == 1 + u16::from(orientation_exps.two_exponent());
            }
            needing_orientation_cycles_count += match orbit_def.orientation {
                // Sanity check: if the orbit cannot orient, its orientation factors always
                // contributes nothing, and this contribution would have been skipped at the
                // beginning of this loop
                OrientationStatus::CannotOrient => unreachable!(),
                OrientationStatus::CanOrient {
                    count: _,
                    sum_constraint: OrientationSumConstraint::Zero,
                } => 2u32.saturating_sub(cycles_count),
                OrientationStatus::CanOrient {
                    count: _,
                    sum_constraint: OrientationSumConstraint::None,
                } => 1u32.saturating_sub(cycles_count),
            };
        }
        let mut extra_piece_count =
            needing_orientation_cycles_count.saturating_sub(leftover_prime_powers_count);
        // first condition: if the leftover cycles are *never* enough to add enough
        // extra pieces to invalidate the two extra cycle transfer
        if extra_piece_count > 2 && receive_extra_two_cycle && transfer_extra_two_cycle {
            extra_piece_count -= 1;
            // we don't know to which orbit we are tranferring this cycle to;
            // such analysis is too complicated so we just give up.
            // TODO: do this
        } else if let Some(two_orientation_contribution_orbit_index) =
            maybe_two_orientation_contribution_orbit_index
            && self.has_even_parity_constraint[two_orientation_contribution_orbit_index]
        {
            // sufficiently advanced analysis cannot assign the two orientation to an orbit
            // with an even parity constraint if the counts are the same to improve because
            // that would not be the worst case scenario
            extra_piece_count += 2;
        }
        min_piece_count += extra_piece_count;

        // Compute the naive minimum piece count and ensure this bound is never worse
        debug_assert!(
            min_piece_count
                >= possible_order
                    .remove_factors(&self.orientations_exps_lcm)
                    .0
                    .as_array()
                    .iter()
                    .zip(FIRST_129_PRIMES)
                    .map(|(&exp, prime)| prime_power_cycle_piece_count(prime, exp))
                    .sum::<u32>()
        );
        // Every non-one order requires at least one piece
        NonZeroU32::new(min_piece_count).unwrap()
    }
}

#[cfg(test)]
mod initialization {
    use crate::min_piece_count::{
        MinPieceCount,
        tests::{PartialMinPieceCount, big_puzzle_with_oris, oe},
    };

    #[test_log::test]
    fn iii_dominates() {
        let puzzle = big_puzzle_with_oris(&[180, 6, 5]);
        assert_eq!(
            MinPieceCount::from(&puzzle),
            PartialMinPieceCount {
                orientations_exps: vec![oe(180), oe(6), oe(5)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(180),
                has_even_parity_constraint: vec![false; 3],
            }
        );
    }

    #[test_log::test]
    fn iii_multi_dominates() {
        let puzzle = big_puzzle_with_oris(&[60, 6, 45]);
        // the prime powers matrix looks like
        // [2, 1, 1]
        // [1, 1, 0]
        // [0, 2, 1]
        //
        // so we expect
        //
        // [2, 0, 1] = 20
        // [0, 2, 0] = 9
        assert_eq!(
            MinPieceCount::from(&puzzle),
            PartialMinPieceCount {
                orientations_exps: vec![oe(60), oe(6), oe(45)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(180),
                has_even_parity_constraint: vec![false; 3],
            }
        );
    }

    #[test_log::test]
    fn iii_dominates_greater_eq() {
        // the prime powers matrix looks like
        // [2, 1, 1]
        // [1, 1, 0]
        // [2, 2, 1]
        //
        // so we expect the last to dominate everything because it has a higher
        // `eq_count`.
        let puzzle = big_puzzle_with_oris(&[60, 6, 180]);

        assert_eq!(
            MinPieceCount::from(&puzzle),
            PartialMinPieceCount {
                orientations_exps: vec![oe(60), oe(6), oe(180)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(180),
                has_even_parity_constraint: vec![false; 3],
            }
        );

        let puzzle = big_puzzle_with_oris(&[180, 6, 60]);

        assert_eq!(
            MinPieceCount::from(&puzzle),
            PartialMinPieceCount {
                orientations_exps: vec![oe(180), oe(6), oe(60)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(180),
                has_even_parity_constraint: vec![false; 3],
            }
        );
    }

    #[test_log::test]
    fn iii_dominates_equal_eq_chooses_first() {
        // the prime powers matrix looks like
        // [2, 0, 1]
        // [1, 1, 0]
        // [2, 1, 0]
        //
        // first wins the 2 and 5 prime powers, and the second wins the 3 prime power.
        // so we expect
        //
        // [2, 0, 1] = 20
        // [0, 1, 0] = 3
        let puzzle = big_puzzle_with_oris(&[20, 6, 12]);

        assert_eq!(
            MinPieceCount::from(&puzzle),
            PartialMinPieceCount {
                orientations_exps: vec![oe(20), oe(6), oe(12)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(60),
                has_even_parity_constraint: vec![false; 3],
            }
        );

        // the prime powers matrix looks like
        // [2, 2, 1]
        // [0, 1, 2]
        // [1, 0, 2]
        //
        // first wins the 2 and 3 prime powers, and the second wins the 3 prime power
        // because it is first. so we expect
        //
        // [2, 2, 0] = 36
        // [0, 0, 2] = 25
        let puzzle = big_puzzle_with_oris(&[180, 75, 50]);

        assert_eq!(
            MinPieceCount::from(&puzzle),
            PartialMinPieceCount {
                orientations_exps: vec![oe(180), oe(75), oe(50)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(900),
                has_even_parity_constraint: vec![false; 3],
            }
        );
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU16;

    use crate::{
        FIRST_129_PRIMES,
        min_piece_count::{MinPieceCount, prime_power_cycle_piece_count},
        orderexps::OrderExps,
        puzzle::{
            EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef,
            PuzzleDef,
        },
    };

    #[derive(Debug)]
    pub struct PartialMinPieceCount<const N: usize> {
        pub orientations_exps: Vec<OrderExps<N>>,
        pub leftover_prime_powers_mask: u64,
        pub orientations_exps_lcm: OrderExps<N>,
        pub has_even_parity_constraint: Vec<bool>,
    }

    impl<const N: usize> PartialEq<PartialMinPieceCount<N>> for MinPieceCount<'_, N> {
        fn eq(&self, other: &PartialMinPieceCount<N>) -> bool {
            let MinPieceCount {
                orientations_exps: orientations_exps_1,
                orbit_orientation_contributions: _,
                orbit_defs: _,
                leftover_prime_powers_mask: leftover_prime_powers_mask_1,
                orientations_exps_lcm: orientations_exps_lcm_1,
                has_even_parity_constraint: has_even_parity_constraint_1,
            } = self;
            let PartialMinPieceCount {
                orientations_exps,
                leftover_prime_powers_mask,
                orientations_exps_lcm,
                has_even_parity_constraint,
            } = other;
            *orientations_exps_1 == *orientations_exps
                && *leftover_prime_powers_mask_1 == *leftover_prime_powers_mask
                && *orientations_exps_lcm_1 == *orientations_exps_lcm
                && *has_even_parity_constraint_1 == *has_even_parity_constraint
        }
    }

    pub fn oe<const N: usize>(x: u16) -> OrderExps<N> {
        OrderExps::try_from(NonZeroU16::try_from(x).unwrap()).unwrap()
    }

    pub fn big_puzzle_with_oris(orientations: &[u8]) -> PuzzleDef<64> {
        puzzle_with_piece_count_and_oris(
            orientations
                .iter()
                .map(|&orientation| (312, orientation))
                .collect::<Vec<_>>()
                .as_slice(),
        )
    }

    pub fn puzzle_with_piece_count_and_oris(partial_orbit_defs: &[(u16, u8)]) -> PuzzleDef<64> {
        PuzzleDef::new(
            partial_orbit_defs
                .iter()
                .map(|&(piece_count, orientation)| PartialOrbitDef {
                    piece_count: piece_count.try_into().unwrap(),
                    orientation: if orientation == 1 {
                        OrientationStatus::CannotOrient
                    } else {
                        OrientationStatus::CanOrient {
                            count: orientation,
                            sum_constraint: OrientationSumConstraint::Zero,
                        }
                    },
                })
                .collect::<Vec<_>>(),
            EvenParityConstraints(vec![]),
        )
        .unwrap()
    }

    #[test_log::test]
    fn ii_suboptimal_piece_count() {
        let puzzle = puzzle_with_piece_count_and_oris(&[(8, 1), (17, 2)]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(1), oe(2)],
                leftover_prime_powers_mask: !0b1,
                orientations_exps_lcm: oe(2),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [3, 0, 0, 0, 0, 0, 1]
        //
        // orbit 1:
        // [0, 0, 0, 0, 0, 0, 0]
        // [3, 0, 0, 0, 0, 0, 1]
        //
        // orbit 2:
        // [1, 0, 0, 0, 0, 0, 0]
        // [2, 0, 0, 0, 0, 0, 1]
        //
        // 1: [0] => 1(pp) * 1(ori)
        // 2: [2] => 4(pp) * 2(ori) * 17(leftover)
        //
        // 17 + 4 = 21
        //
        // Note that this should be 25 since there are not enough pieces to fit
        assert_eq!(min_piece_count.calculate(&oe(136)).get(), 21);
    }

    #[test_log::test]
    fn iii_ones() {
        let puzzle = big_puzzle_with_oris(&[1, 1, 1]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(1), oe(1), oe(1)],
                leftover_prime_powers_mask: !0,
                orientations_exps_lcm: oe(1),
                has_even_parity_constraint: vec![false; 3],
            }
        );
        for i in 2..100 {
            let orderexps = oe(i);
            assert_eq!(
                min_piece_count.calculate(&orderexps).get(),
                orderexps
                    .0
                    .as_array()
                    .iter()
                    .zip(FIRST_129_PRIMES)
                    .map(|(&exp, prime)| prime_power_cycle_piece_count(prime, exp))
                    .sum::<u32>()
            );
        }
    }

    #[test_log::test]
    fn i_single_orbit() {
        let puzzle = big_puzzle_with_oris(&[3]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(3)],
                leftover_prime_powers_mask: !0b10,
                orientations_exps_lcm: oe(3),
                has_even_parity_constraint: vec![false],
            }
        );
        assert_eq!(min_piece_count.calculate(&oe(2)).get(), 2);
        assert_eq!(min_piece_count.calculate(&oe(3)).get(), 2);
        assert_eq!(min_piece_count.calculate(&oe(4)).get(), 4);
        assert_eq!(min_piece_count.calculate(&oe(5)).get(), 5);
        assert_eq!(min_piece_count.calculate(&oe(6)).get(), 3);

        for puzzle in [big_puzzle_with_oris(&[2]), big_puzzle_with_oris(&[2, 2])] {
            let mut min_piece_count = MinPieceCount::from(&puzzle);
            assert_eq!(min_piece_count.calculate(&oe(2)).get(), 2);
            assert_eq!(min_piece_count.calculate(&oe(4)).get(), 3);
            assert_eq!(min_piece_count.calculate(&oe(5)).get(), 5);
            assert_eq!(min_piece_count.calculate(&oe(6)).get(), 4);
            assert_eq!(min_piece_count.calculate(&oe(30)).get(), 8);
            // this test case produces the suboptimal result, but this will almost never
            // happen
            assert_eq!(min_piece_count.calculate(&oe(210)).get(), 15);
        }
    }

    #[test_log::test]
    fn ii_enough_leftover() {
        let puzzle = big_puzzle_with_oris(&[18, 9]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(18), oe(9)],
                leftover_prime_powers_mask: !0b11,
                orientations_exps_lcm: oe(18),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [3, 2, 1, 0, 2]
        //
        // orbit 1:
        // [1, 2, 0, 0, 0]
        // [2, 0, 1, 0, 2]
        //
        // orbit 2:
        // [0, 2, 0, 0, 0]
        // [3, 0, 1, 0, 2]
        //
        // 1: [2, 0] => 4(pp) * 18(ori) * 5(leftover)
        // 2: [0, 0] => 1(pp) * 1(ori) * 11^2(leftover)
        //
        // 4 + 5 + 11^2 = 130
        assert_eq!(min_piece_count.calculate(&oe(43560)).get(), 130);

        let puzzle = big_puzzle_with_oris(&[6, 3]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(6), oe(3)],
                leftover_prime_powers_mask: !0b11,
                orientations_exps_lcm: oe(6),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [3, 2, 1, 0, 2]
        //
        // orbit 1:
        // [1, 1, 0, 0, 0]
        // [2, 1, 1, 0, 2]
        //
        // orbit 2:
        // [0, 1, 0, 0, 0]
        // [3, 1, 1, 0, 2]
        //
        // 1: [2, 1] => 12(pp) * 6(ori) * 5(leftover)
        // 2: [0, 0] => 1(pp) * 1(ori) * 11^2(leftover)
        //
        // 4 + 3 + 5 + 11^2 = 133
        assert_eq!(min_piece_count.calculate(&oe(43560)).get(), 133);

        let puzzle = big_puzzle_with_oris(&[2, 12]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(2), oe(12)],
                leftover_prime_powers_mask: !0b11,
                orientations_exps_lcm: oe(12),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [3, 2, 1, 0, 2]
        //
        // orbit 1:
        // [1, 0, 0, 0, 0]
        // [2, 2, 1, 0, 2]
        //
        // orbit 2:
        // [2, 1, 0, 0, 0]
        // [1, 1, 1, 0, 2]
        //
        // 1: [0, 0] => 1(pp) * 1(ori) * 5(leftover)
        // 2: [1, 1] => 6(pp) * 12(ori) * 11^2(leftover)
        //
        // 2 + 3 + 5 + 11^2 = 131
        assert_eq!(min_piece_count.calculate(&oe(43560)).get(), 131);

        let puzzle = big_puzzle_with_oris(&[2, 3]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(2), oe(3)],
                leftover_prime_powers_mask: !0b11,
                orientations_exps_lcm: oe(6),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [3, 2, 1, 0, 2]
        //
        // orbit 1:
        // [1, 0]
        // [2, 2]
        //
        // orbit 2:
        // [0, 1]
        // [3, 1]
        //
        // 1: [2, 0] => 4(pp) * 2(ori) * 5(leftover)
        // 2: [0, 1] => 3(pp) * 3(ori) * 11^2(leftover)
        //
        // 4 + 3 + 5 + 11^2 = 133
        assert_eq!(min_piece_count.calculate(&oe(43560)).get(), 133);
    }

    #[test_log::test]
    fn ii_not_enough_leftover() {
        let puzzle = big_puzzle_with_oris(&[2, 3]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(2), oe(3)],
                leftover_prime_powers_mask: !0b11,
                orientations_exps_lcm: oe(6),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [3, 2, 1]
        //
        // orbit 1:
        // [1, 0]
        // [2, 2]
        //
        // orbit 2:
        // [0, 1]
        // [3, 1]
        //
        // 1: [2, 0] => 4(pp) * 2(ori) * 5(leftover)
        // 2: [0, 1] => 3(pp) * 3(ori) + 1(EXTRA)
        //
        // 4 + 3 + 5 + 1 = 13
        assert_eq!(min_piece_count.calculate(&oe(360)).get(), 13);

        let puzzle = big_puzzle_with_oris(&[1, 3]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(1), oe(3)],
                leftover_prime_powers_mask: !0b10,
                orientations_exps_lcm: oe(3),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [3, 2]
        //
        // orbit 1:
        // [0, 0]
        // [3, 2]
        //
        // orbit 2:
        // [0, 1]
        // [3, 1]
        //
        // 1: [0] => 1(pp) * 1(ori)
        // 2: [1] => 3(pp) * 3(ori) + 8(leftover)
        //
        // 3 + 8 = 11
        assert_eq!(min_piece_count.calculate(&oe(72)).get(), 11);
    }

    #[test_log::test]
    fn ii_no_leftover() {
        let puzzle = big_puzzle_with_oris(&[2, 3]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(2), oe(3)],
                leftover_prime_powers_mask: !0b11,
                orientations_exps_lcm: oe(6),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [3, 2]
        //
        // orbit 1:
        // [1, 0]
        // [2, 2]
        //
        // orbit 2:
        // [0, 1]
        // [3, 1]
        //
        // 1: [2, 0] => 4(pp) * 2(ori) + 1(EXTRA)
        // 2: [0, 1] => 3(pp) * 3(ori) + 1(EXTRA)
        //
        // 4 + 1 + 3 + 1 = 9
        assert_eq!(min_piece_count.calculate(&oe(72)).get(), 9);
    }

    #[test_log::test]
    fn iii_not_enough_leftover_in_same_orbit() {
        let puzzle = big_puzzle_with_oris(&[2, 8, 8]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(2), oe(8), oe(8)],
                leftover_prime_powers_mask: !0b1,
                orientations_exps_lcm: oe(8),
                has_even_parity_constraint: vec![false; 3],
            }
        );
        // [3, 2]
        //
        // orbit 1:
        // [1, 0]
        // [2, 2]
        //
        // orbit 2:
        // [3, 0]
        // [0, 2]
        //
        // orbit 3:
        // [3, 0]
        // [0, 2]
        //
        // 1: [0] => 1(pp) * 1(ori)
        // 2: [0] => 1(pp) * 1(ori)
        // 3: [0] => 1(pp) * 8(ori) * 9(leftover) + 1(EXTRA)
        //
        // 9 + 1 = 10
        assert_eq!(min_piece_count.calculate(&oe(72)).get(), 10);
    }

    #[test_log::test]
    fn iii_no_leftover_no_extra() {
        let puzzle = big_puzzle_with_oris(&[24, 72, 2]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(24), oe(72), oe(2)],
                leftover_prime_powers_mask: !0b11,
                orientations_exps_lcm: oe(72),
                has_even_parity_constraint: vec![false; 3],
            }
        );
        // [4, 3]
        //
        // orbit 1:
        // [1, 0]
        // [3, 3]
        //
        // orbit 2:
        // [3, 1]
        // [1, 2]
        //
        // orbit 3:
        // [3, 2]
        // [1, 1]
        //
        // 1: [0, 0] => 1(pp) * 1(ori)
        // 2: [0, 0] => 1(pp) * 1(ori)
        // 3: [1, 1] => 6(pp) * 72(ori)
        //
        // 2 + 3 = 5
        assert_eq!(min_piece_count.calculate(&oe(432)).get(), 5);
    }

    #[test_log::test]
    fn ii_no_leftover_no_extra() {
        let puzzle = big_puzzle_with_oris(&[2, 12]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(2), oe(12)],
                leftover_prime_powers_mask: !0b11,
                orientations_exps_lcm: oe(12),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [3, 2]
        //
        // orbit 1:
        // [1, 0]
        // [2, 2]
        //
        // orbit 2:
        // [2, 1]
        // [1, 1]
        //
        // 1: [0, 0] => 1(pp) * 1(ori)
        // 2: [1, 1] => 6(pp) * 12(ori)
        //
        // 3 + 2 = 5
        assert_eq!(min_piece_count.calculate(&oe(72)).get(), 5);

        let puzzle = big_puzzle_with_oris(&[6, 3]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(6), oe(3)],
                leftover_prime_powers_mask: !0b11,
                orientations_exps_lcm: oe(6),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [3, 2]
        //
        // orbit 1:
        // [1, 1]
        // [2, 1]
        //
        // orbit 2:
        // [0, 1]
        // [3, 1]
        //
        // 1: [2, 1] => 12(pp) * 6(ori)
        // 2: [0, 0] => 1(pp) * 1(ori)
        //
        // 4 + 3 = 7
        assert_eq!(min_piece_count.calculate(&oe(72)).get(), 7);
    }

    #[test_log::test]
    fn iii_eq_is_saturating() {
        let puzzle = big_puzzle_with_oris(&[60, 6, 45]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(60), oe(6), oe(45)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(180),
                has_even_parity_constraint: vec![false; 3],
            }
        );
        // [1, 1, 2]
        //
        // orbit 1:
        // [2, 1, 1]
        // [0, 0, 1]
        //
        // orbit 2:
        // [1, 1, 0]
        // [0, 0, 2]
        //
        // orbit 3:
        // [0, 2, 1]
        // [1, 0, 1]
        //
        // bad:
        //
        // 1: [0, 0, 1] => 5(pp) * 10(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        // 3: [0, 0, 0] => 1(pp) * 3(ori) + 2(EXTRA)
        //
        // 5 + 1 + 2 = 8
        //
        // good:
        //
        // 1: [0, 0, 1] => 5(pp) * 30(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        // 3: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 5 + 1 = 6
        assert_eq!(min_piece_count.calculate(&oe(150)).get(), 6);

        let puzzle = big_puzzle_with_oris(&[90, 6, 20]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(90), oe(6), oe(20)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(180),
                has_even_parity_constraint: vec![false; 3],
            }
        );
        // [1, 1, 2]
        //
        // orbit 1:
        // [1, 2, 1]
        // [0, 0, 1]
        //
        // orbit 2:
        // [1, 1, 0]
        // [0, 0, 2]
        //
        // orbit 3:
        // [2, 0, 1]
        // [0, 1, 1]
        //
        // bad:
        //
        // 1: [0, 0, 1] => 5(pp) * 15(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        // 3: [0, 0, 0] => 1(pp) * 2(ori) + 2(EXTRA)
        //
        // 5 + 1 + 2 = 8
        //
        // good:
        //
        // 1: [0, 0, 1] => 5(pp) * 30(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        // 3: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 5 + 1 = 6
        assert_eq!(min_piece_count.calculate(&oe(150)).get(), 6);
    }

    #[test_log::test]
    fn ii_only_extra() {
        let puzzle = big_puzzle_with_oris(&[100, 9]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(100), oe(9)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(900),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [1, 1, 1]
        //
        // orbit 1:
        // [2, 0, 2]
        // [0, 0, 0]
        //
        // orbit 2:
        // [0, 2, 0]
        // [0, 0, 0]
        //
        // 1: [0, 0, 0] => 1(pp) * 10(ori) + 2(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 3(ori) + 2(EXTRA)
        //
        // 2 + 2 = 4
        //
        // Note that we subdivide 100 into a +10 orientation cycle; we are always
        // allowed to do this because saturating the factor is guaranteed to make the
        // number a divisor.
        assert_eq!(min_piece_count.calculate(&oe(30)).get(), 4);
    }
}

#[cfg(test)]
mod transfer_two_cycle {
    use crate::{
        min_piece_count::{
            MinPieceCount,
            tests::{PartialMinPieceCount, big_puzzle_with_oris, oe},
        },
        puzzle::{
            EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef,
            PuzzleDef, cubeN::CUBE3,
        },
    };

    #[test_log::test]
    fn cube3_even_parity_constraint() {
        let cube3 = CUBE3.clone();

        let cube3_no_parity_constraint = PuzzleDef::<8>::new(
            vec![
                PartialOrbitDef {
                    piece_count: 8.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 3,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 12.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 2,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
            ],
            EvenParityConstraints(vec![]),
        )
        .unwrap();

        let cube3_corner_parity_constraint = PuzzleDef::<8>::new(
            vec![
                PartialOrbitDef {
                    piece_count: 8.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 3,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 12.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 2,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
            ],
            EvenParityConstraints(vec![vec![0]]),
        )
        .unwrap();

        let cube3_edge_parity_constraint = PuzzleDef::<8>::new(
            vec![
                PartialOrbitDef {
                    piece_count: 8.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 3,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 12.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 2,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
            ],
            EvenParityConstraints(vec![vec![1]]),
        )
        .unwrap();

        for (puzzle_def, expected_results) in [
            (&cube3, [(1260, 19), (990, 20), (495, 19), (3, 2), (2, 2)]),
            (
                &cube3_no_parity_constraint,
                [(1260, 17), (990, 20), (495, 19), (3, 2), (2, 2)],
            ),
            (
                &cube3_corner_parity_constraint,
                [(1260, 17), (990, 20), (495, 19), (3, 2), (2, 2)],
            ),
            (
                &cube3_edge_parity_constraint,
                [(1260, 19), (990, 20), (495, 19), (3, 2), (2, 2)],
            ),
        ] {
            let mut min_piece_count = MinPieceCount::from(puzzle_def);
            for (input_oe, expected_count) in expected_results {
                assert_eq!(
                    min_piece_count.calculate(&oe(input_oe)).get(),
                    expected_count,
                );
            }
        }
    }

    // TODO: more tests for no orientation constraint
    #[test_log::test]
    fn cube3_no_orientation_constraint() {
        let cube3 = CUBE3.clone();

        let cube3_no_orientation_constraint = PuzzleDef::<8>::new(
            vec![
                PartialOrbitDef {
                    piece_count: 8.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 3,
                        sum_constraint: OrientationSumConstraint::None,
                    },
                },
                PartialOrbitDef {
                    piece_count: 12.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 2,
                        sum_constraint: OrientationSumConstraint::None,
                    },
                },
            ],
            EvenParityConstraints(vec![vec![0, 1]]),
        )
        .unwrap();

        let cube3_corner_orientation_constraint = PuzzleDef::<8>::new(
            vec![
                PartialOrbitDef {
                    piece_count: 8.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 3,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 12.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 2,
                        sum_constraint: OrientationSumConstraint::None,
                    },
                },
            ],
            EvenParityConstraints(vec![vec![0, 1]]),
        )
        .unwrap();

        let cube3_edge_orientation_constraint = PuzzleDef::<8>::new(
            vec![
                PartialOrbitDef {
                    piece_count: 8.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 3,
                        sum_constraint: OrientationSumConstraint::None,
                    },
                },
                PartialOrbitDef {
                    piece_count: 12.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 2,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
            ],
            EvenParityConstraints(vec![vec![0, 1]]),
        )
        .unwrap();

        for (puzzle_def, expected_results) in [
            (&cube3, [(1260, 19), (990, 20), (495, 19), (3, 2), (2, 2)]),
            (
                &cube3_no_orientation_constraint,
                [(1260, 19), (990, 19), (495, 19), (3, 1), (2, 1)],
            ),
            (
                &cube3_corner_orientation_constraint,
                [(1260, 19), (990, 19), (495, 19), (3, 2), (2, 1)],
            ),
            (
                &cube3_edge_orientation_constraint,
                [(1260, 19), (990, 19), (495, 19), (3, 1), (2, 2)],
            ),
        ] {
            let mut min_piece_count = MinPieceCount::from(puzzle_def);
            for (input_oe, expected_count) in expected_results {
                assert_eq!(
                    min_piece_count.calculate(&oe(input_oe)).get(),
                    expected_count,
                );
            }
        }
    }

    #[test_log::test]
    fn only_extras() {
        let puzzle = big_puzzle_with_oris(&[2, 3]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(2), oe(3)],
                leftover_prime_powers_mask: !0b11,
                orientations_exps_lcm: oe(6),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [1, 1]
        //
        // orbit 1:
        // [1, 0]
        // [0, 1]
        //
        // orbit 2:
        // [0, 1]
        // [1, 0]
        //
        // bad:
        //
        // 1: [0, 0] => 1(pp) * 2(ori) + 2(EXTRA)
        // 2: [0, 0] => 1(pp) * 3(ori) + 2(EXTRA)
        //
        // 2 + 2 = 4
        //
        // good:
        //
        // 1: [0, 0] => 1(pp) * 1(ori)
        // 2: [1, 0] => 2(pp) * 3(ori) + 1(EXTRA)
        //
        // 2 + 1 = 3
        assert_eq!(min_piece_count.calculate(&oe(6)).get(), 3);

        let puzzle = big_puzzle_with_oris(&[15, 2]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(15), oe(2)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(30),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [1, 1, 1]
        //
        // orbit 1:
        // [0, 1, 1]
        // [1, 0, 0]
        //
        // orbit 2:
        // [1, 0, 0]
        // [0, 1, 1]
        //
        // bad case:
        //
        // 1: [0, 0, 0] => 1(pp) * 15(ori) + 2(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 2(ori) + 2(EXTRA)
        //
        // 2 + 2 = 4
        //
        // good case:
        //
        // 1: [1, 0, 0] => 2(pp) * 15(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 2 + 1 = 3
        assert_eq!(min_piece_count.calculate(&oe(30)).get(), 3);

        let puzzle = big_puzzle_with_oris(&[225, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(225), oe(4)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(900),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [1, 1, 1]
        //
        // orbit 1:
        // [0, 2, 2]
        // [1, 0, 0]
        //
        // orbit 2:
        // [2, 0, 0]
        // [0, 1, 1]
        //
        // bad case:
        //
        // 1: [0, 0, 0] => 1(pp) * 15(ori) + 2(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 2(ori) + 2(EXTRA)
        //
        // 2 + 2 = 4
        //
        // good case:
        //
        // 1: [1, 0, 0] => 2(pp) * 15(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 2 + 1 = 3
        assert_eq!(min_piece_count.calculate(&oe(30)).get(), 3);

        let puzzle = big_puzzle_with_oris(&[15, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(15), oe(4)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(60),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [1, 1, 1]
        //
        // orbit 1:
        // [0, 1, 1]
        // [1, 0, 0]
        //
        // orbit 2:
        // [2, 0, 0]
        // [0, 1, 1]
        //
        // bad case:
        //
        // 1: [0, 0, 0] => 1(pp) * 15(ori) + 2(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 4(ori) + 2(EXTRA)
        //
        // 2 + 2 = 4
        //
        // good case:
        //
        // 1: [1, 0, 0] => 2(pp) * 15(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 2 + 1 = 3
        assert_eq!(min_piece_count.calculate(&oe(30)).get(), 3);

        let puzzle = big_puzzle_with_oris(&[2, 3, 5]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(2), oe(3), oe(5)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(30),
                has_even_parity_constraint: vec![false; 3],
            }
        );
        // [1, 1, 1]
        //
        // orbit 1:
        // [1, 0, 0]
        // [0, 1, 1]
        //
        // orbit 2:
        // [0, 1, 0]
        // [1, 0, 1]
        //
        // orbit 3:
        // [0, 0, 1]
        // [1, 1, 0]
        //
        // bad:
        //
        // 1: [0, 0, 0] => 1(pp) * 2(ori) + 2(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 3(ori) + 2(EXTRA)
        // 3: [0, 0, 0] => 1(pp) * 5(ori) + 2(EXTRA)
        //
        // 2 + 2 = 4
        //
        // good:
        //
        // 1: [0, 0, 0] => 1(pp) * 1(ori)
        // 2: [1, 0, 0] => 2(pp) * 3(ori) + 1(EXTRA)
        // 3: [0, 0, 0] => 1(pp) * 5(ori) + 2(EXTRA)
        //
        // 2 + 1 = 3
        assert_eq!(min_piece_count.calculate(&oe(30)).get(), 5);
    }

    #[test_log::test]
    fn only_extras_even_orientations() {
        let puzzle = big_puzzle_with_oris(&[30, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(30), oe(4)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(60),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [2, 1, 1]
        //
        // orbit 1:
        // [1, 1, 1]
        // [1, 0, 0]
        //
        // orbit 2:
        // [2, 0, 0]
        // [0, 1, 1]
        //
        // bad:
        //
        // 1: [0, 0, 0] => 1(pp) * 15(ori) + 2(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 4(ori) + 2(EXTRA)
        //
        // 2 + 2 = 4
        //
        // good:
        //
        // 1: [1, 0, 0] => 2(pp) * 30(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 2 + 1 = 3
        assert_eq!(min_piece_count.calculate(&oe(60)).get(), 3);
    }

    #[test_log::test]
    fn not_only_extras() {
        let puzzle = big_puzzle_with_oris(&[15, 2]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(15), oe(2)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(30),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [1, 1, 2]
        //
        // orbit 1:
        // [0, 1, 1]
        // [1, 0, 1]
        //
        // orbit 2:
        // [1, 0, 0]
        // [0, 1, 2]
        //
        // bad:
        //
        // 1: [0, 0, 1] => 5(pp) * 15(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 2(ori) + 2(EXTRA)
        //
        // 5 + 1 + 2 = 8
        //
        // good:
        //
        // 1: [1, 0, 1] => 10(pp) * 15(ori)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 5 + 2 = 7
        assert_eq!(min_piece_count.calculate(&oe(150)).get(), 7);

        let puzzle = big_puzzle_with_oris(&[105, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(105), oe(4)],
                leftover_prime_powers_mask: !0b1111,
                orientations_exps_lcm: oe(420),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [1, 1, 1, 2]
        //
        // orbit 1:
        // [0, 1, 1, 1]
        // [1, 0, 0, 1]
        //
        // orbit 2:
        // [2, 0, 0, 0]
        // [0, 1, 1, 2]
        //
        // bad:
        //
        // 1: [0, 0, 0, 1] => 7(pp) * 105(ori) + 1(EXTRA)
        // 2: [0, 0, 0, 0] => 1(pp) * 2(ori) + 2(EXTRA)
        //
        // 7 + 1 + 2 = 10
        //
        // good:
        //
        // 1: [1, 0, 0, 1] => 14(pp) * 105(ori)
        // 2: [0, 0, 0, 0] => 1(pp) * 1(ori)
        //
        // 7 + 2 = 9
        assert_eq!(min_piece_count.calculate(&oe(1470)).get(), 9);

        let puzzle = big_puzzle_with_oris(&[2, 3, 5]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(2), oe(3), oe(5)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(30),
                has_even_parity_constraint: vec![false; 3],
            }
        );
        // [2, 1, 1]
        //
        // orbit 1:
        // [1, 0, 0]
        // [0, 1, 1]
        //
        // orbit 2:
        // [0, 1, 0]
        // [1, 0, 1]
        //
        // orbit 3:
        // [0, 0, 1]
        // [1, 1, 0]
        //
        // equal:
        //
        // 1: [1, 0, 0] => 2(pp) * 2(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 3(ori) + 2(EXTRA)
        // 3: [0, 0, 0] => 1(pp) * 5(ori) + 2(EXTRA)
        //
        // 2 + 1 + 2 + 2 = 7
        //
        // equal:
        //
        // 1: [0, 0, 0] => 1(pp) * 1(ori)
        // 2: [2, 0, 0] => 4(pp) * 3(ori) + 1(EXTRA)
        // 3: [0, 0, 0] => 1(pp) * 5(ori) + 2(EXTRA)
        //
        // 2 + 2 + 1 + 2 = 7
        assert_eq!(min_piece_count.calculate(&oe(60)).get(), 7);
    }

    #[test_log::test]
    fn not_only_extras_even_orientations() {
        let puzzle = big_puzzle_with_oris(&[30, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(30), oe(4)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(60),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [2, 1, 2]
        //
        // orbit 1:
        // [1, 1, 1]
        // [1, 0, 1]
        //
        // orbit 2:
        // [2, 0, 0]
        // [0, 1, 2]
        //
        // bad:
        //
        // 1: [0, 0, 1] => 5(pp) * 15(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 4(ori) + 2(EXTRA)
        //
        // 5 + 1 + 2 = 8
        //
        // good:
        //
        // 1: [1, 0, 1] => 10(pp) * 30(ori)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 5 + 2 = 7
        assert_eq!(min_piece_count.calculate(&oe(300)).get(), 7);
    }

    #[test_log::test]
    fn no_transfer_edge_cases() {
        let puzzle = big_puzzle_with_oris(&[3, 4, 70]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(3), oe(4), oe(70)],
                leftover_prime_powers_mask: !0b1111,
                orientations_exps_lcm: oe(420),
                has_even_parity_constraint: vec![false; 3],
            }
        );
        // [2, 2, 2, 2]
        //
        // orbit 1:
        // [0, 1, 0, 0]
        // [2, 1, 2, 2]
        //
        // orbit 2:
        // [2, 0, 0, 0]
        // [0, 1, 2, 2]
        //
        // orbit 3:
        // [1, 0, 1, 1]
        // [1, 2, 1, 1]
        //
        // equal:
        //
        // 1: [0, 1, 0, 0] => 3(pp) * 3(ori) + 1(EXTRA)
        // 2: [0, 0, 0, 0] => 1(pp) * 4(ori) + 2(EXTRA)
        // 3: [0, 0, 1, 1] => 35(pp) * 35(ori)
        //
        // 3 + 2 + 7 + 5 = 18
        //
        // equal:
        //
        // 1: [0, 1, 0, 0] => 3(pp) * 3(ori) + 1(EXTRA)
        // 2: [0, 0, 0, 0] => 1(pp) * 1(ori)
        // 3: [1, 0, 1, 1] => 70(pp) * 70(ori)
        //
        // 3 + 1 + 2 + 5 + 7 = 18
        assert_eq!(min_piece_count.calculate(&oe(44100)).get(), 18);

        let puzzle = big_puzzle_with_oris(&[30, 8]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(30), oe(8)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(120),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [3, 1, 2]
        //
        // orbit 1:
        // [1, 1, 1]
        // [2, 0, 1]
        //
        // orbit 2:
        // [3, 0, 0]
        // [0, 1, 2]
        //
        // good:
        //
        // 1: [0, 0, 1] => 5(pp) * 15(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 8(ori) + 2(EXTRA)
        //
        // 5 + 1 + 2 = 8
        //
        // bad:
        //
        // 1: [2, 0, 1] => 40(pp) * 30(ori)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 4 + 5 = 9
        assert_eq!(min_piece_count.calculate(&oe(600)).get(), 8);

        let puzzle = big_puzzle_with_oris(&[10, 3]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(10), oe(3)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(30),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [1, 1, 2]
        //
        // orbit 1:
        // [1, 0, 1]
        // [0, 1, 1]
        //
        // orbit 2:
        // [0, 1, 0]
        // [1, 0, 2]
        //
        // equal:
        //
        // 1: [0, 0, 1] => 5(pp) * 10(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 3(ori) + 2(EXTRA)
        //
        // 5 + 1 + 2 = 8
        //
        // equal:
        //
        // 1: [0, 1, 1] => 15(pp) * 10(ori)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 3 + 5 = 8
        assert_eq!(min_piece_count.calculate(&oe(150)).get(), 8);

        let puzzle = big_puzzle_with_oris(&[5, 6]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(5), oe(6)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(30),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [1, 1, 1, 1]
        //
        // orbit 1:
        // [0, 0, 1, 0]
        // [1, 1, 0, 1]
        //
        // orbit 2:
        // [1, 1, 0, 0]
        // [0, 0, 1, 1]
        //
        // good:
        //
        // 1: [0, 0, 0] => 1(pp) * 5(ori) * 7(leftover) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 6(ori) + 2(EXTRA)
        //
        // 7 + 1 + 2 = 10
        //
        // bad:
        //
        // 1: [1, 0, 0] => 2(pp) * 5(ori) * 7(leftover)
        // 2: [0, 0, 0] => 1(pp) * 3(ori) * 2(EXTRA)
        //
        // 2 + 7 + 2 = 11
        assert_eq!(min_piece_count.calculate(&oe(210)).get(), 10);

        let puzzle = big_puzzle_with_oris(&[210, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(210), oe(4)],
                leftover_prime_powers_mask: !0b1111,
                orientations_exps_lcm: oe(420),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [2, 1, 2, 2]
        //
        // orbit 1:
        // [1, 1, 1, 1]
        // [1, 0, 1, 1]
        //
        // orbit 2:
        // [2, 0, 0, 0]
        // [0, 1, 2, 2]
        //
        // equal:
        //
        // 1: [0, 0, 1, 1] => 35(pp) * 105(ori)
        // 2: [0, 0, 0, 0] => 1(pp) * 4(ori) + 2(EXTRA)
        //
        // 7 + 5 + 2 = 14
        //
        // equal:
        //
        // 1: [1, 0, 1, 1] => 70(pp) * 210(ori)
        // 2: [0, 0, 0, 0] => 1(pp) * 1(ori)
        //
        // 7 + 5 + 2 = 14
        assert_eq!(min_piece_count.calculate(&oe(14700)).get(), 14);

        let puzzle = big_puzzle_with_oris(&[15, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(15), oe(4)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(60),
                has_even_parity_constraint: vec![false; 2],
            }
        );
        // [2, 1, 1]
        //
        // orbit 1:
        // [0, 1, 1]
        // [2, 0, 0]
        //
        // orbit 2:
        // [2, 0, 0]
        // [0, 1, 1]
        //
        // bad:
        //
        // 1: [2, 0, 0] => 4(pp) * 15(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 2 + 2 + 1 = 5
        //
        // good:
        //
        // 1: [0, 0, 0] => 1(pp) * 15(ori) + 2(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 4(ori) + 2(EXTRA)
        //
        // 2 + 2 = 4
        assert_eq!(min_piece_count.calculate(&oe(60)).get(), 4);
    }
}

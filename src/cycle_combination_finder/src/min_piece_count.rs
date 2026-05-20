use std::{
    num::{NonZeroU16, NonZeroU32},
    simd::{Simd, cmp::SimdPartialEq},
};

use crate::{FIRST_129_PRIMES, orderexps::OrderExps, puzzle::PuzzleDef};

#[derive(Debug)]
pub struct MinPieceCount<const N: usize> {
    orientations_exps: Vec<OrderExps<N>>,
    orbit_orientation_contributions: Vec<OrderExps<N>>,
    leftover_prime_powers_mask: u64,
    orientations_exps_lcm: OrderExps<N>,
}

impl<const N: usize> From<&PuzzleDef<N>> for MinPieceCount<N> {
    fn from(puzzle_def: &PuzzleDef<N>) -> Self {
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

        Self {
            orientations_exps,
            orbit_orientation_contributions,
            leftover_prime_powers_mask,
            orientations_exps_lcm,
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

impl<const N: usize> MinPieceCount<N> {
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
            // TOOD: is this really needed?
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

        for (prime_power_index, orbit) in prime_power_to_orbit.into_iter().enumerate() {
            let Some((orbit_index, _)) = orbit else {
                continue;
            };
            self.orbit_orientation_contributions[orbit_index].0[prime_power_index] =
                self.orientations_exps_lcm.0[prime_power_index];
        }

        // The maximum number of contributing orbits is the max N, one for every prime.
        // Thus this fits into a u32.
        let mut needing_orientation_cycles_count = 0u32;
        let mut min_piece_count = leftover_prime_powers_sum;
        for orbit_orientation_contribution in &self.orbit_orientation_contributions {
            let mut contributing_prime_powers = orbit_orientation_contribution
                .0
                .simd_ne(Simd::splat(0))
                .to_bitmask();
            if contributing_prime_powers == 0 {
                continue;
            }
            let mut needs_orientation_cycles_count = true;
            while contributing_prime_powers != 0 {
                let prime_power_index = contributing_prime_powers.trailing_zeros() as usize;
                let exp = required_cycle_prime_powers.0[prime_power_index];
                let prime = FIRST_129_PRIMES[prime_power_index];
                let cycle_piece_count = prime_power_cycle_piece_count(prime, exp);
                if cycle_piece_count != 0 {
                    needs_orientation_cycles_count = false;
                    min_piece_count += cycle_piece_count;
                }
                contributing_prime_powers ^= contributing_prime_powers.isolate_lowest_one();
            }
            if needs_orientation_cycles_count {
                needing_orientation_cycles_count += 1;
            }
        }
        min_piece_count +=
            needing_orientation_cycles_count.saturating_sub(leftover_prime_powers_count);

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
mod tests {
    use std::num::NonZeroU16;

    use crate::{
        FIRST_129_PRIMES,
        min_piece_count::{MinPieceCount, prime_power_cycle_piece_count},
        orderexps::OrderExps,
        puzzle::{
            EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef,
            PuzzleDef, cubeN::CUBE3,
        },
    };

    #[derive(Debug)]
    pub struct PartialMinPieceCount<const N: usize> {
        pub orientations_exps: Vec<OrderExps<N>>,
        pub leftover_prime_powers_mask: u64,
        pub orientations_exps_lcm: OrderExps<N>,
    }

    impl<const N: usize> PartialEq<PartialMinPieceCount<N>> for MinPieceCount<N> {
        fn eq(&self, other: &PartialMinPieceCount<N>) -> bool {
            let MinPieceCount {
                orientations_exps: orientations_exps_1,
                orbit_orientation_contributions: _,
                leftover_prime_powers_mask: leftover_prime_powers_mask_1,
                orientations_exps_lcm: orientations_exps_lcm_1,
            } = self;
            let PartialMinPieceCount {
                orientations_exps,
                leftover_prime_powers_mask,
                orientations_exps_lcm,
            } = other;
            *orientations_exps_1 == *orientations_exps
                && *leftover_prime_powers_mask_1 == *leftover_prime_powers_mask
                && *orientations_exps_lcm_1 == *orientations_exps_lcm
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
            }
        );
        assert_eq!(min_piece_count.calculate(&oe(2)).get(), 2);
        assert_eq!(min_piece_count.calculate(&oe(3)).get(), 1);
        assert_eq!(min_piece_count.calculate(&oe(4)).get(), 4);
        assert_eq!(min_piece_count.calculate(&oe(5)).get(), 5);
        assert_eq!(min_piece_count.calculate(&oe(6)).get(), 2);

        for puzzle in [big_puzzle_with_oris(&[2]), big_puzzle_with_oris(&[2, 2])] {
            let mut min_piece_count = MinPieceCount::from(&puzzle);
            assert_eq!(min_piece_count.calculate(&oe(2)).get(), 1);
            assert_eq!(min_piece_count.calculate(&oe(4)).get(), 2);
            assert_eq!(min_piece_count.calculate(&oe(5)).get(), 5);
            assert_eq!(min_piece_count.calculate(&oe(6)).get(), 3);
            assert_eq!(min_piece_count.calculate(&oe(30)).get(), 8);
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
        // 2: [0, 1] => 3(pp) * 3(ori)
        //
        // 4 + 3 + 5 = 12
        assert_eq!(min_piece_count.calculate(&oe(360)).get(), 12);

        let puzzle = big_puzzle_with_oris(&[1, 3]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(1), oe(3)],
                leftover_prime_powers_mask: !0b10,
                orientations_exps_lcm: oe(3),
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
        // 1: [2, 0] => 4(pp) * 2(ori)
        // 2: [0, 1] => 3(pp) * 3(ori)
        //
        // 4 + 3 = 7
        assert_eq!(min_piece_count.calculate(&oe(72)).get(), 7);
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
        // 3: [0] => 1(pp) * 8(ori) * 9(leftover)
        //
        // 9 = 9
        assert_eq!(min_piece_count.calculate(&oe(72)).get(), 9);
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
        // equal:
        //
        // 1: [0, 0, 1] => 5(pp) * 10(ori)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        // 3: [0, 0, 0] => 1(pp) * 3(ori)
        //
        // 5 = 5
        //
        // equal:
        //
        // 1: [0, 0, 1] => 5(pp) * 30(ori)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        // 3: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 5 = 5
        assert_eq!(min_piece_count.calculate(&oe(150)).get(), 5);

        let puzzle = big_puzzle_with_oris(&[90, 6, 20]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(90), oe(6), oe(20)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(180),
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
        // 1: [0, 0, 1] => 5(pp) * 15(ori)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        // 3: [0, 0, 0] => 1(pp) * 2(ori) + 1(EXTRA)
        //
        // 5 + 1 = 6
        //
        // good:
        //
        // 1: [0, 0, 1] => 5(pp) * 30(ori)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        // 3: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 5 = 5
        assert_eq!(min_piece_count.calculate(&oe(150)).get(), 5);
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
        // 1: [0, 0, 0] => 1(pp) * 10(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 3(ori) + 1(EXTRA)
        //
        // 1 + 1 = 2
        //
        // Note that we subdivide 100 into a +10 orientation cycle; we are always
        // allowed to do this because saturating the factor is guaranteed to make the
        // number a divisor.
        assert_eq!(min_piece_count.calculate(&oe(30)).get(), 2);
    }

    #[test_log::test]
    fn cube3_even_parity_constraint() {
        let cube3 = CUBE3.clone();

        let cube3_no_parity_constraint = PuzzleDef::<8>::new(
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
                        sum_constraint: OrientationSumConstraint::None,
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
            EvenParityConstraints(vec![vec![1]]),
        )
        .unwrap();

        for (puzzle_def, expected_results) in [
            (&cube3, [(1260, 17), (990, 19), (495, 19), (3, 1), (2, 1)]),
            (
                &cube3_no_parity_constraint,
                [(1260, 17), (990, 19), (495, 19), (3, 1), (2, 1)],
            ),
            (
                &cube3_corner_parity_constraint,
                [(1260, 17), (990, 19), (495, 19), (3, 1), (2, 1)],
            ),
            (
                &cube3_edge_parity_constraint,
                [(1260, 17), (990, 19), (495, 19), (3, 1), (2, 1)],
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
        // 1: [0, 0] => 1(pp) * 2(ori) + 1(EXTRA)
        // 2: [0, 0] => 1(pp) * 3(ori) + 1(EXTRA)
        //
        // 1 + 1 = 2
        assert_eq!(min_piece_count.calculate(&oe(6)).get(), 2);

        let puzzle = big_puzzle_with_oris(&[15, 2]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(15), oe(2)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(30),
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
        // 1: [0, 0, 0] => 1(pp) * 15(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 2(ori) + 1(EXTRA)
        //
        // 1 + 1 = 2
        assert_eq!(min_piece_count.calculate(&oe(30)).get(), 2);

        let puzzle = big_puzzle_with_oris(&[225, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(225), oe(4)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(900),
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
        // 1: [0, 0, 0] => 1(pp) * 15(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 2(ori) + 1(EXTRA)
        //
        // 1 + 1 = 2
        assert_eq!(min_piece_count.calculate(&oe(30)).get(), 2);

        let puzzle = big_puzzle_with_oris(&[15, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(15), oe(4)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(60),
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
        // 1: [0, 0, 0] => 1(pp) * 15(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 4(ori) + 1(EXTRA)
        //
        // 1 + 1 = 2
        assert_eq!(min_piece_count.calculate(&oe(30)).get(), 2);

        let puzzle = big_puzzle_with_oris(&[2, 3, 5]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(2), oe(3), oe(5)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(30),
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
        // 1: [0, 0, 0] => 1(pp) * 2(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 3(ori) + 1(EXTRA)
        // 3: [0, 0, 0] => 1(pp) * 5(ori) + 1(EXTRA)
        //
        // 1 + 1 + 1 = 3
        assert_eq!(min_piece_count.calculate(&oe(30)).get(), 3);
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
        // 1: [0, 0, 0] => 1(pp) * 15(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 4(ori) + 1(EXTRA)
        //
        // 1 + 1 = 2
        assert_eq!(min_piece_count.calculate(&oe(60)).get(), 2);
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
        // 1: [0, 0, 1] => 5(pp) * 15(ori)
        // 2: [0, 0, 0] => 1(pp) * 2(ori) + 1(EXTRA)
        //
        // 5 + 1 = 6
        assert_eq!(min_piece_count.calculate(&oe(150)).get(), 6);

        let puzzle = big_puzzle_with_oris(&[105, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(105), oe(4)],
                leftover_prime_powers_mask: !0b1111,
                orientations_exps_lcm: oe(420),
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
        // 1: [0, 0, 0, 1] => 7(pp) * 105(ori)
        // 2: [0, 0, 0, 0] => 1(pp) * 2(ori) + 1(EXTRA)
        //
        // 7 + 1 = 8
        assert_eq!(min_piece_count.calculate(&oe(1470)).get(), 8);

        let puzzle = big_puzzle_with_oris(&[2, 3, 5]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(2), oe(3), oe(5)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(30),
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
        // 1: [1, 0, 0] => 2(pp) * 2(ori)
        // 2: [0, 0, 0] => 1(pp) * 3(ori) + 1(EXTRA)
        // 3: [0, 0, 0] => 1(pp) * 5(ori) + 1(EXTRA)
        //
        // 2 + 1 + 1 = 4
        assert_eq!(min_piece_count.calculate(&oe(60)).get(), 4);
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
        // 1: [0, 0, 1] => 5(pp) * 15(ori)
        // 2: [0, 0, 0] => 1(pp) * 4(ori) + 1(EXTRA)
        //
        // 5 + 1 = 6
        assert_eq!(min_piece_count.calculate(&oe(300)).get(), 6);
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
        // 1: [0, 1, 0, 0] => 3(pp) * 3(ori)
        // 2: [0, 0, 0, 0] => 1(pp) * 4(ori) + 1(EXTRA)
        // 3: [0, 0, 1, 1] => 35(pp) * 35(ori)
        //
        // 3 + 1 + 7 + 5 = 16
        assert_eq!(min_piece_count.calculate(&oe(44100)).get(), 16);

        let puzzle = big_puzzle_with_oris(&[30, 8]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(30), oe(8)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(120),
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
        // 1: [0, 0, 1] => 5(pp) * 15(ori)
        // 2: [0, 0, 0] => 1(pp) * 8(ori) + 1(EXTRA)
        //
        // 5 + 1 = 6
        assert_eq!(min_piece_count.calculate(&oe(600)).get(), 6);

        let puzzle = big_puzzle_with_oris(&[10, 3]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(10), oe(3)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(30),
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
        // 1: [0, 0, 1] => 5(pp) * 10(ori)
        // 2: [0, 0, 0] => 1(pp) * 3(ori) + 1(EXTRA)
        //
        // 5 + 1 = 6
        assert_eq!(min_piece_count.calculate(&oe(150)).get(), 6);

        let puzzle = big_puzzle_with_oris(&[5, 6]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(5), oe(6)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(30),
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
        // 1: [0, 0, 0] => 1(pp) * 5(ori) * 7(leftover)
        // 2: [0, 0, 0] => 1(pp) * 6(ori) + 1(EXTRA)
        //
        // 7 + 1 = 8
        assert_eq!(min_piece_count.calculate(&oe(210)).get(), 8);

        let puzzle = big_puzzle_with_oris(&[210, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(210), oe(4)],
                leftover_prime_powers_mask: !0b1111,
                orientations_exps_lcm: oe(420),
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
        // 1: [0, 0, 1, 1] => 35(pp) * 105(ori)
        // 2: [0, 0, 0, 0] => 1(pp) * 4(ori) + 1(EXTRA)
        //
        // 7 + 5 + 1 = 13
        assert_eq!(min_piece_count.calculate(&oe(14700)).get(), 13);

        let puzzle = big_puzzle_with_oris(&[15, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            PartialMinPieceCount {
                orientations_exps: vec![oe(15), oe(4)],
                leftover_prime_powers_mask: !0b111,
                orientations_exps_lcm: oe(60),
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
        // 1: [0, 0, 0] => 1(pp) * 15(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 4(ori) + 1(EXTRA)
        //
        // 1 + 1 = 2
        assert_eq!(min_piece_count.calculate(&oe(60)).get(), 2);
    }
}

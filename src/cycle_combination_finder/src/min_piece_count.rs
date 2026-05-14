use std::{
    num::{NonZeroU16, NonZeroU32},
    simd::{Simd, cmp::SimdPartialEq, num::SimdUint},
};

use crate::{
    FIRST_129_PRIMES,
    orderexps::OrderExps,
    puzzle::{
        EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef,
        PuzzleDef,
    },
};

#[derive(Debug, PartialEq)]
pub struct MinPieceCount<const N: usize> {
    leftover_prime_powers_mask: u64,
    orbit_orientation_contributions: Vec<OrderExps<N>>,
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
        let mut belongs_to: [Option<(usize, u32)>; N] = [None; N];
        for (orbit_index, orientation_exps) in orientations_exps.iter().enumerate() {
            let mut eq = orientation_exps
                .0
                .simd_eq(orientations_exps_lcm.0)
                .to_bitmask();
            let eq_count = eq.count_ones();
            while eq != 0 {
                #[allow(clippy::cast_possible_truncation)]
                let prime_power_index = usize::from(eq.trailing_zeros() as u16);
                if (leftover_prime_powers_mask >> prime_power_index) & 1 == 0 {
                    match &mut belongs_to[prime_power_index] {
                        Some(belonging) => {
                            if eq_count > belonging.1 {
                                *belonging = (orbit_index, eq_count);
                            }
                        }
                        empty @ None => {
                            *empty = Some((orbit_index, eq_count));
                        }
                    }
                }
                eq ^= eq.isolate_lowest_one();
            }
        }

        let mut orbit_orientation_contributions = orientations_exps;
        orbit_orientation_contributions.fill(OrderExps::one());

        for (prime_power_index, belonging) in belongs_to.into_iter().enumerate() {
            let Some((orbit_index, _)) = belonging else {
                continue;
            };
            orbit_orientation_contributions[orbit_index].0[prime_power_index] =
                orientations_exps_lcm.0[prime_power_index];
        }

        orbit_orientation_contributions.retain(|orbit_orientation_contribution| {
            orbit_orientation_contribution.0 != Simd::splat(0)
        });

        Self {
            leftover_prime_powers_mask,
            orbit_orientation_contributions,
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
    // TODO: only work when orientation sum constraint is Zero
    // TODO: special case C2
    // even parity constraints
    // piece count factors?
    pub fn calculate(&self, possible_order: &OrderExps<N>) -> NonZeroU32 {
        assert_ne!(possible_order, &OrderExps::one());
        let mut leftover_prime_powers_sum = 0;
        let mut leftover_prime_power_count = 0;
        let mut leftover_prime_powers_mask = self.leftover_prime_powers_mask;
        while leftover_prime_powers_mask != 0 {
            let prime_power_index = leftover_prime_powers_mask.trailing_zeros() as usize;
            let exp = possible_order.0[prime_power_index];
            let prime = FIRST_129_PRIMES[prime_power_index];

            let leftover_prime_power = prime_power_cycle_piece_count(prime, exp);
            if leftover_prime_power != 0 {
                leftover_prime_powers_sum += leftover_prime_power;
                leftover_prime_power_count += 1;
            }
            leftover_prime_powers_mask ^= leftover_prime_powers_mask.isolate_lowest_one();
        }

        // The maximum number of contributing orbits is the max N, one for every prime.
        // Thus this fits into a u16.
        let mut needing_orientation_cycles_count = 0u32;
        let mut min_piece_count = leftover_prime_powers_sum;
        for orbit_orientation_contribution in &self.orbit_orientation_contributions {
            let mut contributing_prime_powers = orbit_orientation_contribution
                .0
                .simd_ne(Simd::splat(0))
                .to_bitmask();
            let mut cycles_count = 0u32;
            while contributing_prime_powers != 0 {
                let prime_power_index = contributing_prime_powers.trailing_zeros() as usize;
                let exp = possible_order.0[prime_power_index]
                    .saturating_sub(orbit_orientation_contribution.0[prime_power_index]);
                let prime = FIRST_129_PRIMES[prime_power_index];
                let cycle_piece_count = prime_power_cycle_piece_count(prime, exp);
                if cycle_piece_count != 0 {
                    min_piece_count += cycle_piece_count;
                    cycles_count += 1;
                }

                contributing_prime_powers ^= contributing_prime_powers.isolate_lowest_one();
            }
            needing_orientation_cycles_count += 2u32.saturating_sub(cycles_count);
        }
        min_piece_count +=
            needing_orientation_cycles_count.saturating_sub(leftover_prime_power_count);

        debug_assert!(
            min_piece_count
                >= possible_order
                    .0
                    .saturating_sub(self.orientations_exps_lcm.0)
                    .as_array()
                    .iter()
                    .zip(FIRST_129_PRIMES)
                    .map(|(&exp, prime)| prime_power_cycle_piece_count(prime, exp))
                    .sum::<u32>()
        );
        NonZeroU32::new(min_piece_count).unwrap()
    }
}

#[allow(dead_code)]
fn oe<const N: usize>(x: u16) -> OrderExps<N> {
    OrderExps::try_from(NonZeroU16::try_from(x).unwrap()).unwrap()
}

#[allow(dead_code)]
fn big_puzzle_with_oris(orientations: &[u8]) -> PuzzleDef<64> {
    puzzle_with_piece_count_and_oris(
        orientations
            .iter()
            .map(|&orientation| (100, orientation))
            .collect::<Vec<_>>()
            .as_slice(),
    )
}

#[allow(dead_code)]
fn puzzle_with_piece_count_and_oris(partial_orbit_defs: &[(u16, u8)]) -> PuzzleDef<64> {
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

#[cfg(test)]
mod initialization {
    use crate::{
        min_piece_count::{MinPieceCount, big_puzzle_with_oris, oe},
        puzzle::cubeN::CUBE3,
    };

    #[test_log::test]
    fn cube3() {
        let cube3 = CUBE3.clone();
        assert_eq!(
            MinPieceCount::from(&cube3),
            MinPieceCount {
                leftover_prime_powers_mask: u64::from(!0b11u8),
                orbit_orientation_contributions: vec![oe(3), oe(2)],
                orientations_exps_lcm: oe(6),
            }
        );
    }

    #[test_log::test]
    fn orbit_orientation_dominates() {
        let puzzle = big_puzzle_with_oris(&[180, 6, 5]);
        assert_eq!(
            MinPieceCount::from(&puzzle),
            MinPieceCount {
                leftover_prime_powers_mask: !0b111,
                orbit_orientation_contributions: vec![oe(180)],
                orientations_exps_lcm: oe(180),
            }
        );
    }

    #[test_log::test]
    fn orbit_orientation_multi_dominates() {
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
            MinPieceCount {
                leftover_prime_powers_mask: !0b111,
                orbit_orientation_contributions: vec![oe(20), oe(9)],
                orientations_exps_lcm: oe(180),
            }
        );
    }

    #[test_log::test]
    fn orbit_orientation_dominates_with_more_eq() {
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
            MinPieceCount {
                leftover_prime_powers_mask: !0b111,
                orbit_orientation_contributions: vec![oe(180)],
                orientations_exps_lcm: oe(180),
            }
        );

        let puzzle = big_puzzle_with_oris(&[180, 6, 60]);

        assert_eq!(
            MinPieceCount::from(&puzzle),
            MinPieceCount {
                leftover_prime_powers_mask: !0b111,
                orbit_orientation_contributions: vec![oe(180)],
                orientations_exps_lcm: oe(180),
            }
        );
    }

    #[test_log::test]
    fn ambiguous_orientation_chooses_first() {
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
            MinPieceCount {
                leftover_prime_powers_mask: !0b111,
                orbit_orientation_contributions: vec![oe(20), oe(3)],
                orientations_exps_lcm: oe(60),
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
            MinPieceCount {
                leftover_prime_powers_mask: !0b111,
                orbit_orientation_contributions: vec![oe(36), oe(25)],
                orientations_exps_lcm: oe(900),
            }
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        FIRST_129_PRIMES,
        min_piece_count::{
            MinPieceCount, big_puzzle_with_oris, oe, prime_power_cycle_piece_count,
            puzzle_with_piece_count_and_oris,
        },
    };

    #[test_log::test]
    fn daniels_edge_case() {
        let puzzle = puzzle_with_piece_count_and_oris(&[(8, 1), (17, 2)]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b1,
                orbit_orientation_contributions: vec![oe(2)],
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
        assert_eq!(min_piece_count.calculate(&oe(136)).get(), 21);
    }

    #[test_log::test]
    fn orientation_count_one() {
        let puzzle = big_puzzle_with_oris(&[1, 1, 1]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0,
                orbit_orientation_contributions: vec![],
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
    fn dominates_enough_leftover() {
        let puzzle = big_puzzle_with_oris(&[18, 9]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b11,
                orbit_orientation_contributions: vec![oe(18)],
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
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b11,
                orbit_orientation_contributions: vec![oe(6)],
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
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b11,
                orbit_orientation_contributions: vec![oe(12)],
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
    }

    #[test_log::test]
    fn multi_dominates_enough_leftover() {
        let puzzle = big_puzzle_with_oris(&[2, 3]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b11,
                orbit_orientation_contributions: vec![oe(2), oe(3)],
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
    fn dominates_not_enough_leftover() {
        let puzzle = big_puzzle_with_oris(&[1, 3]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b10,
                orbit_orientation_contributions: vec![oe(3)],
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
    fn multi_dominates_no_leftover() {
        let puzzle = big_puzzle_with_oris(&[2, 3]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b11,
                orbit_orientation_contributions: vec![oe(2), oe(3)],
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
        // 1: [2, 0] => 4(pp) * 2(ori) + 1(EXTRA)
        // 2: [0, 1] => 3(pp) * 3(ori) + 1(EXTRA)
        //
        // 4 + 1 + 3 + 1 = 9
        assert_eq!(min_piece_count.calculate(&oe(72)).get(), 9);
    }

    #[test_log::test]
    fn nontrivial_extra_cycles() {
        let puzzle = big_puzzle_with_oris(&[2, 8, 8]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b1,
                orbit_orientation_contributions: vec![oe(8)],
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
        // 3: [0] => 1(pp) * 8(ori) * 9(leftover) + 1(EXTRA)
        //
        // 9 + 1 = 10
        assert_eq!(min_piece_count.calculate(&oe(72)).get(), 10);

        let puzzle = big_puzzle_with_oris(&[5, 6]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b111,
                orbit_orientation_contributions: vec![oe(5), oe(6)],
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
        // 1: [0, 0, 0] => 1(pp) * 5(ori) * 7(leftover) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 6(ori) + 2(EXTRA)
        //
        // 7 + 1 + 2 = 10
        assert_eq!(min_piece_count.calculate(&oe(210)).get(), 10);
    }

    #[test_log::test]
    fn nontrivial_no_extra_cycles() {
        let puzzle = big_puzzle_with_oris(&[24, 72, 2]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b11,
                orbit_orientation_contributions: vec![oe(72)],
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

        let puzzle = big_puzzle_with_oris(&[2, 12]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b11,
                orbit_orientation_contributions: vec![oe(12)],
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
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b11,
                orbit_orientation_contributions: vec![oe(6)],
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
    fn possible_order_is_lcm() {
        let puzzle = big_puzzle_with_oris(&[2, 3]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b11,
                orbit_orientation_contributions: vec![oe(2), oe(3)],
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
        // 1: [0, 0] => 1(pp) * 2(ori) + 2(EXTRA)
        // 2: [0, 0] => 1(pp) * 3(ori) + 2(EXTRA)
        //
        // 4 + 3 = 7
        assert_eq!(min_piece_count.calculate(&oe(6)).get(), 4);

        let puzzle = big_puzzle_with_oris(&[3]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b10,
                orbit_orientation_contributions: vec![oe(3)],
                orientations_exps_lcm: oe(3),
            }
        );
        // [0, 1]
        //
        // orbit 1:
        // [0, 1]
        // [0, 0]
        //
        // 2: [0] => 1(pp) * 3(ori) + 2(EXTRA)
        //
        // 2 = 2
        assert_eq!(min_piece_count.calculate(&oe(3)).get(), 2);
    }

    #[test_log::test]
    fn small_order_exps() {
        let puzzle = big_puzzle_with_oris(&[100, 9]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b111,
                orbit_orientation_contributions: vec![oe(100), oe(9)],
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
        // 1: [0, 0, 0] => 1(pp) * 10(ori) + 2(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 3(ori) + 2(EXTRA)
        //
        // 2 + 2 = 4
        //
        // Note that we subdivide 100 into a +10 orientation cycle; we are always
        // allowed to do this because saturaing the factor is guaranteed to make the
        // number a divisor.
        assert_eq!(min_piece_count.calculate(&oe(30)).get(), 4);
    }
}

#[cfg(test)]
mod suboptimal_edge_cases {
    use crate::min_piece_count::{MinPieceCount, big_puzzle_with_oris, oe};

    #[test_log::test]
    fn case1() {
        let puzzle = big_puzzle_with_oris(&[225, 4]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b111,
                orbit_orientation_contributions: vec![oe(225), oe(4)],
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
        // bad case:
        //
        // 1: [0, 0, 0] => 1(pp) * 15(ori) + 2(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 2(ori) + 2(EXTRA)
        //
        // 2 + 2 = 4
        //
        // good case:
        //
        // 1: [1, 0, 0] => 2(pp) * 30(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 2 + 1 = 3
        assert_eq!(min_piece_count.calculate(&oe(30)).get(), 3);
    }

    #[test_log::test]
    fn case2() {
        let puzzle = big_puzzle_with_oris(&[15, 2]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b111,
                orbit_orientation_contributions: vec![oe(225), oe(4)],
                orientations_exps_lcm: oe(900),
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
        // 1: [1, 0, 0] => 2(pp) * 30(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 2 + 1 = 3
        assert_eq!(min_piece_count.calculate(&oe(30)).get(), 3);
    }

    #[test_log::test]
    fn case3() {
        let puzzle = big_puzzle_with_oris(&[15, 4]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b111,
                orbit_orientation_contributions: vec![oe(225), oe(4)],
                orientations_exps_lcm: oe(900),
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
        // 1: [1, 0, 0] => 2(pp) * 30(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 2 + 1 = 3
        assert_eq!(min_piece_count.calculate(&oe(30)).get(), 3);
    }

    #[test_log::test]
    fn case4() {
        let puzzle = big_puzzle_with_oris(&[30, 4]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b111,
                orbit_orientation_contributions: vec![oe(225), oe(4)],
                orientations_exps_lcm: oe(900),
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
        // 2: [0, 0, 0] => 1(pp) * 8(ori) + 2(EXTRA)
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

    // TODO: test case when ori is even
    #[test_log::test]
    fn case5() {
        let puzzle = big_puzzle_with_oris(&[60, 6, 45]);
        let min_piece_count = MinPieceCount::from(&puzzle);
        assert_eq!(
            min_piece_count,
            MinPieceCount {
                leftover_prime_powers_mask: !0b111,
                orbit_orientation_contributions: vec![oe(20), oe(9)],
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
        // 1: [0, 0, 1] => 5(pp) * 30(ori) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 1(ori)
        // 3: [0, 0, 0] => 1(pp) * 1(ori)
        //
        // 5 + 1 = 6
        assert_eq!(min_piece_count.calculate(&oe(150)).get(), 6);
    }
}

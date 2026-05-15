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
    orientations_exps: Vec<OrderExps<N>>,
    orbit_orientation_contributions: Vec<OrderExps<N>>,
    orientations_exps_lcm: OrderExps<N>,
    has_even_parity_constraint: Vec<bool>,
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
            leftover_prime_powers_mask,
            orientations_exps,
            orbit_orientation_contributions,
            orientations_exps_lcm,
            has_even_parity_constraint,
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
    // piece count factors?
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
            let mut eq = possible_order
                .remove_factors(orientation_exps)
                .0
                .simd_eq(required_cycle_prime_powers.0)
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
        // Thus this fits into a u16.
        let mut needing_orientation_cycles_count = 0u32;
        let mut min_piece_count = leftover_prime_powers_sum;
        let mut transfer_two_extra_cycle = false;
        for orbit_orientation_contribution in &self.orbit_orientation_contributions {
            let mut contributing_prime_powers = orbit_orientation_contribution
                .0
                .simd_ne(Simd::splat(0))
                .to_bitmask();
            let orientation_contribution_is_two = match contributing_prime_powers.count_ones() {
                0 => continue,
                1 if orbit_orientation_contribution.two_exponent() != 0 => true,
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
            transfer_two_extra_cycle |= orientation_contribution_is_two && cycles_count == 0;
            needing_orientation_cycles_count += 2u32.saturating_sub(cycles_count);
        }
        let mut extra_piece_count =
            needing_orientation_cycles_count.saturating_sub(leftover_prime_powers_count);
        if extra_piece_count > 2 && transfer_two_extra_cycle {
            extra_piece_count -= 1;
            // we don't know to which orbit we are tranferring this cycle to;
            // such analysis is too complicated so we just give up.
        } else if let Some(two_orientation_contribution_orbit_index) =
            maybe_two_orientation_contribution_orbit_index
            && self.has_even_parity_constraint[two_orientation_contribution_orbit_index]
        {
            extra_piece_count += 2;
        }
        min_piece_count += extra_piece_count;

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
    use crate::{min_piece_count::big_puzzle_with_oris, puzzle::cubeN::CUBE3};

    #[test_log::test]
    fn cube3() {
        let cube3 = CUBE3.clone();
        // assert_eq!(
        //     MinPieceCount::from(&cube3),
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1), oe(1)],
        //         orientations_exps: vec![oe(3), oe(2)],
        //         leftover_prime_powers_mask: u64::from(!0b11u8),
        //         orientations_exps_lcm: oe(6),
        //     }
        // );
    }

    #[test_log::test]
    fn orbit_orientation_dominates() {
        let puzzle = big_puzzle_with_oris(&[180, 6, 5]);
        // assert_eq!(
        //     MinPieceCount::from(&puzzle),
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b111,
        //         orientations_exps_lcm: oe(180),
        //     }
        // );
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
        // assert_eq!(
        //     MinPieceCount::from(&puzzle),
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1), oe(1), oe(1)],
        //         orientations_exps: vec![oe(60), oe(6), oe(45)],
        //         leftover_prime_powers_mask: !0b111,
        //         orientations_exps_lcm: oe(180),
        //     }
        // );
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

        // assert_eq!(
        //     MinPieceCount::from(&puzzle),
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b111,
        //         orientations_exps_lcm: oe(180),
        //     }
        // );

        let puzzle = big_puzzle_with_oris(&[180, 6, 60]);

        // assert_eq!(
        //     MinPieceCount::from(&puzzle),
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b111,
        //         orientations_exps_lcm: oe(180),
        //     }
        // );
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

        // assert_eq!(
        //     MinPieceCount::from(&puzzle),
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b111,
        //         orientations_exps_lcm: oe(60),
        //     }
        // );

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

        // assert_eq!(
        //     MinPieceCount::from(&puzzle),
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b111,
        //         orientations_exps_lcm: oe(900),
        //     }
        // );
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
        puzzle::cubeN::CUBE3,
    };

    #[test_log::test]
    fn daniels_edge_case() {
        let puzzle = puzzle_with_piece_count_and_oris(&[(8, 1), (17, 2)]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b1,
        //         orientations_exps_lcm: oe(2),
        //     }
        // );
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
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0,
        //         orientations_exps_lcm: oe(1),
        //     }
        // );
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
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b11,
        //         orientations_exps_lcm: oe(18),
        //     }
        // );
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
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b11,
        //         orientations_exps_lcm: oe(6),
        //     }
        // );
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
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b11,
        //         orientations_exps_lcm: oe(12),
        //     }
        // );
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
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b11,
        //         orientations_exps_lcm: oe(6),
        //     }
        // );
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
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b10,
        //         orientations_exps_lcm: oe(3),
        //     }
        // );
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
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b11,
        //         orientations_exps_lcm: oe(6),
        //     }
        // );
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
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b1,
        //         orientations_exps_lcm: oe(8),
        //     }
        // );
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
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b111,
        //         orientations_exps_lcm: oe(30),
        //     }
        // );
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
        // bad:
        //
        // 1: [0, 0, 0] => 1(pp) * 5(ori) * 7(leftover) + 1(EXTRA)
        // 2: [0, 0, 0] => 1(pp) * 6(ori) + 2(EXTRA)
        //
        // 7 + 1 + 2 = 10
        //
        // good:
        //
        // 1: [1, 0, 0] => 2(pp) * 5(ori) * 7(leftover)
        // 2: [0, 0, 0] => 1(pp) * 3(ori) * 2(EXTRA)
        assert_eq!(min_piece_count.calculate(&oe(210)).get(), 10);
    }

    #[test_log::test]
    fn nontrivial_no_extra_cycles() {
        let puzzle = big_puzzle_with_oris(&[24, 72, 2]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b11,
        //         orientations_exps_lcm: oe(72),
        //     }
        // );
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
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b11,
        //         orientations_exps_lcm: oe(12),
        //     }
        // );
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
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b11,
        //         orientations_exps_lcm: oe(6),
        //     }
        // );
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
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b11,
        //         orientations_exps_lcm: oe(6),
        //     }
        // );
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
        // 2: [0, 0] => 1(pp) * 3(ori) + 1(EXTRA)
        //
        // 2 + 1 = 3
        //
        // good:
        //
        // 1: [0, 0] => 1(pp) * 1(ori)
        // 2: [1, 0] => 2(pp) * 3(ori) + 1(EXTRA)
        //
        // 2 + 1 = 3
        assert_eq!(min_piece_count.calculate(&oe(6)).get(), 3);

        let puzzle = big_puzzle_with_oris(&[3]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b10,
        //         orientations_exps_lcm: oe(3),
        //     }
        // );
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
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b111,
        //         orientations_exps_lcm: oe(900),
        //     }
        // );
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

    #[test_log::test]
    fn even_parity_constraint() {
        let cube3 = CUBE3.clone();
        let mut min_piece_count = MinPieceCount::from(&cube3);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b111,
        //         orientations_exps_lcm: oe(900),
        //     }
        // );
        assert_eq!(min_piece_count.calculate(&oe(1260)).get(), 19);
        assert_eq!(min_piece_count.calculate(&oe(990)).get(), 20);
        assert_eq!(min_piece_count.calculate(&oe(495)).get(), 19);
    }

    // TODO: test for not applying even parity constraint
}

#[cfg(test)]
mod suboptimal_edge_cases {
    use crate::min_piece_count::{MinPieceCount, big_puzzle_with_oris, oe};

    #[test_log::test]
    fn case1() {
        let puzzle = big_puzzle_with_oris(&[225, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b111,
        //         orientations_exps_lcm: oe(900),
        //     }
        // );
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
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1), oe(1)],
        //         orientations_exps: vec![oe(15), oe(12)],
        //         leftover_prime_powers_mask: !0b111,
        //         orientations_exps_lcm: oe(30),
        //     }
        // );
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
    }

    #[test_log::test]
    fn case3() {
        let puzzle = big_puzzle_with_oris(&[15, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b111,
        //         orientations_exps_lcm: oe(60),
        //     }
        // );
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
    }

    #[test_log::test]
    fn case4() {
        let puzzle = big_puzzle_with_oris(&[30, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b111,
        //         orientations_exps_lcm: oe(60),
        //     }
        // );
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
    fn case5() {
        let puzzle = big_puzzle_with_oris(&[30, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b111,
        //         orientations_exps_lcm: oe(60),
        //     }
        // );
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
    fn case6() {
        let puzzle = big_puzzle_with_oris(&[210, 4]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1), oe(1)],
        //         orientations_exps: vec![oe(210), oe(4)],
        //         leftover_prime_powers_mask: !0b1111,
        //         orientations_exps_lcm: oe(420),
        //     }
        // );
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
    }

    // TODO: test case when there are other 2 extras, and is_power_of_two is false
    // TODO: combine case7 and case6, case7 and case4
    #[test_log::test]
    fn case7() {
        let puzzle = big_puzzle_with_oris(&[60, 6, 45]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b111,
        //         orientations_exps_lcm: oe(180),
        //     }
        // );
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
    }

    // TODO: test case when there are other 2 extras, and is_power_of_two is false
    // TODO: combine case7 and case6, case7 and case4
    #[test_log::test]
    fn case8() {
        let puzzle = big_puzzle_with_oris(&[90, 6, 20]);
        let mut min_piece_count = MinPieceCount::from(&puzzle);
        // assert_eq!(
        //     min_piece_count,
        //     MinPieceCount {
        //         orbit_orientation_contributions: vec![oe(1)],
        //         orientations_exps: vec![oe(1)],
        //         leftover_prime_powers_mask: !0b111,
        //         orientations_exps_lcm: oe(180),
        //     }
        // );
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
}

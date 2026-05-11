use std::{
    num::NonZeroU16,
    simd::{Simd, cmp::SimdPartialEq, num::SimdUint},
};

use crate::{FIRST_129_PRIMES, orderexps::OrderExps, puzzle::PuzzleDef};

// TODO: only work when orientation sum constraint is Zero
#[derive(Debug)]
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

fn prime_power_cycle_piece_count(prime: u16, exp: u8) -> u16 {
    if exp == 0 {
        0
    } else {
        prime.pow(u32::from(exp))
    }
}

impl<const N: usize> MinPieceCount<N> {
    // TODO: u32
    pub fn smart(&self, possible_order: &OrderExps<N>) -> NonZeroU16 {
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
        let mut min_piece_count = u16::try_from(self.orbit_orientation_contributions.len())
            .unwrap()
            .saturating_sub(leftover_prime_power_count)
            + leftover_prime_powers_sum;
        for orbit_orientation_contribution in &self.orbit_orientation_contributions {
            let mut contributed_primes = orbit_orientation_contribution
                .0
                .simd_ne(Simd::splat(0))
                .to_bitmask();
            while contributed_primes != 0 {
                let prime_power_index = contributed_primes.trailing_zeros() as usize;
                let exp = possible_order.0[prime_power_index]
                    .checked_sub(orbit_orientation_contribution.0[prime_power_index])
                    .unwrap();
                let prime = FIRST_129_PRIMES[prime_power_index];
                min_piece_count += prime_power_cycle_piece_count(prime, exp);

                contributed_primes ^= contributed_primes.isolate_lowest_one();
            }
        }

        debug_assert!(min_piece_count >= self.naive(possible_order).get());
        NonZeroU16::new(min_piece_count).unwrap()
    }

    pub fn naive(&self, possible_order: &OrderExps<N>) -> NonZeroU16 {
        assert_ne!(possible_order, &OrderExps::one());
        NonZeroU16::new(
            possible_order
                .0
                .saturating_sub(self.orientations_exps_lcm.0)
                .as_array()
                .iter()
                .zip(FIRST_129_PRIMES)
                .map(|(&exp, prime)| prime_power_cycle_piece_count(prime, exp))
                .sum::<u16>()
                .max(1),
        )
        .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::simd::Simd;

    use crate::{
        min_piece_count::MinPieceCount,
        orderexps::OrderExps,
        puzzle::{
            EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef,
            PuzzleDef, cubeN::CUBE3,
        },
    };

    #[test]
    fn foo() {
        let cube3 = CUBE3.clone();
        println!("{:?}", MinPieceCount::from(&cube3));
        panic!();
    }

    #[test]
    fn foo2() {
        let puzzle: PuzzleDef<64> = PuzzleDef::new(
            vec![
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 180,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 6,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 5,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
            ],
            EvenParityConstraints(vec![]),
        )
        .unwrap();
        println!("{:?}", MinPieceCount::from(&puzzle));
        panic!();
    }

    #[test]
    fn foo3() {
        // [2, 1, 1]
        // [1, 1, 0]
        // [0, 2, 1]
        let puzzle: PuzzleDef<64> = PuzzleDef::new(
            vec![
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 60,
                        sum_constraint: OrientationSumConstraint::Zero,
                        // orbit_orientation_contributions.0.simd_eq(S)
                    },
                },
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 6,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 45,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
            ],
            EvenParityConstraints(vec![]),
        )
        .unwrap();
        println!("{:?}", MinPieceCount::from(&puzzle));
        panic!();
    }

    #[test]
    fn foo4() {
        // [2, 1, 1]
        // [1, 1, 0]
        // [2, 2, 1]
        let puzzle: PuzzleDef<64> = PuzzleDef::new(
            vec![
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 60,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 6,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 180,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
            ],
            EvenParityConstraints(vec![]),
        )
        .unwrap();
        println!("{:?}", MinPieceCount::from(&puzzle));
        panic!();
    }

    #[test]
    fn foo5() {
        // [2, 1, 1]
        // [1, 1, 0]
        // [2, 2, 1]
        let puzzle: PuzzleDef<64> = PuzzleDef::new(
            vec![
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 6,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 3,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
            ],
            EvenParityConstraints(vec![]),
        )
        .unwrap();
        println!("{:?}", MinPieceCount::from(&puzzle));
        panic!();
    }

    #[test]
    fn foo6() {
        // [2, 1, 1]
        // [1, 1, 0]
        // [2, 2, 1]
        let puzzle: PuzzleDef<64> = PuzzleDef::new(
            vec![
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 18,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 9,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
            ],
            EvenParityConstraints(vec![]),
        )
        .unwrap();
        println!("{:?}", MinPieceCount::from(&puzzle));
        panic!();
    }

    #[test]
    fn foo7() {
        // [2, 1, 1]
        // [1, 1, 0]
        // [2, 2, 1]
        let puzzle: PuzzleDef<64> = PuzzleDef::new(
            vec![
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 18,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 9,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
            ],
            EvenParityConstraints(vec![]),
        )
        .unwrap();
        let a = MinPieceCount::from(&puzzle);
        println!("{:?}", a);
        let mut b = [0; 64];
        b[0] = 3;
        b[1] = 2;
        b[2] = 1;
        b[4] = 2;
        println!("{:?}", a.smart(&OrderExps(Simd::from_array(b))));
        panic!();
    }

    #[test]
    fn foo8() {
        // [2, 1, 1]
        // [1, 1, 0]
        // [2, 2, 1]
        let puzzle: PuzzleDef<64> = PuzzleDef::new(
            vec![
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 5,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 6,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
            ],
            EvenParityConstraints(vec![]),
        )
        .unwrap();
        let a = MinPieceCount::from(&puzzle);
        println!("{:?}", a);
        let mut b = [0; 64];
        b[0] = 1;
        b[1] = 1;
        b[2] = 1;
        b[3] = 1;
        println!("{:?}", a.smart(&OrderExps(Simd::from_array(b))));
        panic!();
    }

    #[test]
    fn foo9() {
        // [2, 1, 1]
        // [1, 1, 0]
        // [2, 2, 1]
        let puzzle: PuzzleDef<64> = PuzzleDef::new(
            vec![
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 2,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
                PartialOrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 3,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                },
            ],
            EvenParityConstraints(vec![]),
        )
        .unwrap();
        let a = MinPieceCount::from(&puzzle);
        println!("{:?}", a);
        let mut b = [0; 64];
        b[0] = 3;
        b[1] = 2;
        println!("{:?}", a.smart(&OrderExps(Simd::from_array(b))));
        panic!();
    }
}

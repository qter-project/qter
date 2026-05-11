use std::{
    num::{NonZeroU16, NonZeroUsize},
    ops::Deref,
    simd::{
        Mask, Select, Simd,
        cmp::SimdPartialEq,
        num::SimdUint,
    },
};

use log::{debug, trace};
use pareto_front::{Dominate, ParetoFront};

use crate::{FIRST_129_PRIMES, orderexps::OrderExps, puzzle::PuzzleDef};

#[derive(Clone, Copy)]
pub enum Optimality {
    Equivalent,
    Optimal,
    // TODO: SubOptimal which uses the naive pareto front dominate approach
}

#[derive(Clone, Copy)]
pub enum RegisterCount {
    Exactly(NonZeroU16),
    All,
}

pub struct Cycle<const N: usize> {
    // partitions: Vec<Vec<u16>>,
}

#[derive(Debug, Clone)]
pub struct PossibleOrder<const N: usize> {
    order: OrderExps<N>,
    min_piece_count: NonZeroU16,
}

struct CycleCombinationCandidate<const N: usize> {
    // first_order_index: usize,
    orders: Vec<PossibleOrder<N>>,
    details: Option<CycleCombinationDetails<N>>,
}

pub struct CycleCombinationDetails<const N: usize> {
    cycles: Vec<Cycle<N>>,
}

pub struct CycleCombination<const N: usize> {
    orders: Vec<PossibleOrder<N>>,
    details: CycleCombinationDetails<N>,
}

pub struct CycleCombinationFinder<const N: usize> {
    puzzle_def: PuzzleDef<N>,
    orientations_exps: Vec<OrderExps<N>>,
    orientation_exps_lcm: OrderExps<N>,
}

#[derive(Clone, Copy)]
pub struct CycleCombinationFinderConfig {
    pub optimality: Optimality,
    pub register_count: RegisterCount,
}

impl<const N: usize> CycleCombination<N> {
    pub fn orders(&self) -> impl Iterator<Item = &OrderExps<N>> {
        self.orders.iter().map(|PossibleOrder { order, .. }| order)
    }
}

impl<const N: usize> Deref for CycleCombination<N> {
    type Target = CycleCombinationDetails<N>;

    fn deref(&self) -> &Self::Target {
        &self.details
    }
}

impl<const N: usize> CycleCombinationDetails<N> {
    #[must_use]
    pub fn cycles(&self) -> &[Cycle<N>] {
        &self.cycles
    }
}

impl<const N: usize> Dominate for CycleCombinationCandidate<N> {
    fn dominate(&self, other: &Self) -> bool {
        // Note that we should never have a case when `self == other` because
        // `cycle_combinations` visits a different order every time, hence we do not
        // have to implement this check as suggested by the `pareto_front` crate.
        // TODO: multi_dominate
        debug_assert!(
            self.orders
                .iter()
                .zip(&other.orders)
                .any(|(s, o)| s.order != o.order)
        );
        self.orders
            .iter()
            .zip(&other.orders)
            .all(|(s, o)| s.order >= o.order)
    }
}

impl<const N: usize> TryFrom<&[PossibleOrder<N>]> for CycleCombinationDetails<N> {
    type Error = ();

    fn try_from(_precheck: &[PossibleOrder<N>]) -> Result<Self, ()> {
        Ok(CycleCombinationDetails { cycles: vec![] })
    }
}

impl<const N: usize> From<PuzzleDef<N>> for CycleCombinationFinder<N> {
    fn from(puzzle_def: PuzzleDef<N>) -> Self {
        let orientations_exps = puzzle_def
            .orbit_defs()
            .iter()
            .map(|orbit_def| {
                OrderExps::<N>::try_from(NonZeroU16::from(orbit_def.orientation_count())).unwrap()
            })
            .collect::<Vec<_>>();
        // `puzzle_def` must have at least one `OrbitDef`
        let orientation_exps_lcm = OrderExps::lcms(orientations_exps.iter().cloned()).unwrap();
        Self {
            puzzle_def,
            orientations_exps,
            orientation_exps_lcm,
        }
    }
}

fn cycle_combinations_helper<const N: usize>(
    possible_orders_except_one: &[PossibleOrder<N>],
    remaining_register_count: NonZeroUsize,
    remaining_piece_count: NonZeroU16,
    max_last_register: &mut OrderExps<N>,
    registers: &mut [PossibleOrder<N>],
    cycle_combination_candidates: &mut ParetoFront<CycleCombinationCandidate<N>>,
    iter_count: &mut u64,
) {
    let register_index = registers.len() - remaining_register_count.get();
    let mut curr_possible_orders = possible_orders_except_one;
    while let Some((possible_order, next_possible_orders)) = curr_possible_orders.split_first() {
        // TODO: compare with index rather than le; faster?
        if register_index == 0 && possible_order.order <= *max_last_register {
            break;
        }

        let Some(next_remaining_piece_count) = remaining_piece_count
            .get()
            .checked_sub(possible_order.min_piece_count.get())
        else {
            curr_possible_orders = next_possible_orders;
            continue;
        };

        if let Some(next_remaining_register_count) =
            NonZeroUsize::new(remaining_register_count.get() - 1)
        {
            if let Some(next_remaining_piece_count) = NonZeroU16::new(next_remaining_piece_count) {
                let old = std::mem::replace(&mut registers[register_index], possible_order.clone());
                cycle_combinations_helper(
                    curr_possible_orders,
                    next_remaining_register_count,
                    next_remaining_piece_count,
                    max_last_register,
                    registers,
                    cycle_combination_candidates,
                    iter_count,
                );
                registers[register_index] = old;
            }
        } else {
            let old = std::mem::replace(&mut registers[register_index], possible_order.clone());
            *iter_count += 1;
            let mut candidate = CycleCombinationCandidate {
                orders: registers.to_vec(),
                details: None,
            };
            // TODO: only use one pass
            // TODO: _remove_dominated_starting_at should reuse a vec
            if !cycle_combination_candidates.dominate(&candidate)
                && let Ok(details) = CycleCombinationDetails::try_from(&*candidate.orders)
            {
                candidate.details = Some(details);
                assert!(cycle_combination_candidates.push(candidate));
                *max_last_register = max_last_register
                    .clone()
                    .max(registers.last().unwrap().order.clone());
                break;
            }
            registers[register_index] = old;
        }
        curr_possible_orders = next_possible_orders;
    }
}

fn prime_power_cycle_piece_count(prime: u16, exp: u8) -> u16 {
    if exp == 0 {
        0
    } else {
        prime.pow(u32::from(exp))
    }
}

impl<const N: usize> CycleCombinationFinder<N> {
    fn min_piece_count(&self, possible_order: &OrderExps<N>) -> NonZeroU16 {
        assert_ne!(possible_order, &OrderExps::one());
        NonZeroU16::new(
            possible_order
                .0
                .saturating_sub(self.orientation_exps_lcm.0)
                .as_array()
                .iter()
                .zip(FIRST_129_PRIMES)
                .map(|(&exp, prime)| prime_power_cycle_piece_count(prime, exp))
                .sum::<u16>()
                .max(1),
        )
        .unwrap()
    }

    fn _min_piece_count(&self, possible_order: &OrderExps<N>) -> NonZeroUsize {
        assert_ne!(possible_order, &OrderExps::one());

        // let leftover_primes = self
        //     .orientations_exps
        //     .iter()
        //     .fold(Mask::splat(true), |acc, orientation_exps| {
        //         acc & orientation_exps.0.simd_eq(acc)
        //     });
        let mut leftover_prime_powers_mask = Mask::splat(true);
        // self.orientation_exps_lcm
        // let mut max_orienation_exps = Simd::splat(0);
        for [a, b] in self.orientations_exps.array_windows() {
            leftover_prime_powers_mask &= a.0.simd_eq(b.0);
            // max_orienation_exps = max_orienation_exps.simd_max(other)
        }

        let mut leftover_prime_powers_sum = 0;
        let mut leftover_prime_power_count = 0;
        for leftover_prime_power in leftover_prime_powers_mask
            .select(possible_order.0, Simd::splat(0))
            .as_array()
            .iter()
            .zip(FIRST_129_PRIMES)
            .map(|(&exp, prime)| prime_power_cycle_piece_count(prime, exp))
        {
            leftover_prime_powers_sum += leftover_prime_power;
            leftover_prime_power_count += 1;
        }

        // leftover_primes.to_array()

        todo!();
    }

    fn find_optimal(&self, register_count: RegisterCount) -> Vec<CycleCombination<N>> {
        let RegisterCount::Exactly(total_register_count) = register_count else {
            panic!("expected exactly variant for now");
        };

        let total_piece_count = NonZeroU16::new(
            self.puzzle_def
                .orbit_defs()
                .iter()
                .map(|&orbit_def| orbit_def.piece_count.get())
                .sum::<u16>(),
        )
        .unwrap();

        let possible_orders_except_one = self.puzzle_def.possible_orders();
        possible_orders_except_one.remove(&OrderExps::one());

        let mut possible_orders_except_one = possible_orders_except_one
            .into_iter()
            .map(|possible_order| {
                let min_piece_count = self.min_piece_count(&possible_order);
                PossibleOrder {
                    order: possible_order,
                    min_piece_count,
                }
            })
            .collect::<Vec<_>>();
        possible_orders_except_one.sort_unstable_by(|a, b| a.order.cmp(&b.order).reverse());
        trace!(
            "{}",
            possible_orders_except_one
                .iter()
                .map(|a| format!("({:?}, {})", a.order, a.min_piece_count))
                .collect::<Vec<_>>()
                .join("\n")
        );

        let mut registers = vec![
            PossibleOrder {
                order: OrderExps::one(),
                min_piece_count: 1.try_into().unwrap(),
            };
            usize::from(total_register_count.get())
        ];
        let mut cycle_combination_candidates = ParetoFront::new();
        let mut max_last_register = OrderExps::one();
        let mut iter_count = 0;
        cycle_combinations_helper(
            &possible_orders_except_one,
            NonZeroUsize::from(total_register_count),
            total_piece_count,
            &mut max_last_register,
            &mut registers,
            &mut cycle_combination_candidates,
            &mut iter_count,
        );
        drop(possible_orders_except_one);
        debug!("Cycle combinations in {iter_count} iterations");
        cycle_combination_candidates
            .into_iter()
            .map(|candidate| CycleCombination {
                orders: candidate.orders,
                // details: candidate.details.unwrap(),
                details: CycleCombinationDetails { cycles: vec![] },
            })
            .collect()
    }

    #[must_use]
    pub fn find(&self, config: CycleCombinationFinderConfig) -> Vec<CycleCombination<N>> {
        match config.optimality {
            Optimality::Equivalent => unimplemented!(),
            Optimality::Optimal => self.find_optimal(config.register_count),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU16, time::Instant};

    use humanize_duration::{Truncate, prelude::DurationExt};
    use log::info;

    use crate::finder::{
        CycleCombination, CycleCombinationFinder, CycleCombinationFinderConfig, Optimality,
        RegisterCount,
    };

    pub fn cycles<const N: usize>(cycle_combinations: Vec<CycleCombination<N>>) -> Vec<Vec<u32>> {
        cycle_combinations
            .into_iter()
            .map(|cycle_combination| {
                cycle_combination
                    .orders()
                    .map(|order| order.as_bigint().try_into().unwrap())
                    .collect::<Vec<u32>>()
            })
            .collect::<Vec<_>>()
    }

    #[test_log::test]
    fn control() {
        let minx3 = crate::puzzle::minxN::MINX3.clone();
        // let cube3 = crate::puzzle::cubeN::CUBE3.clone();
        let now = Instant::now();
        let ccf = CycleCombinationFinder::from(minx3);
        let cycle_combinations = ccf.find(CycleCombinationFinderConfig {
            optimality: Optimality::Optimal,
            register_count: RegisterCount::Exactly(NonZeroU16::new(4).unwrap()),
        });
        info!("CCF in {}", now.elapsed().human(Truncate::Micro));
        info!("Solutions length: {}", cycle_combinations.len());

        assert_eq!(
            cycles(cycle_combinations),
            vec![
                vec![1260, 2],
                vec![840, 3],
                vec![720, 4],
                vec![630, 9],
                vec![420, 12],
                vec![360, 36],
                vec![180, 72],
                vec![90, 90],
            ],
        );
    }
}

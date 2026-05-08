use std::{
    num::{NonZeroU16, NonZeroUsize},
    simd::num::SimdUint,
};

use log::{debug, info, trace};
use pareto_front::{Dominate, ParetoFront};
use puzzle_theory::numbers::{Int, U};

use crate::{FIRST_129_PRIMES, orderexps::OrderExps, puzzle::PuzzleDef};

#[derive(Clone, Copy)]
pub enum Optimality {
    Equivalent,
    Optimal,
}

#[derive(Clone, Copy)]
pub enum RegisterCount {
    Exactly(NonZeroU16),
    All,
}

#[derive(Debug)]
struct Partition(Vec<u16>);

pub struct Cycle {
    partitions: Vec<Vec<u16>>,
}

#[derive(Debug)]
pub struct Cycle2 {
    order: Int<U>,
    partitions: Vec<Partition>,
}

#[derive(Debug, Clone)]
pub struct PossibleOrder<const N: usize> {
    order: OrderExps<N>,
    min_piece_count: NonZeroUsize,
}

struct CycleCombinationPrecheck<const N: usize> {
    orders: Vec<PossibleOrder<N>>,
    combination: Option<CycleCombination>,
}

struct CycleCombination {
    cycles: Vec<Cycle>,
}

#[derive(Debug)]
pub struct CycleCombination2 {
    order_product: Int<U>,
    cycles: Vec<Cycle2>,
    shared_pieces: Vec<u16>,
}

pub struct CycleCombinationFinder<const N: usize> {
    puzzle_def: PuzzleDef<N>,
    orientation_contribution: OrderExps<N>,
}

impl Cycle2 {
    #[must_use]
    pub fn order(&self) -> Int<U> {
        self.order
    }
}

impl CycleCombination2 {
    #[must_use]
    pub fn cycles(&self) -> &[Cycle2] {
        &self.cycles
    }
}

impl<const N: usize> Dominate for CycleCombinationPrecheck<N> {
    fn dominate(&self, other: &Self) -> bool {
        // Note that we should never have a case when `self == other` because
        // `cycle_combinations` visits a different order every time, hence we do not
        // have to implement this check as suggested by the `pareto_front` crate.
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

impl<const N: usize> TryFrom<&[PossibleOrder<N>]> for CycleCombination {
    type Error = ();

    fn try_from(precheck: &[PossibleOrder<N>]) -> Result<Self, ()> {
        todo!()
    }
}

impl<const N: usize> From<PuzzleDef<N>> for CycleCombinationFinder<N> {
    fn from(puzzle_def: PuzzleDef<N>) -> Self {
        let orientation_contribution =
            OrderExps::lcms(puzzle_def.orbit_defs().iter().map(|orbit_def| {
                OrderExps::<N>::try_from(NonZeroU16::from(orbit_def.orientation_count())).unwrap()
            }))
            .unwrap();
        Self {
            puzzle_def,
            orientation_contribution,
        }
    }
}

fn cycle_combinations<const N: usize>(
    possible_orders_except_one: Vec<PossibleOrder<N>>,
    total_register_count: NonZeroU16,
    total_piece_count: NonZeroUsize,
) -> Vec<CycleCombinationPrecheck<N>> {
    let mut registers = vec![
        PossibleOrder {
            order: OrderExps::one(),
            min_piece_count: 1.try_into().unwrap(),
        };
        usize::from(total_register_count.get())
    ];
    let mut out = ParetoFront::new();
    // Note that this cannot be a possible order
    let mut max_last_register = OrderExps::one();
    let mut iter_count = 0;
    cycle_combinations_helper(
        &possible_orders_except_one,
        NonZeroUsize::from(total_register_count),
        total_piece_count,
        &mut max_last_register,
        &mut registers,
        &mut out,
        &mut iter_count,
    );
    drop(possible_orders_except_one);
    debug!("Cycle combinations in {iter_count} iterations");
    out.into()
}

fn cycle_combinations_helper<const N: usize>(
    possible_orders_except_one: &[PossibleOrder<N>],
    remaining_register_count: NonZeroUsize,
    remaining_piece_count: NonZeroUsize,
    max_last_register: &mut OrderExps<N>,
    registers: &mut [PossibleOrder<N>],
    out: &mut ParetoFront<CycleCombinationPrecheck<N>>,
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
            if let Some(next_remaining_piece_count) = NonZeroUsize::new(next_remaining_piece_count)
            {
                let old = std::mem::replace(&mut registers[register_index], possible_order.clone());
                cycle_combinations_helper(
                    curr_possible_orders,
                    next_remaining_register_count,
                    next_remaining_piece_count,
                    max_last_register,
                    registers,
                    out,
                    iter_count,
                );
                registers[register_index] = old;
            }
        } else {
            let old = std::mem::replace(&mut registers[register_index], possible_order.clone());
            *iter_count += 1;
            let mut precheck = CycleCombinationPrecheck {
                orders: registers.to_vec(),
                combination: None,
            };
            // TODO: only use one pass
            if !out.dominate(&precheck)
                && let Ok(cycle_combination) = CycleCombination::try_from(&*precheck.orders)
            {
                precheck.combination = Some(cycle_combination);
                assert!(out.push(precheck));
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
    fn min_piece_count_naive(&self, possible_order: &OrderExps<N>) -> NonZeroUsize {
        NonZeroUsize::new(
            possible_order
                .0
                .saturating_sub(self.orientation_contribution.0)
                .as_array()
                .iter()
                .zip(FIRST_129_PRIMES)
                .map(|(&exp, prime)| usize::from(prime_power_cycle_piece_count(prime, exp)))
                .sum::<usize>()
                .max(1),
        )
        .unwrap()
    }

    fn min_piece_count(&self, possible_order: &OrderExps<N>) -> NonZeroUsize {
        assert_ne!(possible_order, &OrderExps::one());
        todo!()
    }

    fn find_optimal(&self, register_count: RegisterCount) -> Vec<CycleCombination2> {
        let RegisterCount::Exactly(total_register_count) = register_count else {
            panic!("expected exactly variant for now");
        };

        let total_piece_count = NonZeroUsize::from(
            NonZeroU16::new(
                self.puzzle_def
                    .orbit_defs()
                    .iter()
                    .map(|&orbit_def| orbit_def.piece_count.get())
                    .sum::<u16>(),
            )
            .unwrap(),
        );

        let possible_orders_except_one = self.puzzle_def.possible_orders();
        possible_orders_except_one.remove(&OrderExps::one());

        let mut possible_orders_except_one = possible_orders_except_one
            .into_iter()
            .map(|possible_order| {
                let min_piece_count = self.min_piece_count_naive(&possible_order);
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

        let cycle_combination_candidates = cycle_combinations(
            possible_orders_except_one,
            total_register_count,
            total_piece_count,
        );
        let len = cycle_combination_candidates.len();
        // .combination
        // .into_inner()
        // .flatten()
        // .unwrap()
        // .cycles
        // .into_iter()
        // .map(|j| j.partitions)
        // .collect::<Vec<_>>())
        info!(
            "{:?}",
            cycle_combination_candidates
                .into_iter()
                .map(|i| i.orders.into_iter().map(|j| j.order).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        );
        info!("Len {len:?}");

        todo!()
    }

    #[must_use]
    pub fn find(
        &self,
        optimality: Optimality,
        register_count: RegisterCount,
    ) -> Vec<CycleCombination2> {
        match optimality {
            Optimality::Equivalent => unimplemented!(),
            Optimality::Optimal => self.find_optimal(register_count),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU16;

    use crate::{
        finder::{CycleCombination2, CycleCombinationFinder, Optimality, RegisterCount},
        puzzle::cubeN::CUBE3,
    };

    pub fn cycles(cycle_combinations: Vec<CycleCombination2>) -> Vec<Vec<u32>> {
        cycle_combinations
            .into_iter()
            .map(|cycle_combination| {
                cycle_combination
                    .cycles()
                    .iter()
                    .map(|cycle| cycle.order().try_into().unwrap())
                    .collect::<Vec<u32>>()
            })
            .collect::<Vec<_>>()
    }

    #[test_log::test]
    fn optimal_2() {
        // let cube3 = MINX4.clone();
        let cube3 = CUBE3.clone();
        let ccf = CycleCombinationFinder::from(cube3);
        let cycle_combinations = ccf.find(
            Optimality::Optimal,
            RegisterCount::Exactly(NonZeroU16::new(2).unwrap()),
        );
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

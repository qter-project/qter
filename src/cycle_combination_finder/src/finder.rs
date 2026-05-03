use std::{
    num::{NonZeroU16, NonZeroUsize},
    simd::num::SimdUint,
};

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

#[derive(Debug)]
pub struct Cycle {
    order: Int<U>,
    partitions: Vec<Partition>,
}

#[derive(Debug, Clone)]
pub struct PossibleOrder<const N: usize> {
    order: OrderExps<N>,
    min_piece_count: NonZeroUsize,
}

pub struct CycleCombinationCandidate<const N: usize> {
    registers: Vec<PossibleOrder<N>>,
}

#[derive(Debug)]
pub struct CycleCombination {
    order_product: Int<U>,
    cycles: Vec<Cycle>,
    shared_pieces: Vec<u16>,
}

pub struct CycleCombinationFinder<const N: usize> {
    puzzle_def: PuzzleDef<N>,
}

impl Cycle {
    #[must_use]
    pub fn order(&self) -> Int<U> {
        self.order
    }
}

impl CycleCombination {
    #[must_use]
    pub fn cycles(&self) -> &[Cycle] {
        &self.cycles
    }
}

impl<const N: usize> Dominate for CycleCombinationCandidate<N> {
    fn dominate(&self, other: &Self) -> bool {
        self.registers
            .iter()
            .zip(&other.registers)
            .all(|(s, o)| o.order <= s.order)
    }
}

impl<const N: usize> From<PuzzleDef<N>> for CycleCombinationFinder<N> {
    fn from(puzzle_def: PuzzleDef<N>) -> Self {
        Self { puzzle_def }
    }
}

fn min_piece_count<const N: usize>(
    possible_order: &OrderExps<N>,
    orientation_contribution: &OrderExps<N>,
) -> NonZeroUsize {
    assert_ne!(possible_order, &OrderExps::one());
    NonZeroUsize::new(
        possible_order
            .0
            .saturating_sub(orientation_contribution.0)
            .as_array()
            .iter()
            .zip(FIRST_129_PRIMES)
            .map(|(&exp, prime)| {
                if exp == 0 {
                    0
                } else {
                    usize::from(prime.pow(u32::from(exp)))
                }
            })
            .sum::<usize>()
            .max(1),
    )
    .unwrap()
}

fn cycle_combination_candidates<const N: usize>(
    possible_orders: Vec<PossibleOrder<N>>,
    register_count: NonZeroU16,
    piece_count_sum: NonZeroUsize,
) -> ParetoFront<CycleCombinationCandidate<N>> {
    let mut registers = vec![
        PossibleOrder {
            order: OrderExps::one(),
            min_piece_count: 1.try_into().unwrap(),
        };
        usize::from(register_count.get())
    ];
    let mut out = ParetoFront::new();
    // Note that this cannot be a possible order
    let mut max_last_register = OrderExps::one();
    cycle_combination_candidates_helper(
        &possible_orders,
        NonZeroUsize::from(register_count),
        piece_count_sum,
        &mut max_last_register,
        &mut registers,
        &mut out,
    );
    drop(possible_orders);
    out
}

fn cycle_combination_candidates_helper<const N: usize>(
    possible_orders: &[PossibleOrder<N>],
    remaining_register_count: NonZeroUsize,
    remaining_piece_count: NonZeroUsize,
    max_last_register: &mut OrderExps<N>,
    registers: &mut [PossibleOrder<N>],
    out: &mut ParetoFront<CycleCombinationCandidate<N>>,
) {
    let register_index = registers.len() - remaining_register_count.get();
    let mut rest = possible_orders;
    while let Some((first, tail)) = rest.split_first() {
        if register_index == 0 && first.order == *max_last_register {
            break;
        }
        rest = tail;

        let Some(next_remaining_piece_count) = remaining_piece_count
            .get()
            .checked_sub(first.min_piece_count.get())
        else {
            continue;
        };

        if let Some(next_remaining_register_count) =
            NonZeroUsize::new(remaining_register_count.get() - 1)
        {
            if let Some(next_remaining_piece_count) = NonZeroUsize::new(next_remaining_piece_count)
            {
                let old = std::mem::replace(&mut registers[register_index], first.clone());
                cycle_combination_candidates_helper(
                    rest,
                    next_remaining_register_count,
                    next_remaining_piece_count,
                    max_last_register,
                    registers,
                    out,
                );
                registers[register_index] = old;
            }
        } else {
            let old = std::mem::replace(&mut registers[register_index], first.clone());
            if out.push(CycleCombinationCandidate {
                registers: registers.to_vec(),
            }) {
                *max_last_register = max_last_register
                    .clone()
                    .max(registers.last().unwrap().order.clone());
                break;
            }
            registers[register_index] = old;
        }
    }
}

impl<const N: usize> CycleCombinationFinder<N> {
    fn find_optimal(&self, register_count: RegisterCount) -> Vec<CycleCombination> {
        let RegisterCount::Exactly(register_count) = register_count else {
            panic!("expected exactly variant for now");
        };

        let piece_count_sum = NonZeroUsize::new(usize::from(
            self.puzzle_def
                .orbit_defs()
                .iter()
                .map(|&orbit_def| orbit_def.piece_count.get())
                .sum::<u16>(),
        ))
        .unwrap();

        let possible_orders = self.puzzle_def.possible_orders();
        possible_orders.remove(&OrderExps::one());

        let orientation_contribution =
            self.puzzle_def
                .orbit_defs()
                .iter()
                .fold(OrderExps::one(), |acc, orbit_def| {
                    acc.lcm(&OrderExps::<N>::try_from(orbit_def.orientation_count2()).unwrap())
                });
        let mut possible_orders = possible_orders
            .into_iter()
            .map(|possible_order| {
                let min_piece_count = min_piece_count(&possible_order, &orientation_contribution);
                PossibleOrder {
                    order: possible_order,
                    min_piece_count,
                }
            })
            .collect::<Vec<_>>();
        possible_orders.sort_unstable_by(|a, b| a.order.cmp(&b.order).reverse());

        let cycle_combination_candidates =
            cycle_combination_candidates(possible_orders, register_count, piece_count_sum);
        println!(
            "{:?}",
            cycle_combination_candidates
                .into_iter()
                .map(|i| i.registers.into_iter().map(|j| j.order).collect::<Vec<_>>())
                .collect::<Vec<_>>() // cycle_combination_candidates.len()
        );
        todo!()
    }

    #[must_use]
    pub fn find(
        &self,
        optimality: Optimality,
        register_count: RegisterCount,
    ) -> Vec<CycleCombination> {
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
        finder::{CycleCombination, CycleCombinationFinder, Optimality, RegisterCount},
        puzzle::cubeN::CUBE3,
    };

    pub fn cycles(cycle_combinations: Vec<CycleCombination>) -> Vec<Vec<u32>> {
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

use std::{
    num::{NonZeroU16, NonZeroUsize},
    simd::{Simd, num::SimdUint},
};

use pareto_front::{Dominate, ParetoFront};
use puzzle_theory::numbers::{Int, U};

use crate::{
    FIRST_129_PRIMES, number_theory::max_prime_powers_below, orderexps::OrderExps,
    puzzle::PuzzleDef,
};

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
    min_piece_count: usize,
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
        for (s, o) in self.registers.iter().zip(&other.registers) {
            if o.order > s.order {
                return false;
            }
        }
        true
    }
}

impl<const N: usize> From<PuzzleDef<N>> for CycleCombinationFinder<N> {
    fn from(puzzle_def: PuzzleDef<N>) -> Self {
        Self { puzzle_def }
    }
}

fn cycle_combination_candidates<const N: usize>(
    possible_orders: Vec<PossibleOrder<N>>,
    register_count: NonZeroU16,
    piece_count_sum: NonZeroUsize,
) -> ParetoFront<CycleCombinationCandidate<N>> {
    let mut path = vec![
        PossibleOrder {
            order: OrderExps::one(),
            min_piece_count: 1.try_into().unwrap(),
        };
        usize::from(register_count.get())
    ];
    let mut out = ParetoFront::new();
    cycle_combination_candidates_helper(
        &possible_orders,
        NonZeroUsize::from(register_count),
        piece_count_sum,
        &mut path,
        // false,
        &mut out,
    );
    drop(possible_orders);
    out
}

fn cycle_combination_candidates_helper<const N: usize>(
    possible_orders: &[PossibleOrder<N>],
    remaining_register_count: NonZeroUsize,
    remaining_piece_count: NonZeroUsize,
    registers: &mut [PossibleOrder<N>],
    out: &mut ParetoFront<CycleCombinationCandidate<N>>,
) {
    let register_count = registers.len();
    let register_index = register_count - remaining_register_count.get();
    let mut rest = possible_orders;
    while let Some((first, tail)) = rest.split_first() {
        rest = tail;

        let Some(next_remaining_piece_count) = remaining_piece_count
            .get()
            .checked_sub(first.min_piece_count)
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
                    registers,
                    out,
                );
                registers[register_index] = old;
            }
        } else {
            let old = std::mem::replace(&mut registers[register_index], first.clone());
            out.push(CycleCombinationCandidate {
                registers: registers.to_vec(),
            });
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
                .fold(0, |acc, &orbit_def| acc + orbit_def.piece_count.get()),
        ))
        .unwrap();

        let partition_max = self
            .puzzle_def
            .orbit_defs()
            .iter()
            .map(|orbit_def| orbit_def.piece_count)
            .max()
            // PuzzleDef enforces that there is at least one orbit
            .unwrap();

        let max_prime_powers =
            max_prime_powers_below(self.puzzle_def.orbit_defs(), partition_max.get());

        let possible_orders = self.puzzle_def.possible_orders();
        possible_orders.remove(&OrderExps::one());

        let mut orienting_orbits = OrderExps::one();
        for (exp, prime) in orienting_orbits
            .0
            .as_mut_array()
            .iter_mut()
            .zip(FIRST_129_PRIMES)
        {
            *exp = if self
                .puzzle_def
                .orbit_defs()
                .iter()
                .any(|orbit_def| u16::from(orbit_def.orientation_count()) == prime)
            {
                1
            } else {
                0
            }
        }
        let mut possible_orders = possible_orders
            .into_iter()
            .map(|possible_order| {
                // TODO: incorporate very basic parity and orientation requirements to make this
                // heuristic better
                let min_piece_count = possible_order.remove_factors(&orienting_orbits);
                let min_piece_count = min_piece_count.0.cast::<u16>()
                    * Simd::from_array(FIRST_129_PRIMES.first_chunk().unwrap().to_owned());
                let min_piece_count = min_piece_count.reduce_sum();
                PossibleOrder {
                    order: possible_order,
                    min_piece_count: usize::from(min_piece_count),
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
                .collect::<Vec<_>>()
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

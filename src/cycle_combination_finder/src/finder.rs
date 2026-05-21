use std::{
    num::{NonZeroU16, NonZeroU32, NonZeroUsize},
    ops::Deref,
    time::Instant,
};

use bumpalo::Bump;
use humanize_duration::{Truncate, prelude::DurationExt};
use log::{debug, trace};

use crate::{
    min_piece_count::MinPieceCount,
    orderexps::OrderExps,
    pareto_front::{CycleCombinationDominate, CycleCombinationParetoFront},
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
pub struct Cycle<const N: usize> {
    // partitions: Vec<Vec<u16>>,
}

#[derive(Debug, Clone)]
pub struct PossibleOrder<const N: usize> {
    order: OrderExps<N>,
    min_piece_count: NonZeroU32,
}

#[derive(Debug)]
struct ArenaCycleCombination<'a, const N: usize> {
    // first_order_index: usize,
    orders: Box<[PossibleOrder<N>], &'a Bump>,
    details: CycleCombinationDetails<N>,
}

#[derive(Debug)]
pub struct CycleCombinationDetails<const N: usize> {
    cycles: Vec<Cycle<N>>,
}

pub struct CycleCombination<const N: usize> {
    orders: Box<[PossibleOrder<N>]>,
    details: CycleCombinationDetails<N>,
}

pub struct CycleCombinationFinder<const N: usize> {
    puzzle_def: PuzzleDef<N>,
}

#[derive(Clone, Copy)]
pub struct CycleCombinationFinderConfig {
    pub optimality: Optimality,
    pub register_count: RegisterCount,
}

impl<const N: usize> PossibleOrder<N> {
    fn initialized() -> Self {
        PossibleOrder {
            order: OrderExps::one(),
            min_piece_count: 1.try_into().unwrap(),
        }
    }
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

impl<const N: usize> CycleCombinationDominate<N> for ArenaCycleCombination<'_, N> {
    fn dominate(&self, registers_except_first: &[PossibleOrder<N>]) -> bool {
        // Note that we should never have a case when `self == other` because
        // `cycle_combinations` visits a different order every time, hence we do not
        // have to implement this check as suggested by the `pareto_front` crate.
        debug_assert!(
            self.orders
                .iter()
                .skip(1)
                .zip(registers_except_first)
                .all(|(s, o)| s.order != o.order)
        );
        self.orders
            .iter()
            .skip(1)
            .zip(registers_except_first)
            .all(|(s, o)| s.order >= o.order)
    }
}

impl<const N: usize> From<PuzzleDef<N>> for CycleCombinationFinder<N> {
    fn from(puzzle_def: PuzzleDef<N>) -> Self {
        Self { puzzle_def }
    }
}

impl<const N: usize> TryFrom<&[PossibleOrder<N>]> for CycleCombinationDetails<N> {
    type Error = ();

    fn try_from(_precheck: &[PossibleOrder<N>]) -> Result<Self, ()> {
        Ok(CycleCombinationDetails { cycles: vec![] })
    }
}

unsafe fn cycle_combinations_helper<'a, const N: usize>(
    possible_orders_except_one: &[PossibleOrder<N>],
    remaining_register_count: NonZeroUsize,
    remaining_piece_count: NonZeroU32,
    max_last_register: &mut OrderExps<N>,
    registers: &mut [PossibleOrder<N>],
    cycle_combinations: &mut CycleCombinationParetoFront<N, ArenaCycleCombination<'a, N>>,
    iter_count: &mut u64,
    bump: &'a Bump,
) {
    let register_index = registers.len() - remaining_register_count.get();
    let mut curr_possible_orders = possible_orders_except_one;
    while let Some((possible_order, next_possible_orders)) = curr_possible_orders.split_first() {
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
            if let Some(next_remaining_piece_count) = NonZeroU32::new(next_remaining_piece_count) {
                let old = std::mem::replace(
                    unsafe { registers.get_unchecked_mut(register_index) },
                    possible_order.clone(),
                );
                unsafe {
                    cycle_combinations_helper(
                        curr_possible_orders,
                        next_remaining_register_count,
                        next_remaining_piece_count,
                        max_last_register,
                        registers,
                        cycle_combinations,
                        iter_count,
                        bump,
                    );
                }
                *unsafe { registers.get_unchecked_mut(register_index) } = old;
            }
        } else {
            // SAFETY: `register_index`
            let old = std::mem::replace(
                unsafe { registers.get_unchecked_mut(register_index) },
                possible_order.clone(),
            );
            *iter_count += 1;
            let registers_except_first = unsafe { registers.split_first().unwrap_unchecked().1 };
            if cycle_combinations.push_and_dominating_check(
                registers_except_first,
                |dominating_registers| {
                    Some(ArenaCycleCombination {
                        orders: Box::clone_from_ref_in(registers, bump),
                        details: CycleCombinationDetails::try_from(dominating_registers).ok()?,
                    })
                },
            ) {
                *max_last_register = max_last_register
                    .clone()
                    .max(registers.last().unwrap().order.clone());
                break;
            }
            *unsafe { registers.get_unchecked_mut(register_index) } = old;
        }
        curr_possible_orders = next_possible_orders;
    }
}

impl<const N: usize> CycleCombinationFinder<N> {
    fn find_optimal(&self, register_count: RegisterCount) -> Vec<CycleCombination<N>> {
        let RegisterCount::Exactly(total_register_count) = register_count else {
            panic!("expected exactly variant for now");
        };

        let total_piece_count = NonZeroU32::new(
            self.puzzle_def
                .orbit_defs()
                .iter()
                .map(|&orbit_def| u32::from(orbit_def.piece_count.get()))
                .sum::<u32>(),
        )
        .unwrap();

        let possible_orders_except_one = self.puzzle_def.possible_orders();
        possible_orders_except_one.remove(&OrderExps::one());

        let now = Instant::now();
        let mut min_piece_count_calculator = MinPieceCount::from(&self.puzzle_def);
        let mut possible_orders_except_one = possible_orders_except_one
            .into_iter()
            .map(|possible_order| {
                let min_piece_count = min_piece_count_calculator.calculate(&possible_order).0;
                PossibleOrder {
                    order: possible_order,
                    min_piece_count,
                }
            })
            .collect::<Vec<_>>();
        debug!(
            "All min piece counts in {}",
            now.elapsed().human(Truncate::Micro)
        );
        possible_orders_except_one.sort_unstable_by(|a, b| b.order.cmp(&a.order));
        trace!(
            "{}",
            possible_orders_except_one
                .iter()
                .map(|a| format!("({:?}, {})", a.order, a.min_piece_count))
                .collect::<Vec<_>>()
                .join("\n")
        );

        let mut registers =
            vec![PossibleOrder::initialized(); usize::from(total_register_count.get())];
        let mut max_last_register = OrderExps::one();
        let mut iter_count = 0;
        let bump = Bump::new();
        let mut cycle_combinations = CycleCombinationParetoFront::default();
        unsafe {
            cycle_combinations_helper(
                &possible_orders_except_one,
                NonZeroUsize::from(total_register_count),
                total_piece_count,
                &mut max_last_register,
                &mut registers,
                &mut cycle_combinations,
                &mut iter_count,
                &bump,
            );
        }
        drop(possible_orders_except_one);
        debug!("Cycle combinations in {iter_count} iterations");
        Vec::from(cycle_combinations)
            .into_iter()
            .map(|candidate| CycleCombination {
                orders: Box::clone_from_ref(&candidate.orders),
                details: candidate.details,
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

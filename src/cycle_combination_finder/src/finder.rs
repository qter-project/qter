use std::{
    cmp::Ordering,
    num::{NonZeroU16, NonZeroU32},
    ops::Deref,
    time::Instant,
};

use humanize_duration::{Truncate, prelude::DurationExt};
use log::{debug, trace};

use crate::{
    cycle_combination_details::CycleCombinationDetails,
    cycle_combinations_tree::CycleCombinationsTree, min_piece_count::MinPieceCount,
    orderexps::OrderExps, puzzle::PuzzleDef,
};

#[derive(Clone, Copy, Default)]
pub enum Optimality {
    Equivalent,
    #[default]
    Optimal,
}

#[derive(Clone, Copy, Default)]
pub enum RegisterCount {
    Exactly(NonZeroU16),
    #[default]
    All,
}

#[derive(Debug, Clone)]
pub struct PossibleOrder<const N: usize> {
    pub(crate) order: OrderExps<N>,
    pub(crate) min_piece_count: NonZeroU32,
}

#[derive(Debug)]
pub struct CycleCombination<const N: usize> {
    pub(crate) registers: Box<[PossibleOrder<N>]>,
    pub(crate) details: CycleCombinationDetails<N>,
}

pub struct CycleCombinationFinder<const N: usize> {
    puzzle_def: PuzzleDef<N>,
    config: CycleCombinationFinderConfig,
}

#[derive(Clone, Copy, Default)]
pub struct CycleCombinationFinderConfig {
    optimality: Optimality,
    register_count: RegisterCount,
    // TODO:
    // max_solutions_count: 
    sorted: bool,
}

impl<const N: usize> PossibleOrder<N> {
    #[must_use]
    pub fn initialized() -> Self {
        #[allow(clippy::missing_panics_doc)]
        PossibleOrder {
            order: OrderExps::one(),
            min_piece_count: 1.try_into().unwrap(),
        }
    }
}

impl<const N: usize> Ord for PossibleOrder<N> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order.cmp(&other.order)
    }
}

impl<const N: usize> PartialOrd for PossibleOrder<N> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<const N: usize> Eq for PossibleOrder<N> {}

impl<const N: usize> PartialEq for PossibleOrder<N> {
    fn eq(&self, other: &Self) -> bool {
        self.order == other.order
    }
}

impl<const N: usize> Ord for CycleCombination<N> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.registers.iter().cmp(&other.registers)
    }
}

impl<const N: usize> PartialOrd for CycleCombination<N> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<const N: usize> Eq for CycleCombination<N> {}

impl<const N: usize> PartialEq for CycleCombination<N> {
    fn eq(&self, other: &Self) -> bool {
        self.registers == other.registers
    }
}

impl<const N: usize> CycleCombination<N> {
    pub fn registers(&self) -> impl Iterator<Item = &OrderExps<N>> {
        self.registers
            .iter()
            .map(|PossibleOrder { order, .. }| order)
    }
}

impl<const N: usize> Deref for CycleCombination<N> {
    type Target = CycleCombinationDetails<N>;

    fn deref(&self) -> &Self::Target {
        &self.details
    }
}

impl<const N: usize> From<PuzzleDef<N>> for CycleCombinationFinder<N> {
    fn from(puzzle_def: PuzzleDef<N>) -> Self {
        Self {
            puzzle_def,
            config: CycleCombinationFinderConfig::default(),
        }
    }
}

impl<const N: usize> CycleCombinationFinder<N> {
    #[must_use]
    pub fn with_sorted(mut self, sorted: bool) -> Self {
        self.config.sorted = sorted;
        self
    }

    #[must_use]
    pub fn with_optimality(mut self, optimality: Optimality) -> Self {
        self.config.optimality = optimality;
        self
    }

    #[must_use]
    pub fn with_register_count(mut self, register_count: RegisterCount) -> Self {
        self.config.register_count = register_count;
        self
    }

    fn find_optimal(&self, register_count: RegisterCount) -> Vec<CycleCombination<N>> {
        let RegisterCount::Exactly(exact_register_count) = register_count else {
            panic!("expected exactly variant for now");
        };

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
        CycleCombinationsTree::new(
            exact_register_count,
            possible_orders_except_one,
            self.puzzle_def.orbit_defs(),
        )
        .search_dfs()
    }

    #[must_use]
    pub fn find(&self) -> Vec<CycleCombination<N>> {
        let mut ret = match self.config.optimality {
            Optimality::Equivalent => unimplemented!(),
            Optimality::Optimal => self.find_optimal(self.config.register_count),
        };
        ret.sort_unstable();
        ret
    }
}

use std::{
    cell::OnceCell,
    cmp::Ordering,
    num::{NonZeroU16, NonZeroU32, NonZeroUsize},
    ops::Deref,
    rc::Rc,
    sync::Arc,
    time::Instant,
};

use humanize_duration::{Truncate, prelude::DurationExt};
use log::{debug, trace};
use thiserror::Error;

use crate::{
    cycle_combination_details::CycleCombinationDetails,
    cycle_combinations_tree::{dbg_registers, search_dfs},
    min_piece_count::MinPieceCount,
    orderexps::OrderExps,
    puzzle::PuzzleDef,
};

#[derive(Clone, Copy, Default)]
pub enum Optimality {
    Equivalent,
    #[default]
    Optimal,
}

#[derive(Clone, Copy, Default)]
pub enum NumCores {
    #[default]
    AllCores,
    Num(NonZeroUsize),
}

#[derive(Debug, Clone)]
pub struct PossibleOrder<const N: usize> {
    pub(crate) order: OrderExps<N>,
    pub(crate) min_piece_count: NonZeroU32,
}

#[derive(Debug, Clone)]
pub(crate) struct CycleCombination {
    pub(crate) inner: Arc<CycleCombinationInner>,
}

#[derive(Debug)]
pub(crate) struct CycleCombinationInner {
    pub(crate) registers: Box<[u32]>,
    pub(crate) details: CycleCombinationDetails,
}

pub struct CycleCombinations<const N: usize> {
    data: Box<[CycleCombination]>,
    possible_orders_except_one: Arc<[PossibleOrder<N>]>,
}

#[derive(Error, Debug)]
pub enum CycleCombinationFinderError {
    #[error(
        "This puzzle has too many orders. This is a hint that your puzzle is anyways too large \
         for the CCF to finish computing in a reasonable amount of time."
    )]
    PuzzleTooManyOrders,
}

#[derive(Clone)]
pub struct NeedsRegisterCount;

#[derive(Clone)]
pub struct HasRegisterCount(NonZeroU16);

#[derive(Clone)]
pub struct NeedsPuzzleDef;

#[derive(Clone)]
pub struct HasPuzzleDef<const N: usize> {
    puzzle_def: PuzzleDef<N>,
    possible_orders_except_one: OnceCell<Arc<[PossibleOrder<N>]>>,
}

#[derive(Clone)]
pub struct CycleCombinationFinder<R, P> {
    config: CycleCombinationFinderConfig,
    register_count: R,
    puzzle_def: Rc<P>,
}

#[derive(Clone, Copy, Default)]
pub struct CycleCombinationFinderConfig {
    optimality: Optimality,
    num_cores: NumCores,
    sorted: bool,
    maybe_expected_length: Option<usize>,
}

impl Ord for CycleCombination {
    fn cmp(&self, other: &Self) -> Ordering {
        self.inner.registers.iter().cmp(&other.inner.registers)
    }
}

impl PartialOrd for CycleCombination {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for CycleCombination {}

impl PartialEq for CycleCombination {
    fn eq(&self, other: &Self) -> bool {
        self.inner.registers == other.inner.registers
    }
}

impl<const N: usize> CycleCombinations<N> {
    pub fn registers(&self) -> impl Iterator<Item = impl Iterator<Item = &OrderExps<N>>> {
        self.data.iter().map(|x| {
            x.inner.registers
                .iter()
                .map(|&i| &self.possible_orders_except_one[i as usize].order)
        })
    }
}

impl Deref for CycleCombination {
    type Target = CycleCombinationDetails;

    fn deref(&self) -> &Self::Target {
        &self.inner.details
    }
}

impl CycleCombinationFinder<NeedsRegisterCount, NeedsPuzzleDef> {
    #[must_use]
    pub fn builder() -> Self {
        CycleCombinationFinder {
            config: CycleCombinationFinderConfig::default(),
            register_count: NeedsRegisterCount,
            puzzle_def: Rc::new(NeedsPuzzleDef),
        }
    }
}

impl<R, P> CycleCombinationFinder<R, P> {
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
    pub fn with_num_cores(mut self, num_cores: NumCores) -> Self {
        self.config.num_cores = num_cores;
        self
    }

    #[must_use]
    pub fn with_expected_length_assertion(mut self, expected_length: usize) -> Self {
        self.config.maybe_expected_length = Some(expected_length);
        self
    }

    #[must_use]
    pub fn with_register_count(
        self,
        register_count: NonZeroU16,
    ) -> CycleCombinationFinder<HasRegisterCount, P> {
        CycleCombinationFinder {
            config: self.config,
            register_count: HasRegisterCount(register_count),
            puzzle_def: self.puzzle_def,
        }
    }

    #[must_use]
    pub fn with_puzzle_def<const N: usize>(
        self,
        puzzle_def: PuzzleDef<N>,
    ) -> CycleCombinationFinder<R, HasPuzzleDef<N>> {
        CycleCombinationFinder {
            config: self.config,
            register_count: self.register_count,
            puzzle_def: Rc::new(HasPuzzleDef {
                puzzle_def,
                possible_orders_except_one: OnceCell::default(),
            }),
        }
    }
}

impl<const N: usize> CycleCombinationFinder<HasRegisterCount, HasPuzzleDef<N>> {
    /// Search for CCF solutions in parallel.
    ///
    /// # Errors
    ///
    /// Errors if the puzzle specified during initialization has too many orders
    /// of elements. In other words, if your puzzle is unreasonably large.
    ///
    /// # Panics
    ///
    /// Panics if an expected length assertion was set via
    /// [`Self::with_expected_length_assertion`] and the solutions length
    /// mismatches.
    pub fn find(self) -> Result<CycleCombinations<N>, CycleCombinationFinderError> {
        let HasRegisterCount(exact_register_count) = self.register_count;
        let HasPuzzleDef {
            puzzle_def,
            possible_orders_except_one,
        } = &*self.puzzle_def;
        let possible_orders_except_one = possible_orders_except_one.get_or_try_init(|| {
            let maybe_pool = if let NumCores::Num(num_cores) = self.config.num_cores {
                Some(
                    rayon::ThreadPoolBuilder::new()
                        .num_threads(num_cores.get())
                        .build()
                        .unwrap(),
                )
            } else {
                None
            };
            let possible_orders = puzzle_def
                .possible_orders(maybe_pool)
                .ok_or(CycleCombinationFinderError::PuzzleTooManyOrders)?;
            possible_orders.remove(&OrderExps::one());
            let now = Instant::now();
            let mut min_piece_count_calculator = MinPieceCount::from(puzzle_def);
            let mut possible_orders_except_one = possible_orders
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
            possible_orders_except_one.sort_unstable_by(|a, b| a.order.cmp(&b.order));
            trace!(
                "Possible orders: {}",
                possible_orders_except_one
                    .iter()
                    .map(|a| format!("{:?}", a.order))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            Ok(Arc::from(possible_orders_except_one.into_boxed_slice()))
        })?;
        let mut cycle_combinations = match self.config.optimality {
            Optimality::Equivalent => unimplemented!(),
            Optimality::Optimal => search_dfs(
                puzzle_def,
                possible_orders_except_one,
                exact_register_count,
                self.config.num_cores,
                10,
                NonZeroUsize::new(1).unwrap(),
            ),
        };
        if self.config.sorted {
            cycle_combinations.sort_unstable();
        }
        let cycle_combinations = cycle_combinations.into_boxed_slice();
        if let Some(expected_length) = self.config.maybe_expected_length {
            assert_eq!(
                cycle_combinations.len(),
                expected_length,
                "Expected {expected_length} solutions, found {}. Solutions: {}",
                cycle_combinations.len(),
                cycle_combinations
                    .into_iter()
                    .map(|i| dbg_registers(i.inner.registers.iter().copied(), possible_orders_except_one))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            debug!("Successfully found {} solutions", cycle_combinations.len());
            trace!("{cycle_combinations:?}");
        }
        Ok(CycleCombinations {
            data: cycle_combinations,
            possible_orders_except_one: Arc::clone(possible_orders_except_one),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU16;

    use crate::{
        finder::{CycleCombinationFinder, CycleCombinations},
        puzzle::{
            cubeN::CUBE3,
            minxN::{MINX3, MINX4},
        },
    };

    #[allow(unused)]
    fn cycles<const N: usize>(cycle_combinations: CycleCombinations<N>) -> Vec<Vec<u64>> {
        let cycles = cycle_combinations
            .registers()
            .map(|cycle_combination| {
                cycle_combination
                    .map(|register| register.as_bigint().try_into().unwrap())
                    .collect::<Vec<u64>>()
            })
            .collect::<Vec<_>>();
        drop(cycle_combinations);
        cycles
    }

    #[test_log::test]
    fn minx4_optimal_3() {
        let minx4 = MINX4.clone();
        CycleCombinationFinder::builder()
            .with_puzzle_def(minx4)
            .with_register_count(NonZeroU16::new(3).unwrap())
            .with_expected_length_assertion(251)
            .find()
            .unwrap();
    }

    #[ignore = "takes too long"]
    #[test_log::test]
    fn minx3_optimal_5() {
        let minx3 = MINX3.clone();
        CycleCombinationFinder::builder()
            .with_puzzle_def(minx3)
            .with_register_count(NonZeroU16::new(5).unwrap())
            .with_expected_length_assertion(1052)
            .find()
            .unwrap();
    }

    #[test_log::test]
    fn minx3_optimal_4() {
        let minx3 = MINX3.clone();
        CycleCombinationFinder::builder()
            .with_puzzle_def(minx3)
            .with_register_count(NonZeroU16::new(4).unwrap())
            .with_expected_length_assertion(347)
            .find()
            .unwrap();
    }

    #[test_log::test]
    fn minx3_optimal_3() {
        let minx3 = MINX3.clone();
        CycleCombinationFinder::builder()
            .with_puzzle_def(minx3)
            .with_register_count(NonZeroU16::new(3).unwrap())
            .with_expected_length_assertion(64)
            .find()
            .unwrap();
    }

    #[test_log::test]
    fn cube3_optimal_4() {
        let cube3 = CUBE3.clone();
        CycleCombinationFinder::builder()
            .with_puzzle_def(cube3)
            .with_register_count(NonZeroU16::new(4).unwrap())
            .with_expected_length_assertion(50)
            .find()
            .unwrap();
    }

    #[test_log::test]
    fn cube3_optimal_3() {
        let cube3 = CUBE3.clone();
        CycleCombinationFinder::builder()
            .with_puzzle_def(cube3)
            .with_register_count(NonZeroU16::new(3).unwrap())
            .with_expected_length_assertion(17)
            .find()
            .unwrap();
    }

    #[test_log::test]
    fn cube3_optimal_2() {
        let cube3 = CUBE3.clone();
        CycleCombinationFinder::builder()
            .with_puzzle_def(cube3)
            .with_register_count(NonZeroU16::new(2).unwrap())
            .with_expected_length_assertion(5)
            .find()
            .unwrap();
    }
}

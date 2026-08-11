use std::{
    cell::OnceCell,
    cmp::Ordering,
    fmt::{self},
    num::{NonZeroU16, NonZeroUsize},
    sync::{
        Arc,
        atomic::{self, AtomicUsize},
        nonpoison::Mutex,
    },
    time::{Duration, Instant},
};

use humanize_duration::{Truncate, prelude::DurationExt};
use log::{Level, debug, log_enabled, trace};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use thiserror::Error;

use crate::{
    cycle_combination_solutions::{
        CycleCombinationSolution, CycleCombinationSolutions, CycleCombinationSolutionsCalculator,
    },
    cycle_combinations_tree::{DisjointRegisters, dbg_registers, search_dfs},
    min_piece_count::MinPieceCount,
    nonemptyvec::NonemptySlice,
    orderexps::OrderExps,
    possible_orders::OrdersDashSet,
    puzzle::{PuzzleDef, possible_order_index_cast},
};

#[derive(Clone, Copy, Default, Debug)]
pub enum Optimality {
    Equivalent,
    #[default]
    Optimal,
    // TODO: min order ratio
    MaxOrderRatio(f64),
}

#[derive(Clone, Copy, Default, Debug)]
pub enum NumCores {
    #[default]
    AllCores,
    Num(NonZeroUsize),
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub enum SolutionExpansion {
    #[default]
    All,
    Limit(NonZeroUsize),
}

impl SolutionExpansion {
    pub const FIRST: Self = SolutionExpansion::Limit(NonZeroUsize::new(1).unwrap());
}

#[derive(Debug, Clone)]
pub struct PossibleOrder<const N: usize> {
    pub(crate) order: OrderExps<N>,
    pub(crate) min_piece_count: NonZeroU16,
}

#[derive(Debug)]
pub struct CycleCombination {
    pub(crate) registers: Arc<[u32]>,
    pub(crate) solutions: CycleCombinationSolutions,
}

pub struct CycleCombinations<const N: usize> {
    cycle_combinations: Box<[CycleCombination]>,
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
pub struct HasPuzzleDef<'a, const N: usize> {
    puzzle_def: &'a PuzzleDef<N>,
    possible_orders_except_one: OnceCell<Arc<[PossibleOrder<N>]>>,
}

#[derive(Clone)]
pub struct CycleCombinationFinder<R, P> {
    config: CycleCombinationFinderConfig,
    register_count: R,
    puzzle_def: P,
}

#[derive(Debug, Clone, Copy)]
pub struct CycleCombinationFinderConfig {
    pub optimality: Optimality,
    pub num_cores: NumCores,
    pub sorted: bool,
    pub maybe_expected_length: Option<usize>,
    pub maybe_max_fitting_tries: Option<u32>,
    pub solution_expansion: SolutionExpansion,
    pub mss_batch_size: NonZeroUsize,
    pub maybe_time_limit: Option<Duration>,
}

impl Default for CycleCombinationFinderConfig {
    fn default() -> Self {
        Self {
            optimality: Optimality::default(),
            num_cores: NumCores::default(),
            sorted: true,
            maybe_expected_length: None,
            maybe_max_fitting_tries: None,
            solution_expansion: SolutionExpansion::default(),
            mss_batch_size: NonZeroUsize::new(10).unwrap(),
            maybe_time_limit: None,
        }
    }
}

impl CycleCombination {
    #[must_use]
    pub fn orders_fmt<'a, const N: usize>(
        &'a self,
        possible_orders_except_one: &'a [PossibleOrder<N>],
    ) -> String {
        struct OrdersDisplay<'a, const N: usize> {
            inner: &'a CycleCombination,
            possible_orders_except_one: &'a [PossibleOrder<N>],
        }
        impl<const N: usize> fmt::Display for OrdersDisplay<'_, N> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let orders = self
                    .inner
                    .registers
                    .iter()
                    .map(|&register_index| {
                        self.possible_orders_except_one[possible_order_index_cast(register_index)]
                            .order
                            .as_bigint()
                            .to_string()
                    })
                    .collect::<Vec<_>>()
                    .join(", ");

                write!(f, "{orders}")?;
                Ok(())
            }
        }
        format!(
            "{}",
            OrdersDisplay {
                inner: self,
                possible_orders_except_one,
            }
        )
    }

    #[must_use]
    pub fn solutions_fmt<'a, const N: usize>(
        &'a self,
        possible_orders_except_one: &'a [PossibleOrder<N>],
        puzzle_def: &'a PuzzleDef<N>,
    ) -> String {
        struct SolutionsDisplay<'a, const N: usize> {
            inner: &'a CycleCombination,
            possible_orders_except_one: &'a [PossibleOrder<N>],
            puzzle_def: &'a PuzzleDef<N>,
        }
        impl<const N: usize> fmt::Display for SolutionsDisplay<'_, N> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                for CycleCombinationSolution {
                    orbit_remaining_pieces,
                    register_orbit_cycles,
                } in &self.inner.solutions.0
                {
                    let mut reg_orbit_cycles = register_orbit_cycles.iter();
                    for register_index in 0..self.inner.registers.len() {
                        writeln!(
                            f,
                            "{:?}:",
                            self.possible_orders_except_one
                                [possible_order_index_cast(self.inner.registers[register_index])]
                            .order
                        )?;
                        writeln!(f)?;
                        for orbit_index2 in 0..self.puzzle_def.orbit_defs().len().get() {
                            let s = reg_orbit_cycles
                                .next()
                                .unwrap()
                                .iter()
                                .map(|cycle| {
                                    let mut s = format!("{}", cycle.piece_count);
                                    if cycle.must_orient {
                                        s.push('+');
                                    }
                                    s
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            writeln!(f, "{orbit_index2}: ({s})")?;
                        }
                        writeln!(f)?;
                    }
                    for (orbit_index2, orbit_remaining_piece) in
                        orbit_remaining_pieces.iter().enumerate()
                    {
                        writeln!(
                            f,
                            "{orbit_index2}: {} ignored, {} unused",
                            orbit_remaining_piece.ignored,
                            orbit_remaining_piece
                                .unused
                                .checked_sub(orbit_remaining_piece.ignored)
                                .unwrap()
                        )?;
                    }
                    writeln!(f)?;
                }
                Ok(())
            }
        }
        format!(
            "{}",
            SolutionsDisplay {
                inner: self,
                puzzle_def,
                possible_orders_except_one,
            }
        )
    }
}

impl Ord for CycleCombination {
    fn cmp(&self, other: &Self) -> Ordering {
        self.registers.iter().cmp(&*other.registers)
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
        self.registers == other.registers
    }
}

impl<const N: usize> CycleCombinations<N> {
    pub fn registers(&self) -> impl Iterator<Item = impl Iterator<Item = &OrderExps<N>>> {
        self.cycle_combinations.iter().map(|x| {
            x.registers
                .iter()
                .map(|&i| &self.possible_orders_except_one[i as usize].order)
        })
    }
}

impl CycleCombinationFinder<NeedsRegisterCount, NeedsPuzzleDef> {
    #[must_use]
    pub fn builder() -> Self {
        CycleCombinationFinder {
            config: CycleCombinationFinderConfig::default(),
            register_count: NeedsRegisterCount,
            puzzle_def: NeedsPuzzleDef,
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
        self.config.optimality = if let Optimality::MaxOrderRatio(max_order_ratio) = optimality
            && max_order_ratio <= 1.0
        {
            Optimality::Equivalent
        } else {
            optimality
        };
        self
    }

    #[must_use]
    pub fn with_num_cores(mut self, num_cores: NumCores) -> Self {
        self.config.num_cores = num_cores;
        self
    }

    #[must_use]
    pub fn with_expected_length_assertion(mut self, maybe_expected_length: Option<usize>) -> Self {
        self.config.maybe_expected_length = maybe_expected_length;
        self
    }

    #[must_use]
    pub fn with_max_fitting_tries(mut self, maybe_max_fitting_tries: Option<u32>) -> Self {
        self.config.maybe_max_fitting_tries = maybe_max_fitting_tries;
        self
    }

    #[must_use]
    pub fn with_solution_expansion(mut self, solution_expansion: SolutionExpansion) -> Self {
        self.config.solution_expansion = solution_expansion;
        self
    }

    #[must_use]
    pub fn with_mss_batch_size(mut self, mss_batch_size: NonZeroUsize) -> Self {
        self.config.mss_batch_size = mss_batch_size;
        self
    }

    #[must_use]
    pub fn with_time_limit(mut self, maybe_time_limit: Option<Duration>) -> Self {
        self.config.maybe_time_limit = maybe_time_limit;
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
        puzzle_def: &PuzzleDef<N>,
    ) -> CycleCombinationFinder<R, HasPuzzleDef<'_, N>> {
        CycleCombinationFinder {
            config: self.config,
            register_count: self.register_count,
            puzzle_def: HasPuzzleDef {
                puzzle_def,
                possible_orders_except_one: OnceCell::default(),
            },
        }
    }
}

pub(crate) fn mk_possible_orders_except_one<const N: usize>(
    puzzle_def: &PuzzleDef<N>,
    possible_orders: OrdersDashSet<N>,
) -> Vec<PossibleOrder<N>> {
    assert!(possible_orders.remove(&OrderExps::one()).is_some());
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
        "Possible orders done; first 100: {}",
        possible_orders_except_one
            .iter()
            .map(|a| format!("{:?}", a.order))
            .take(100)
            .collect::<Vec<_>>()
            .join(" ")
    );
    possible_orders_except_one
}

impl<const N: usize> CycleCombinationFinder<HasRegisterCount, HasPuzzleDef<'_, N>> {
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
        } = self.puzzle_def;
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

        let possible_orders_except_one = possible_orders_except_one.get_or_try_init(|| {
            let possible_orders = puzzle_def
                .possible_orders(maybe_pool.as_ref())
                .ok_or(CycleCombinationFinderError::PuzzleTooManyOrders)?;
            let possible_orders_except_one =
                mk_possible_orders_except_one(puzzle_def, possible_orders);
            Ok(Arc::from(possible_orders_except_one.into_boxed_slice()))
        })?;
        let mut possible_registers = match self.config.optimality {
            Optimality::Equivalent => unimplemented!(),
            Optimality::Optimal => search_dfs(
                puzzle_def,
                &self.config,
                possible_orders_except_one,
                exact_register_count,
                None,
            ),
            Optimality::MaxOrderRatio(max_order_ratio) => search_dfs(
                puzzle_def,
                &self.config,
                possible_orders_except_one,
                exact_register_count,
                Some(max_order_ratio),
            ),
        };
        if self.config.sorted {
            possible_registers.sort_unstable();
        }
        let expansion_percent_done = AtomicUsize::new(0);
        let logged_bucket = Mutex::new(0);

        let expand = || {
            possible_registers
                .par_iter()
                .map_init(
                    || {
                        CycleCombinationSolutionsCalculator::new(
                            puzzle_def,
                            possible_orders_except_one,
                            exact_register_count,
                            self.config.solution_expansion,
                            self.config.maybe_max_fitting_tries,
                        )
                    },
                    |solutions_calculator, possible_register| {
                        let possible_register2 = DisjointRegisters::from(
                            NonemptySlice::try_from(&**possible_register)
                                .expect("The number of registers is non-zero"),
                        );
                        let cycle_combination = CycleCombination {
                            registers: Arc::clone(possible_register),
                            solutions: solutions_calculator
                                .expansion(possible_register2)
                                .expect("This solution is in the front and therefore exists"),
                        };

                        if log_enabled!(Level::Debug) {
                            const PERCENT: usize = 1;

                            let done =
                                expansion_percent_done.fetch_add(1, atomic::Ordering::Relaxed) + 1;
                            let new_bucket = done * 100 / (PERCENT * possible_registers.len());
                            let mut bucket = logged_bucket.lock();
                            if new_bucket > *bucket {
                                *bucket = new_bucket;
                                debug!("Expansion: {}%", done * 100 / possible_registers.len());
                            }
                        }

                        cycle_combination
                    },
                )
                .collect::<Box<[_]>>()
        };

        let now = Instant::now();
        let cycle_combinations = maybe_pool.map_or_else(expand, |pool| pool.install(expand));
        debug!("Expansion took: {}", now.elapsed().human(Truncate::Micro));
        debug!(
            "Found {} solutions, with {} expansions average",
            cycle_combinations.len(),
            cycle_combinations
                .iter()
                .map(|cycle_combination| cycle_combination.solutions.0.len())
                .sum::<usize>()
        );
        if let Some(expected_length) = self.config.maybe_expected_length {
            assert_eq!(
                cycle_combinations.len(),
                expected_length,
                "Expected {expected_length} solutions, found {}. Solutions: {}",
                cycle_combinations.len(),
                cycle_combinations
                    .into_iter()
                    .map(|i| dbg_registers(i.registers.iter().copied(), possible_orders_except_one))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            trace!("{cycle_combinations:?}");
        }
        Ok(CycleCombinations {
            cycle_combinations,
            possible_orders_except_one: Arc::clone(possible_orders_except_one),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU16, NonZeroUsize};

    use crate::{
        finder::{
            CycleCombinationFinder, CycleCombinations,
            NumCores::{self},
            Optimality, SolutionExpansion,
        },
        puzzle::{
            cubeN::{CUBE3, CUBE4},
            minxN::{MINX3, MINX4, MINX5},
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
    fn minx3_optimal_3() {
        let minx3 = MINX3.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx3)
            .with_optimality(Optimality::MaxOrderRatio(1.01))
            .with_num_cores(NumCores::Num(NonZeroUsize::new(1).unwrap()))
            .with_register_count(NonZeroU16::new(3).unwrap())
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!(
                "{}",
                x.solutions_fmt(&ret.possible_orders_except_one, &minx3)
            );
        }
    }

    #[test_log::test]
    fn minx3_optimal_4() {
        let minx3 = MINX3.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx3)
            .with_register_count(NonZeroU16::new(4).unwrap())
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!(
                "{}",
                x.solutions_fmt(&ret.possible_orders_except_one, &minx3)
            );
        }
    }

    #[test_log::test]
    fn minx3_optimal_5() {
        let minx3 = MINX3.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx3)
            .with_register_count(NonZeroU16::new(5).unwrap())
            .with_optimality(Optimality::MaxOrderRatio(10.0))
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!(
                "{}",
                x.solutions_fmt(&ret.possible_orders_except_one, &minx3)
            );
        }
    }

    #[test_log::test]
    fn minx3_optimal_6() {
        let minx3 = MINX3.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx3)
            .with_register_count(NonZeroU16::new(6).unwrap())
            .with_optimality(Optimality::MaxOrderRatio(10.0))
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!(
                "{}",
                x.solutions_fmt(&ret.possible_orders_except_one, &minx3)
            );
        }
    }

    #[test_log::test]
    fn minx4_optimal_3() {
        let minx4 = MINX4.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx4)
            .with_register_count(NonZeroU16::new(3).unwrap())
            .with_time_limit(None)
            .with_optimality(Optimality::MaxOrderRatio(5.0))
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!("{}", x.orders_fmt(&ret.possible_orders_except_one));
        }
    }

    #[test_log::test]
    fn minx4_optimal_4() {
        let minx4 = MINX4.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx4)
            .with_register_count(NonZeroU16::new(4).unwrap())
            .with_optimality(Optimality::MaxOrderRatio(10.0))
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!(
                "{}",
                x.solutions_fmt(&ret.possible_orders_except_one, &minx4)
            );
        }
    }

    #[test_log::test]
    fn minx4_optimal_5() {
        let minx4 = MINX4.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx4)
            .with_register_count(NonZeroU16::new(5).unwrap())
            .with_optimality(Optimality::MaxOrderRatio(10.0))
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!(
                "{}",
                x.solutions_fmt(&ret.possible_orders_except_one, &minx4)
            );
        }
    }

    #[test_log::test]
    fn minx5_optimal_2() {
        let minx5 = MINX5.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx5)
            .with_register_count(NonZeroU16::new(2).unwrap())
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!("{}", x.orders_fmt(&ret.possible_orders_except_one));
        }
    }

    #[test_log::test]
    fn minx5_optimal_3() {
        let minx5 = MINX5.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx5)
            .with_register_count(NonZeroU16::new(3).unwrap())
            .with_max_fitting_tries(Some(500))
            .with_optimality(Optimality::MaxOrderRatio(1.0))
            .with_solution_expansion(SolutionExpansion::Limit(NonZeroUsize::new(10).unwrap()))
            .with_mss_batch_size(NonZeroUsize::new(1).unwrap())
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!(
                "{}",
                x.solutions_fmt(&ret.possible_orders_except_one, &minx5)
            );
        }
    }

    #[test_log::test]
    fn cube3_optimal_4() {
        let cube3 = CUBE3.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&cube3)
            .with_register_count(NonZeroU16::new(4).unwrap())
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!(
                "{}",
                x.solutions_fmt(&ret.possible_orders_except_one, &cube3)
            );
        }
    }

    #[test_log::test]
    fn cube3_optimal_3() {
        let cube3 = CUBE3.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&cube3)
            .with_register_count(NonZeroU16::new(3).unwrap())
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!(
                "{}",
                x.solutions_fmt(&ret.possible_orders_except_one, &cube3)
            );
        }
    }

    #[test_log::test]
    fn cube4_optimal_2() {
        let cube4 = CUBE4.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&cube4)
            .with_register_count(NonZeroU16::new(2).unwrap())
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!(
                "{}",
                x.solutions_fmt(&ret.possible_orders_except_one, &cube4)
            );
        }
    }
}

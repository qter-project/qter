use std::{
    cell::OnceCell,
    cmp::Ordering,
    fmt::{self},
    num::{NonZeroU16, NonZeroU32, NonZeroUsize},
    sync::{
        Arc,
        atomic::{self, AtomicUsize},
        nonpoison::Mutex,
    },
    time::Instant,
};

use humanize_duration::{Truncate, prelude::DurationExt};
use log::{Level, debug, log_enabled, trace};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use thiserror::Error;

use crate::{
    cycle_combination_details::{CycleCombinationDetail, CycleCombinationDetails},
    cycle_combinations_tree::{DisjointRegisters, dbg_registers, search_dfs},
    min_piece_count::MinPieceCount,
    nonemptyvec::NonemptySlice,
    orderexps::OrderExps,
    possible_orders::OrdersDashSet,
    puzzle::{PuzzleDef, possible_order_index_cast},
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

#[derive(Debug)]
pub struct CycleCombination {
    registers: Arc<[u32]>,
    detail: CycleCombinationDetail,
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

#[derive(Clone, Copy, Default)]
pub struct CycleCombinationFinderConfig {
    optimality: Optimality,
    pub(crate) num_cores: NumCores,
    sorted: bool,
    maybe_expected_length: Option<usize>,
    pub(crate) max_fitting_tries: Option<u32>,
}

impl CycleCombination {
    #[must_use]
    pub fn display_fmt<'a, const N: usize>(
        &'a self,
        possible_orders_except_one: &'a [PossibleOrder<N>],
        puzzle_def: &'a PuzzleDef<N>,
    ) -> String {
        struct CycleCombinationDisplay<'a, const N: usize> {
            inner: &'a CycleCombination,
            possible_orders_except_one: &'a [PossibleOrder<N>],
            puzzle_def: &'a PuzzleDef<N>,
        }
        impl<const N: usize> fmt::Display for CycleCombinationDisplay<'_, N> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                for (remaining_piece_counts, reg_orbit_cycles) in &self.inner.detail.detail {
                    let mut reg_orbit_cycles = reg_orbit_cycles.iter();
                    for register_index in 0..self.inner.registers.len() {
                        writeln!(
                            f,
                            "{:?}:",
                            self.possible_orders_except_one
                                [possible_order_index_cast(self.inner.registers[register_index])]
                            .order
                        )?;
                        writeln!(f)?;
                        // for orbit_index in 0..self.puzzle_def.orbit_defs().len().get() {
                        for (orbit_index, &orbit_def) in
                            self.puzzle_def.orbit_defs().iter().enumerate()
                        {
                            let s = reg_orbit_cycles
                                .next()
                                .unwrap()
                                .iter()
                                .map(|cycle| {
                                    if cycle.must_orient {
                                        format!(
                                            "{}+",
                                            cycle
                                                .piece_count
                                                .div_exact(u16::from(
                                                    orbit_def.orientation_count().get()
                                                ))
                                                .unwrap_or(cycle.piece_count)
                                        )
                                    } else {
                                        format!("{}", cycle.piece_count)
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            writeln!(f, "{orbit_index}: ({s})")?;
                        }
                        writeln!(f)?;
                    }
                    writeln!(f, "{remaining_piece_counts:#?}")?;
                    writeln!(f)?;
                }
                Ok(())
            }
        }
        format!(
            "\n{}----------",
            CycleCombinationDisplay {
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
    pub fn with_max_fitting_tries(mut self, max_fitting_tries: u32) -> Self {
        self.config.max_fitting_tries = Some(max_fitting_tries);
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
        "100 Possible orders: {}",
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
                1,
                NonZeroUsize::new(100).unwrap(),
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
                        CycleCombinationDetails::new(
                            exact_register_count,
                            possible_orders_except_one,
                            self.config.max_fitting_tries,
                            puzzle_def,
                        )
                    },
                    |details, possible_register| {
                        let possible_register2 = DisjointRegisters::from(
                            NonemptySlice::try_from(&**possible_register)
                                .expect("The number of registers is non-zero"),
                        );
                        let cycle_combination = CycleCombination {
                            registers: Arc::clone(possible_register),
                            detail: details
                                .calculate_all(possible_register2)
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
        debug!("Find all took: {}", now.elapsed().human(Truncate::Micro));
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
            debug!("Successfully found {} solutions", cycle_combinations.len());
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
    use std::num::NonZeroU16;

    use crate::{
        finder::{CycleCombinationFinder, CycleCombinations},
        puzzle::{
            cubeN::{CUBE3, CUBE4},
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
            .with_puzzle_def(&minx4)
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
            .with_puzzle_def(&minx3)
            .with_register_count(NonZeroU16::new(5).unwrap())
            .with_expected_length_assertion(1052)
            .find()
            .unwrap();
    }

    #[test_log::test]
    fn minx3_optimal_4() {
        let minx3 = MINX3.clone();
        CycleCombinationFinder::builder()
            .with_puzzle_def(&minx3)
            .with_register_count(NonZeroU16::new(4).unwrap())
            .with_expected_length_assertion(347)
            .find()
            .unwrap();
    }

    #[test_log::test]
    fn minx3_optimal_3() {
        let minx3 = MINX3.clone();
        CycleCombinationFinder::builder()
            .with_puzzle_def(&minx3)
            .with_register_count(NonZeroU16::new(3).unwrap())
            .with_expected_length_assertion(64)
            .find()
            .unwrap();
    }

    #[test_log::test]
    fn cube3_optimal_4() {
        let cube3 = CUBE3.clone();
        CycleCombinationFinder::builder()
            .with_puzzle_def(&cube3)
            .with_register_count(NonZeroU16::new(4).unwrap())
            .with_expected_length_assertion(50)
            .find()
            .unwrap();
    }

    #[test_log::test]
    fn cube3_optimal_3() {
        let cube3 = CUBE3.clone();
        CycleCombinationFinder::builder()
            .with_puzzle_def(&cube3)
            .with_register_count(NonZeroU16::new(3).unwrap())
            .with_expected_length_assertion(17)
            .find()
            .unwrap();
    }

    #[test_log::test]
    fn cube4_optimal_2() {
        let cube3 = CUBE4.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&cube3)
            .with_register_count(NonZeroU16::new(2).unwrap())
            .with_expected_length_assertion(13)
            .with_sorted(true)
            .find()
            .unwrap();
        // println!("{:?}", ret.data);
        for x in ret.cycle_combinations {
            println!("{}", x.display_fmt(&ret.possible_orders_except_one, &cube3));
        }
    }
}

use std::{
    cmp::Ordering,
    fmt::Write,
    num::{NonZeroU16, NonZeroUsize},
    sync::{Arc, atomic::AtomicUsize, nonpoison::Mutex},
    time::{Duration, Instant},
};

use humanize_duration::{Truncate, prelude::DurationExt};
use log::{debug, info, trace};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use thiserror::Error;

use crate::{
    cycle_combination_solutions::{
        CycleCombinationSolution, CycleCombinationSolutions, expand_possible_register,
    },
    min_piece_count::MinPieceCount,
    orderexps::OrderExps,
    possible_orders::OrdersDashSet,
    puzzle::{PuzzleDef, orbit_index_cast, possible_order_index_cast},
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Optimality {
    #[default]
    Optimal,
    MaxOrderRatio(f64),
    MinOrderRatio(f64),
    ClampOrderRatio(ClampOrderRatio),
}

impl Optimality {
    pub const EQUIVALENT: Self = Self::MaxOrderRatio(1.0);

    pub(crate) fn maybe_min_max_order_ratio(self) -> (Option<f64>, Option<f64>) {
        match self {
            Optimality::Optimal => (None, None),
            Optimality::MaxOrderRatio(max) => (None, Some(max)),
            Optimality::MinOrderRatio(min) => (Some(min), None),
            Optimality::ClampOrderRatio(ClampOrderRatio { max, min }) => (Some(min), Some(max)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClampOrderRatio {
    pub max: f64,
    pub min: f64,
}

#[derive(Clone, Copy, Default, Debug)]
pub(crate) enum ValidatedNumCores {
    #[default]
    AllCores,
    Num(NonZeroUsize),
}

#[derive(Clone, Copy, Default, Debug)]
pub enum NumCores {
    #[default]
    AllCores,
    Num(usize),
}

impl NumCores {
    pub const ONE: Self = NumCores::Num(1);
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub(crate) enum ValidatedSolutionExpansion {
    #[default]
    All,
    Limit(NonZeroUsize),
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub enum SolutionExpansion {
    #[default]
    All,
    Limit(usize),
}

impl SolutionExpansion {
    pub const FIRST: Self = Self::Limit(1);
}

#[derive(Clone)]
enum MssBatchSize {
    Invalid,
    Default,
    Value(NonZeroUsize),
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

#[derive(Debug)]
pub struct CycleCombinations<const N: usize> {
    pub cycle_combinations: Box<[CycleCombination]>,
    pub possible_orders_except_one: Arc<[PossibleOrder<N>]>,
}

#[derive(Error, Debug)]
pub enum CycleCombinationFinderError<const N: usize> {
    #[error(
        "This puzzle has too many orders. This is a hint that your puzzle is anyways too large \
         for the CCF to finish computing in a reasonable amount of time."
    )]
    PuzzleTooManyOrders,
    #[error("Expected {expected} solutions, found {actual}.")]
    MismatchedSolutionCount {
        cycle_combinations: CycleCombinations<N>,
        expected: usize,
        actual: usize,
    },
}

#[derive(Error, Debug)]
pub enum CycleCombinationFinderValidationError {
    #[error("Register count must be non-zero.")]
    InvalidRegisterCount,
    #[error(
        "Number of cores must be non-zero and less than or equal to the number of cores on this \
         machine."
    )]
    InvalidNumCores,
    #[error("MSS batch size must be non-zero.")]
    InvalidMssBatchSize,
    #[error("Solution expansion limit must be non-zero.")]
    InvalidSolutionExpansion,
    #[error(
        "Optimality config must have `maybe_min_order_ratio` and `maybe_max_order_ratio` finite \
         and >= 1.0 when set."
    )]
    InvalidOptimality,
}

#[derive(Clone)]
pub struct NeedsRegisterCount;

#[derive(Clone)]
pub struct HasRegisterCount(Option<NonZeroU16>);

pub trait RegisterCountState {}
impl RegisterCountState for NeedsRegisterCount {}
impl RegisterCountState for HasRegisterCount {}

#[derive(Clone)]
pub struct NeedsPuzzleDef;

#[derive(Clone)]
pub struct HasPuzzleDef<'a, const N: usize>(&'a PuzzleDef<N>);

pub trait PuzzleDefState {}
impl PuzzleDefState for NeedsPuzzleDef {}
impl<const N: usize> PuzzleDefState for HasPuzzleDef<'_, N> {}

#[derive(Clone)]
pub struct CycleCombinationFinder<R: RegisterCountState, P: PuzzleDefState> {
    register_count: R,
    puzzle_def: P,
    optimality: Option<Optimality>,
    num_cores: Option<ValidatedNumCores>,
    sorted: bool,
    maybe_expected_solution_count: Option<usize>,
    maybe_max_fitting_tries: Option<u32>,
    solution_expansion: Option<ValidatedSolutionExpansion>,
    mss_batch_size: MssBatchSize,
    maybe_time_limit: Option<Duration>,
    fast_assumptions: bool,
}

#[derive(Clone)]
pub struct ValidatedCycleCombinationFinder<'a, const N: usize> {
    pub(crate) register_count: NonZeroU16,
    pub(crate) puzzle_def: &'a PuzzleDef<N>,
    pub(crate) optimality: Optimality,
    pub(crate) num_cores: ValidatedNumCores,
    pub(crate) sorted: bool,
    pub(crate) maybe_expected_solution_count: Option<usize>,
    pub(crate) maybe_max_fitting_tries: Option<u32>,
    pub(crate) solution_expansion: ValidatedSolutionExpansion,
    pub(crate) mss_batch_size: NonZeroUsize,
    pub(crate) maybe_time_limit: Option<Duration>,
    pub(crate) fast_assumptions: bool,
}

impl CycleCombination {
    #[must_use]
    pub fn orders_fmt<const N: usize>(
        &self,
        possible_orders_except_one: &[PossibleOrder<N>],
    ) -> String {
        self.registers
            .iter()
            .map(|&register_index| {
                possible_orders_except_one[possible_order_index_cast(register_index)]
                    .order
                    .as_bigint()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    #[must_use]
    pub fn solutions_fmt<const N: usize>(
        &self,
        possible_orders_except_one: &[PossibleOrder<N>],
        puzzle_def: &PuzzleDef<N>,
    ) -> String {
        let mut ret = String::new();
        for CycleCombinationSolution {
            orbit_remaining_pieces,
            register_orbit_cycles,
        } in &self.solutions.0
        {
            let mut register_orbit_cycles_iter = register_orbit_cycles.iter();
            for register_index in 0..self.registers.len() {
                let _ = writeln!(
                    &mut ret,
                    "{:?}:",
                    possible_orders_except_one
                        [possible_order_index_cast(self.registers[register_index])]
                    .order
                );
                let _ = writeln!(&mut ret);
                for (orbit_index2, maybe_orbit_name) in puzzle_def.orbit_names().iter().enumerate()
                {
                    #[allow(clippy::missing_panics_doc)]
                    let s = register_orbit_cycles_iter
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
                    let _ = writeln!(
                        &mut ret,
                        "{}: ({s})",
                        maybe_orbit_name
                            .as_ref()
                            .and_then(|orbit_name| orbit_name.chars().next())
                            .unwrap_or_else(|| {
                                #[allow(clippy::missing_panics_doc)]
                                char::from_digit(u32::from(orbit_index_cast(orbit_index2)), 10)
                                    .unwrap()
                            }),
                    );
                }
                let _ = writeln!(&mut ret);
            }
            for (orbit_index2, (orbit_remaining_piece, maybe_orbit_name)) in orbit_remaining_pieces
                .iter()
                .zip(puzzle_def.orbit_names().iter())
                .enumerate()
            {
                let _ = writeln!(
                    &mut ret,
                    "{}: {} ignored, {} unused",
                    maybe_orbit_name
                        .as_ref()
                        .and_then(|orbit_name| orbit_name.chars().next())
                        .unwrap_or_else(|| {
                            #[allow(clippy::missing_panics_doc)]
                            char::from_digit(u32::from(orbit_index_cast(orbit_index2)), 10).unwrap()
                        }),
                    orbit_remaining_piece.ignored,
                    orbit_remaining_piece.unused,
                );
            }
            let _ = writeln!(&mut ret);
        }
        ret
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
            register_count: NeedsRegisterCount,
            puzzle_def: NeedsPuzzleDef,
            optimality: Some(Optimality::Optimal),
            num_cores: Some(ValidatedNumCores::default()),
            sorted: true,
            maybe_expected_solution_count: None,
            maybe_max_fitting_tries: None,
            solution_expansion: Some(ValidatedSolutionExpansion::default()),
            mss_batch_size: MssBatchSize::Default,
            maybe_time_limit: None,
            fast_assumptions: true,
        }
    }
}

impl<R: RegisterCountState, P: PuzzleDefState> CycleCombinationFinder<R, P> {
    #[must_use]
    pub fn with_sorted(mut self, sorted: bool) -> Self {
        self.sorted = sorted;
        self
    }

    #[must_use]
    pub fn with_optimality(mut self, optimality: Optimality) -> Self {
        let (maybe_min_order_ratio, maybe_max_order_ratio) = optimality.maybe_min_max_order_ratio();

        if let Some(min_order_ratio) = maybe_min_order_ratio {
            match min_order_ratio.partial_cmp(&1.0) {
                Some(Ordering::Less) | None => {
                    self.optimality = None;
                    return self;
                }
                _ => (),
            }
        }
        if let Some(max_order_ratio) = maybe_max_order_ratio {
            match max_order_ratio.partial_cmp(&1.0) {
                Some(Ordering::Less) | None => {
                    self.optimality = None;
                    return self;
                }
                _ => (),
            }
        }

        self.optimality = Some(optimality);
        self
    }

    #[must_use]
    pub fn with_num_cores(mut self, num_cores: NumCores) -> Self {
        self.num_cores = match num_cores {
            NumCores::AllCores => Some(ValidatedNumCores::AllCores),
            NumCores::Num(num) => NonZeroUsize::new(num)
                .filter(|num| num.get() <= num_cpus::get())
                .map(ValidatedNumCores::Num),
        };
        self
    }

    #[must_use]
    pub fn with_expected_solutions_count_assertion(
        mut self,
        maybe_expected_solution_count: Option<usize>,
    ) -> Self {
        self.maybe_expected_solution_count = maybe_expected_solution_count;
        self
    }

    #[must_use]
    pub fn with_max_fitting_tries(mut self, maybe_max_fitting_tries: Option<u32>) -> Self {
        self.maybe_max_fitting_tries = maybe_max_fitting_tries;
        self
    }

    #[must_use]
    pub fn with_solution_expansion(mut self, solution_expansion: SolutionExpansion) -> Self {
        self.solution_expansion = match solution_expansion {
            SolutionExpansion::All => Some(ValidatedSolutionExpansion::All),
            SolutionExpansion::Limit(limit) => {
                NonZeroUsize::new(limit).map(ValidatedSolutionExpansion::Limit)
            }
        };
        self
    }

    #[must_use]
    pub fn with_mss_batch_size(mut self, maybe_mss_batch_size: Option<usize>) -> Self {
        self.mss_batch_size = if let Some(mss_batch_size) = maybe_mss_batch_size {
            NonZeroUsize::new(mss_batch_size)
                .map_or_else(|| MssBatchSize::Invalid, MssBatchSize::Value)
        } else {
            MssBatchSize::Default
        };
        self
    }

    #[must_use]
    pub fn with_time_limit(mut self, maybe_time_limit: Option<Duration>) -> Self {
        self.maybe_time_limit = maybe_time_limit;
        self
    }

    #[must_use]
    pub fn with_fast_assumptions(mut self, fast_assumptions: bool) -> Self {
        self.fast_assumptions = fast_assumptions;
        self
    }

    #[must_use]
    pub fn with_register_count(
        self,
        register_count: u16,
    ) -> CycleCombinationFinder<HasRegisterCount, P> {
        CycleCombinationFinder {
            register_count: HasRegisterCount(NonZeroU16::new(register_count)),
            puzzle_def: self.puzzle_def,
            optimality: self.optimality,
            num_cores: self.num_cores,
            sorted: self.sorted,
            maybe_expected_solution_count: self.maybe_expected_solution_count,
            maybe_max_fitting_tries: self.maybe_max_fitting_tries,
            solution_expansion: self.solution_expansion,
            mss_batch_size: self.mss_batch_size,
            maybe_time_limit: self.maybe_time_limit,
            fast_assumptions: self.fast_assumptions,
        }
    }

    #[must_use]
    pub fn with_puzzle_def<const N: usize>(
        self,
        puzzle_def: &PuzzleDef<N>,
    ) -> CycleCombinationFinder<R, HasPuzzleDef<'_, N>> {
        CycleCombinationFinder {
            register_count: self.register_count,
            puzzle_def: HasPuzzleDef(puzzle_def),
            optimality: self.optimality,
            num_cores: self.num_cores,
            sorted: self.sorted,
            maybe_expected_solution_count: self.maybe_expected_solution_count,
            maybe_max_fitting_tries: self.maybe_max_fitting_tries,
            solution_expansion: self.solution_expansion,
            mss_batch_size: self.mss_batch_size,
            maybe_time_limit: self.maybe_time_limit,
            fast_assumptions: self.fast_assumptions,
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

impl<'a, const N: usize> CycleCombinationFinder<HasRegisterCount, HasPuzzleDef<'a, N>> {
    /// Validate the builder.
    ///
    /// # Errors
    ///
    /// Errors following the variants of
    /// `CycleCombinationFinderValidationError`.
    pub fn validate(
        self,
    ) -> Result<ValidatedCycleCombinationFinder<'a, N>, CycleCombinationFinderValidationError> {
        let CycleCombinationFinder {
            register_count,
            puzzle_def,
            optimality,
            num_cores,
            sorted,
            maybe_expected_solution_count,
            maybe_max_fitting_tries,
            solution_expansion,
            mss_batch_size,
            maybe_time_limit,
            fast_assumptions,
        } = self;
        Ok(ValidatedCycleCombinationFinder {
            register_count: register_count
                .0
                .ok_or(CycleCombinationFinderValidationError::InvalidRegisterCount)?,
            puzzle_def: puzzle_def.0,
            optimality: optimality
                .ok_or(CycleCombinationFinderValidationError::InvalidOptimality)?,
            num_cores: num_cores
                .ok_or(CycleCombinationFinderValidationError::InvalidNumCores)?,
            sorted,
            maybe_expected_solution_count,
            maybe_max_fitting_tries,
            solution_expansion: solution_expansion
                .ok_or(CycleCombinationFinderValidationError::InvalidSolutionExpansion)?,
            mss_batch_size: match mss_batch_size {
                MssBatchSize::Invalid => {
                    return Err(CycleCombinationFinderValidationError::InvalidMssBatchSize);
                }
                #[allow(clippy::missing_panics_doc)]
                MssBatchSize::Default => NonZeroUsize::new(1000).unwrap(),
                MssBatchSize::Value(value) => value,
            },
            maybe_time_limit,
            fast_assumptions,
        })
    }
}

impl<const N: usize> ValidatedCycleCombinationFinder<'_, N> {
    /// Search for CCF solutions in parallel.
    ///
    /// # Errors
    ///
    /// Errors follow the variants for [`CycleCombinationFinderError`].
    ///
    /// # Panics
    ///
    /// Panics if an expected length assertion was set via
    /// [`Self::with_expected_length_assertion`] and the solutions length
    /// mismatches.
    pub fn find(self) -> Result<CycleCombinations<N>, CycleCombinationFinderError<N>> {
        let maybe_pool = match self.num_cores {
            ValidatedNumCores::AllCores => None,
            ValidatedNumCores::Num(num_cores) => Some(
                rayon::ThreadPoolBuilder::new()
                    .num_threads(num_cores.get())
                    .build()
                    .unwrap(),
            ),
        };

        // let possible_orders_except_one: &Arc<[PossibleOrder<N>]> =
        //     self.possible_orders_except_one.get_or_try_init(|| {
        let possible_orders = self
            .puzzle_def
            .possible_orders(maybe_pool.as_ref())
            .ok_or(CycleCombinationFinderError::PuzzleTooManyOrders)?;
        let possible_orders_except_one =
            mk_possible_orders_except_one(self.puzzle_def, possible_orders);
        let possible_orders_except_one = Arc::from(possible_orders_except_one.into_boxed_slice());

        let all_possible_registers = if self.optimality == Optimality::EQUIVALENT {
            unimplemented!()
        } else {
            self.search_dfs(&possible_orders_except_one)
        };
        let expansion_percent_done = AtomicUsize::new(0);
        let logged_bucket = Mutex::new(0);
        let possible_registers_len = all_possible_registers.len();

        let expand = || {
            all_possible_registers
                .into_par_iter()
                .map_init(
                    || self.solutions_calculator(&possible_orders_except_one),
                    |solutions_calculator, possible_registers| {
                        expand_possible_register(
                            solutions_calculator,
                            possible_registers,
                            &expansion_percent_done,
                            &logged_bucket,
                            possible_registers_len,
                        )
                    },
                )
                .collect::<Box<[_]>>()
        };

        let now = Instant::now();
        let mut cycle_combinations = match maybe_pool {
            Some(pool) => pool.install(expand),
            None => expand(),
        };
        info!("Expansion took {}", now.elapsed().human(Truncate::Micro));
        debug!(
            "Found {} solutions, with {} expansions average",
            cycle_combinations.len(),
            cycle_combinations
                .iter()
                .map(|cycle_combination| cycle_combination.solutions.0.len())
                .sum::<usize>()
        );
        if self.sorted {
            cycle_combinations.sort_unstable();
        }
        let actual_solution_count = cycle_combinations.len();
        let cycle_combinations = CycleCombinations {
            cycle_combinations,
            possible_orders_except_one,
        };
        if let Some(expected_solution_count) = self.maybe_expected_solution_count
            && actual_solution_count != expected_solution_count
        {
            Err(CycleCombinationFinderError::MismatchedSolutionCount {
                cycle_combinations,
                expected: expected_solution_count,
                actual: actual_solution_count,
            })
        } else {
            Ok(cycle_combinations)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        finder::{
            CycleCombinationFinder, CycleCombinations, NumCores, Optimality, SolutionExpansion,
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
    fn minx3_optimal_2() {
        let minx3 = MINX3.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx3)
            .with_mss_batch_size(Some(10))
            .with_register_count(2)
            .validate()
            .unwrap()
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
    fn minx3_optimal_3() {
        let minx3 = MINX3.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx3)
            .with_optimality(Optimality::MaxOrderRatio(1.01))
            .with_num_cores(NumCores::Num(1))
            .with_register_count(3)
            .validate()
            .unwrap()
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
            .with_register_count(4)
            .validate()
            .unwrap()
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!("{}", x.orders_fmt(&ret.possible_orders_except_one));
        }
    }

    #[test_log::test]
    fn minx3_optimal_5() {
        let minx3 = MINX3.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx3)
            .with_register_count(5)
            .with_optimality(Optimality::MaxOrderRatio(10.0))
            .validate()
            .unwrap()
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
            .with_register_count(6)
            .with_optimality(Optimality::MaxOrderRatio(10.0))
            .validate()
            .unwrap()
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
    fn minx4_optimal_2() {
        let minx4 = MINX4.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx4)
            .with_register_count(2)
            .with_mss_batch_size(Some(1))
            .validate()
            .unwrap()
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!("{}", x.orders_fmt(&ret.possible_orders_except_one));
        }
    }

    #[test_log::test]
    fn minx4_optimal_3() {
        let minx4 = MINX4.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx4)
            .with_register_count(3)
            .with_mss_batch_size(Some(1000))
            .with_expected_solutions_count_assertion(Some(296))
            // .with_time_limit(None)
            // .with_optimality(Optimality::MaxOrderRatio(5.0))
            .validate()
            .unwrap()
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
            .with_register_count(4)
            .with_optimality(Optimality::MaxOrderRatio(10.0))
            .validate()
            .unwrap()
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
            .with_register_count(5)
            .with_optimality(Optimality::MaxOrderRatio(10.0))
            .validate()
            .unwrap()
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
            .with_register_count(2)
            .with_mss_batch_size(Some(1000))
            .with_expected_solutions_count_assertion(Some(33))
            .validate()
            .unwrap()
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
            .with_register_count(3)
            // .with_max_fitting_tries(Some(500))
            // .with_optimality(Optimality::MaxOrderRatio(1.0))
            .with_solution_expansion(SolutionExpansion::Limit(10))
            .with_mss_batch_size(Some(100))
            .validate()
            .unwrap()
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!("{}", x.orders_fmt(&ret.possible_orders_except_one));
        }
    }

    #[test_log::test]
    fn cube3_optimal_4() {
        let cube3 = CUBE3.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&cube3)
            .with_register_count(4)
            .with_expected_solutions_count_assertion(Some(43))
            .with_num_cores(NumCores::Num(1))
            .validate()
            .unwrap()
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!("{}", x.orders_fmt(&ret.possible_orders_except_one));
        }
    }

    #[test_log::test]
    fn cube3_optimal_3() {
        let cube3 = CUBE3.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&cube3)
            .with_register_count(3)
            .with_expected_solutions_count_assertion(Some(19))
            .validate()
            .unwrap()
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
    fn cube3_optimal_2() {
        let cube3 = CUBE3.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&cube3)
            .with_register_count(2)
            .with_expected_solutions_count_assertion(Some(7))
            .validate()
            .unwrap()
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!("{}", x.orders_fmt(&ret.possible_orders_except_one));
        }
    }

    #[test_log::test]
    fn cube4_optimal_2() {
        let cube4 = CUBE4.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&cube4)
            .with_register_count(2)
            .validate()
            .unwrap()
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

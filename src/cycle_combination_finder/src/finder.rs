use std::{
    cmp::Ordering,
    fmt::{self, Write},
    num::{NonZeroU16, NonZeroUsize},
    sync::{Arc, atomic::AtomicUsize, nonpoison::Mutex},
    time::{Duration, Instant},
};

use fxhash::FxHashMap;
use humanize_duration::{Truncate, prelude::DurationExt};
use log::{debug, info, trace};
use puzzle_theory::numbers::{Int, U};
use rayon::{
    ThreadPool,
    iter::{IntoParallelIterator, ParallelIterator},
};
use thiserror::Error;

use crate::{
    cycle_combination_solutions::{
        CycleCombinationSolution, CycleCombinationSolutions, expand_possible_registers,
    },
    min_piece_count::MinPieceCount,
    orderexps::OrderExps,
    puzzle::{
        PuzzleDef, orbit_index_cast, possible_order_index_cast, possible_orders_len_cast,
        register_index_cast,
    },
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

    fn is_valid(self) -> bool {
        let is_valid_ratio = |ratio: f64| ratio.is_finite() && ratio >= 1.0;
        let (maybe_min_order_ratio, maybe_max_order_ratio) = self.maybe_min_max_order_ratio();
        maybe_min_order_ratio.is_none_or(is_valid_ratio)
            && maybe_max_order_ratio.is_none_or(is_valid_ratio)
            && !matches!(
                (maybe_min_order_ratio, maybe_max_order_ratio),
                (Some(min), Some(max)) if min > max
            )
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

    fn validate(self) -> Option<ValidatedNumCores> {
        match self {
            NumCores::AllCores => Some(ValidatedNumCores::AllCores),
            NumCores::Num(num) => NonZeroUsize::new(num)
                .filter(|num| num.get() <= num_cpus::get())
                .map(ValidatedNumCores::Num),
        }
    }
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

    fn validate(self) -> Option<ValidatedSolutionExpansion> {
        match self {
            SolutionExpansion::All => Some(ValidatedSolutionExpansion::All),
            SolutionExpansion::Limit(limit) => {
                NonZeroUsize::new(limit).map(ValidatedSolutionExpansion::Limit)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum OrderValueConstraint {
    #[default]
    None,
    AtLeast(Int<U>),
    Exactly(Int<U>),
}

#[derive(Clone, Debug)]
pub enum ValidatedOrderValueConstraint {
    AtLeast(u32),
    Exactly(u32),
}

#[derive(Debug, Clone)]
pub struct PossibleOrder<const N: usize> {
    pub(crate) order: OrderExps<N>,
    pub(crate) min_piece_count: NonZeroU16,
    pub(crate) min_piece_count_naive: u16,
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
    #[error("Expected {expected} solutions, found {actual}")]
    MismatchedSolutionCount { expected: usize, actual: usize },
}

#[derive(Error, Debug)]
pub enum PuzzleAnalysisError {
    #[error(
        "This puzzle has too many orders. This is a hint that your puzzle is anyways too large \
         for the CCF to finish computing in a reasonable amount of time"
    )]
    PuzzleTooManyOrders,
    #[error(
        "Number of cores must be non-zero and less than or equal to the number of cores on this \
         machine."
    )]
    InvalidNumCores,
}

#[derive(Error, Debug)]
pub enum CycleCombinationFinderBuildError {
    #[error("Register order {order_value} does not exist; try one of: {try_}")]
    OrderValueDoesNotExist {
        order_value: Int<U>,
        try_: PossibleOrdersDisplayList,
    },
    #[error("Register count must be non-zero")]
    InvalidRegisterCount,
    #[error(
        "Number of cores must be non-zero and less than or equal to the number of cores on this \
         machine"
    )]
    InvalidNumCores,
    #[error("MSS batch size must be non-zero")]
    InvalidMssBatchSize,
    #[error("Solution expansion limit must be non-zero")]
    InvalidSolutionExpansion,
    #[error(
        "Optimality order ratios must be finite and >= 1.0, and a clamp's min must not exceed its \
         max."
    )]
    InvalidOptimality,
}

#[derive(Debug)]
pub struct PossibleOrdersDisplayList(Vec<Int<U>>);

impl fmt::Display for PossibleOrdersDisplayList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            &self
                .0
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
                .join(", "),
        )
    }
}

#[derive(Clone, Copy, Default)]
pub struct Register {
    order_value_constraint: OrderValueConstraint,
    custom_constraint: Option<fn()>,
}

/// The possible orders of a puzzle, computed once and shared by every
/// [`CycleCombinationFinder`] built from it.
#[derive(Clone, Debug)]
pub struct PuzzleAnalysis<'pd, const N: usize> {
    pub(crate) puzzle_def: &'pd PuzzleDef<N>,
    pub(crate) possible_orders_except_one: Arc<[PossibleOrder<N>]>,
}

#[derive(Clone)]
pub struct NeedsRegisters;

#[derive(Clone)]
pub struct HasRegisters(Vec<Register>);

#[derive(Clone)]
pub struct NeedsPuzzleAnalysis;

#[derive(Clone)]
pub struct HasPuzzleAnalysis<'pd, 'a, const N: usize>(&'a PuzzleAnalysis<'pd, N>);

#[derive(Clone)]
pub struct CycleCombinationFinderBuilder<R, P> {
    registers: R,
    puzzle_analysis: P,
    optimality: Optimality,
    num_cores: NumCores,
    sorted: bool,
    maybe_expected_solution_count: Option<usize>,
    maybe_max_fitting_tries: Option<u32>,
    solution_expansion: SolutionExpansion,
    maybe_mss_batch_size: Option<usize>,
    maybe_time_limit: Option<Duration>,
    fast_assumptions: bool,
    order_value_constraints: Vec<OrderValueConstraint>,
}

#[derive(Clone, Debug)]
pub struct CycleCombinationFinder<'a, const N: usize> {
    pub(crate) register_count: NonZeroU16,
    pub(crate) puzzle_def: &'a PuzzleDef<N>,
    pub(crate) possible_orders_except_one: Arc<[PossibleOrder<N>]>,
    pub(crate) optimality: Optimality,
    pub(crate) num_cores: ValidatedNumCores,
    pub(crate) sorted: bool,
    pub(crate) maybe_expected_solution_count: Option<usize>,
    pub(crate) maybe_max_fitting_tries: Option<u32>,
    pub(crate) solution_expansion: ValidatedSolutionExpansion,
    pub(crate) mss_batch_size: NonZeroUsize,
    pub(crate) maybe_time_limit: Option<Duration>,
    pub(crate) fast_assumptions: bool,
    pub(crate) order_value_constraints: Box<[ValidatedOrderValueConstraint]>,
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

impl Default for CycleCombinationFinderBuilder<NeedsRegisters, NeedsPuzzleAnalysis> {
    fn default() -> Self {
        CycleCombinationFinderBuilder {
            registers: NeedsRegisters,
            puzzle_analysis: NeedsPuzzleAnalysis,
            optimality: Optimality::Optimal,
            num_cores: NumCores::AllCores,
            sorted: true,
            maybe_expected_solution_count: None,
            maybe_max_fitting_tries: None,
            solution_expansion: SolutionExpansion::Limit(10_000),
            maybe_mss_batch_size: None,
            maybe_time_limit: None,
            fast_assumptions: true,
            order_value_constraints: vec![],
        }
    }
}

impl Register {
    #[must_use]
    pub fn default_count(c: u16) -> Vec<Self> {
        vec![Self::default(); usize::from(c)]
    }

    #[must_use]
    pub fn with_order_value_constraint(
        mut self,
        order_value_constraint: OrderValueConstraint,
    ) -> Self {
        self.order_value_constraint = order_value_constraint;
        self
    }

    #[must_use]
    pub fn with_custom_constraint(mut self, custom_constraint: Option<fn()>) -> Self {
        self.custom_constraint = custom_constraint;
        self
    }
}

impl ValidatedNumCores {
    fn maybe_pool(self) -> Option<ThreadPool> {
        match self {
            ValidatedNumCores::AllCores => None,
            ValidatedNumCores::Num(num_cores) => Some(
                rayon::ThreadPoolBuilder::new()
                    .num_threads(num_cores.get())
                    .build()
                    .unwrap(),
            ),
        }
    }
}

impl<'pd, const N: usize> PuzzleAnalysis<'pd, N> {
    /// Compute the possible orders of `puzzle_def`.
    ///
    /// # Errors
    ///
    /// Errors following the variants of `PuzzleAnalysisError`.
    #[allow(clippy::missing_panics_doc)]
    pub fn new(
        puzzle_def: &'pd PuzzleDef<N>,
        num_cores: NumCores,
    ) -> Result<Self, PuzzleAnalysisError> {
        let num_cores = num_cores
            .validate()
            .ok_or(PuzzleAnalysisError::InvalidNumCores)?;
        let maybe_pool = num_cores.maybe_pool();
        let possible_orders = puzzle_def
            .possible_orders(maybe_pool.as_ref())
            .ok_or(PuzzleAnalysisError::PuzzleTooManyOrders)?;
        assert!(
            possible_orders.remove(&OrderExps::one()).is_some(),
            "Order one exists on every group"
        );

        let now = Instant::now();
        let mut min_piece_count_calculator = MinPieceCount::from(puzzle_def);
        let mut possible_orders_except_one = possible_orders
            .into_iter()
            .map(|possible_order| {
                let (min_piece_count, min_piece_count_naive) =
                    min_piece_count_calculator.calculate(&possible_order);
                PossibleOrder {
                    order: possible_order,
                    min_piece_count,
                    min_piece_count_naive,
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

        Ok(PuzzleAnalysis {
            puzzle_def,
            possible_orders_except_one: Arc::from(possible_orders_except_one.into_boxed_slice()),
        })
    }
}

impl<R, P> CycleCombinationFinderBuilder<R, P> {
    #[must_use]
    pub fn with_sorted(mut self, sorted: bool) -> Self {
        self.sorted = sorted;
        self
    }

    #[must_use]
    pub fn with_optimality(mut self, optimality: Optimality) -> Self {
        self.optimality = optimality;
        self
    }

    #[must_use]
    pub fn with_num_cores(mut self, num_cores: NumCores) -> Self {
        self.num_cores = num_cores;
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
        self.solution_expansion = solution_expansion;
        self
    }

    #[must_use]
    pub fn with_mss_batch_size(mut self, maybe_mss_batch_size: Option<usize>) -> Self {
        self.maybe_mss_batch_size = maybe_mss_batch_size;
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
    pub fn with_registers(
        self,
        registers: Vec<Register>,
    ) -> CycleCombinationFinderBuilder<HasRegisters, P> {
        CycleCombinationFinderBuilder {
            registers: HasRegisters(registers),
            puzzle_analysis: self.puzzle_analysis,
            optimality: self.optimality,
            num_cores: self.num_cores,
            sorted: self.sorted,
            maybe_expected_solution_count: self.maybe_expected_solution_count,
            maybe_max_fitting_tries: self.maybe_max_fitting_tries,
            solution_expansion: self.solution_expansion,
            maybe_mss_batch_size: self.maybe_mss_batch_size,
            maybe_time_limit: self.maybe_time_limit,
            fast_assumptions: self.fast_assumptions,
            order_value_constraints: self.order_value_constraints,
        }
    }

    #[must_use]
    pub fn with_puzzle_analysis<'pd, 'a, const N: usize>(
        self,
        puzzle_analysis: &'a PuzzleAnalysis<'pd, N>,
    ) -> CycleCombinationFinderBuilder<R, HasPuzzleAnalysis<'pd, 'a, N>> {
        CycleCombinationFinderBuilder {
            registers: self.registers,
            puzzle_analysis: HasPuzzleAnalysis(puzzle_analysis),
            optimality: self.optimality,
            num_cores: self.num_cores,
            sorted: self.sorted,
            maybe_expected_solution_count: self.maybe_expected_solution_count,
            maybe_max_fitting_tries: self.maybe_max_fitting_tries,
            solution_expansion: self.solution_expansion,
            maybe_mss_batch_size: self.maybe_mss_batch_size,
            maybe_time_limit: self.maybe_time_limit,
            fast_assumptions: self.fast_assumptions,
            order_value_constraints: self.order_value_constraints,
        }
    }
}

impl<'a, const N: usize> CycleCombinationFinderBuilder<HasRegisters, HasPuzzleAnalysis<'a, '_, N>> {
    /// Validate the builder.
    ///
    /// # Errors
    ///
    /// Errors following the variants of `CycleCombinationFinderBuildError`.
    pub fn build(self) -> Result<CycleCombinationFinder<'a, N>, CycleCombinationFinderBuildError> {
        let CycleCombinationFinderBuilder {
            registers: HasRegisters(registers),
            puzzle_analysis: HasPuzzleAnalysis(puzzle_analysis),
            optimality,
            num_cores,
            sorted,
            maybe_expected_solution_count,
            maybe_max_fitting_tries,
            solution_expansion,
            maybe_mss_batch_size,
            maybe_time_limit,
            fast_assumptions,
            order_value_constraints,
        } = self;
        let register_count = NonZeroU16::new(register_index_cast(registers.len()))
            .ok_or(CycleCombinationFinderBuildError::InvalidRegisterCount)?;
        if !optimality.is_valid() {
            return Err(CycleCombinationFinderBuildError::InvalidOptimality);
        }
        let num_cores = num_cores
            .validate()
            .ok_or(CycleCombinationFinderBuildError::InvalidNumCores)?;
        let solution_expansion = solution_expansion
            .validate()
            .ok_or(CycleCombinationFinderBuildError::InvalidSolutionExpansion)?;
        let mss_batch_size = NonZeroUsize::new(maybe_mss_batch_size.unwrap_or(1000))
            .ok_or(CycleCombinationFinderBuildError::InvalidMssBatchSize)?;
        let puzzle_def = puzzle_analysis.puzzle_def;
        let possible_orders_except_one = Arc::clone(&puzzle_analysis.possible_orders_except_one);
        let mut possible_order_to_bigint = FxHashMap::default();
        let order_value_constraints = order_value_constraints
            .into_iter()
            .filter_map(|order_value_constraint| {
                Some(Ok(match order_value_constraint {
                    OrderValueConstraint::None => {
                        return None;
                    }
                    OrderValueConstraint::AtLeast(order_value_bigint) => {
                        let i = possible_orders_except_one.partition_point(|possible_order| {
                            let possible_order_bigint = *possible_order_to_bigint
                                .entry(possible_order.order.clone())
                                .or_insert_with_key(OrderExps::as_bigint);
                            possible_order_bigint <= order_value_bigint
                        });
                        ValidatedOrderValueConstraint::AtLeast(possible_orders_len_cast(i))
                    }
                    OrderValueConstraint::Exactly(order_value_bigint) => {
                        let maybe_i =
                            possible_orders_except_one.binary_search_by(|possible_order| {
                                let possible_order_bigint = possible_order_to_bigint
                                    .entry(possible_order.order.clone())
                                    .or_insert_with_key(OrderExps::as_bigint);
                                (*possible_order_bigint).cmp(&order_value_bigint)
                            });
                        match maybe_i {
                            Ok(i) => ValidatedOrderValueConstraint::Exactly(possible_orders_len_cast(i)),
                            Err(right_i) => {
                                let mut try_ = vec![];
                                if let Some(left) = right_i.checked_sub(1)
                                    && let Some(left_possible_order) =
                                        possible_orders_except_one.get(left)
                                {
                                    let possible_order_bigint = possible_order_to_bigint
                                        .entry(left_possible_order.order.clone())
                                        .or_insert_with_key(OrderExps::as_bigint);
                                    try_.push(*possible_order_bigint);
                                }
                                if let Some(right_possible_order) =
                                    possible_orders_except_one.get(right_i)
                                {
                                    let possible_order_bigint = possible_order_to_bigint
                                        .entry(right_possible_order.order.clone())
                                        .or_insert_with_key(OrderExps::as_bigint);
                                    try_.push(*possible_order_bigint);
                                }
                                return Some(Err(CycleCombinationFinderBuildError::OrderValueDoesNotExist {
                                    order_value: order_value_bigint,
                                    try_: PossibleOrdersDisplayList(try_),
                                }));
                            }
                        }
                    }
                }))
            })
            .collect::<Result<Box<[_]>, _>>()?;
        Ok(CycleCombinationFinder {
            register_count,
            puzzle_def,
            possible_orders_except_one,
            optimality,
            num_cores,
            sorted,
            maybe_expected_solution_count,
            maybe_max_fitting_tries,
            solution_expansion,
            mss_batch_size,
            maybe_time_limit,
            fast_assumptions,
            order_value_constraints,
        })
    }
}

impl<const N: usize> CycleCombinationFinder<'_, N> {
    /// Search for CCF solutions in parallel.
    ///
    /// # Errors
    ///
    /// Errors if an expected solution count was set via
    /// [`CycleCombinationFinderBuilder::with_expected_solutions_count_assertion`]
    /// and the solution count mismatches.
    ///
    /// # Panics
    ///
    /// Panics if the optimality is [`Optimality::EQUIVALENT`], which is
    /// unimplemented.
    pub fn find(self) -> Result<CycleCombinations<N>, CycleCombinationFinderError<N>> {
        // Make it such that every index corresponds to a valid possible order so we can make this
        // assumption (an AtLeast constraint will be `self.possible_orders_except_one.len()` when
        // set to a number larger than the maximum order).
        for order_value_constraint in &self.order_value_constraints {
            if let &ValidatedOrderValueConstraint::AtLeast(c) = order_value_constraint
                && c == possible_orders_len_cast(self.possible_orders_except_one.len())
            {
                return Ok(CycleCombinations {
                    cycle_combinations: vec![].into_boxed_slice(),
                    possible_orders_except_one: self.possible_orders_except_one,
                });
            }
        }

        let maybe_pool = self.num_cores.maybe_pool();
        let all_possible_registers = if self.optimality == Optimality::EQUIVALENT {
            unimplemented!()
        } else {
            self.search_dfs()
        };
        let expansion_percent_done = AtomicUsize::new(0);
        let logged_bucket = Mutex::new(0);
        let possible_registers_len = all_possible_registers.len();

        let expand = || {
            all_possible_registers
                .into_par_iter()
                .map_init(
                    || self.solutions_calculator(),
                    |solutions_calculator, possible_registers| {
                        expand_possible_registers(
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
            possible_orders_except_one: Arc::clone(&self.possible_orders_except_one),
        };
        if let Some(expected_solution_count) = self.maybe_expected_solution_count
            && actual_solution_count != expected_solution_count
        {
            Err(CycleCombinationFinderError::MismatchedSolutionCount {
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
            CycleCombinationFinderBuilder, CycleCombinations, NumCores, Optimality, PuzzleAnalysis,
            Register, SolutionExpansion,
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
        let analysis = PuzzleAnalysis::new(&minx3, NumCores::AllCores).unwrap();
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(2))
            .with_mss_batch_size(Some(10))
            .build()
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
        let analysis = PuzzleAnalysis::new(&minx3, NumCores::AllCores).unwrap();
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(3))
            .build()
            .unwrap()
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!(
                "{}",
                // x.orders_fmt(&ret.possible_orders_except_one)
                x.solutions_fmt(&ret.possible_orders_except_one, &minx3)
            );
        }
    }

    #[test_log::test]
    fn minx3_optimal_4() {
        let minx3 = MINX3.clone();
        let analysis = PuzzleAnalysis::new(&minx3, NumCores::AllCores).unwrap();
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(4))
            .build()
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
        let analysis = PuzzleAnalysis::new(&minx3, NumCores::AllCores).unwrap();
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(5))
            .with_optimality(Optimality::MaxOrderRatio(10.0))
            .build()
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
        let analysis = PuzzleAnalysis::new(&minx3, NumCores::AllCores).unwrap();
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(6))
            .with_optimality(Optimality::MaxOrderRatio(10.0))
            .build()
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
        let analysis = PuzzleAnalysis::new(&minx4, NumCores::AllCores).unwrap();
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(2))
            .with_mss_batch_size(Some(1))
            .build()
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
        let analysis = PuzzleAnalysis::new(&minx4, NumCores::AllCores).unwrap();
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(3))
            .with_mss_batch_size(Some(10000))
            // .with_expected_solutions_count_assertion(Some(296))
            // .with_time_limit(None)
            // .with_optimality(Optimality::MaxOrderRatio(5.0))
            .build()
            .unwrap()
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!(
                "{}",
                // x.solutions_fmt(&ret.possible_orders_except_one, &minx4)
                x.orders_fmt(&ret.possible_orders_except_one)
            );
        }
    }

    #[test_log::test]
    fn minx4_optimal_4() {
        let minx4 = MINX4.clone();
        let analysis = PuzzleAnalysis::new(&minx4, NumCores::AllCores).unwrap();
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(4))
            .with_optimality(Optimality::MaxOrderRatio(10.0))
            .build()
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
        let analysis = PuzzleAnalysis::new(&minx4, NumCores::AllCores).unwrap();
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(5))
            .with_optimality(Optimality::MaxOrderRatio(10.0))
            .build()
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
        let analysis = PuzzleAnalysis::new(&minx5, NumCores::AllCores).unwrap();
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(2))
            .with_mss_batch_size(Some(1000))
            .with_expected_solutions_count_assertion(Some(33))
            .build()
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
        let analysis = PuzzleAnalysis::new(&minx5, NumCores::AllCores).unwrap();
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(3))
            // .with_max_fitting_tries(Some(500))
            // .with_optimality(Optimality::MaxOrderRatio(1.0))
            .with_solution_expansion(SolutionExpansion::Limit(10))
            .with_mss_batch_size(Some(100))
            .build()
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
        let analysis = PuzzleAnalysis::new(&cube3, NumCores::AllCores).unwrap();
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(4))
            .with_expected_solutions_count_assertion(Some(43))
            .build()
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
        let analysis = PuzzleAnalysis::new(&cube3, NumCores::AllCores).unwrap();
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(3))
            // .with_expected_solutions_count_assertion(Some(18))
            .build()
            .unwrap()
            .find()
            .unwrap();

        for x in ret.cycle_combinations {
            println!(
                "{}",
                // x.solutions_fmt(&ret.possible_orders_except_one, &cube3)
                x.orders_fmt(&ret.possible_orders_except_one)
            );
        }
    }

    #[test_log::test]
    fn cube3_optimal_2() {
        let cube3 = CUBE3.clone();
        let analysis = PuzzleAnalysis::new(&cube3, NumCores::AllCores).unwrap();
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(2))
            .with_expected_solutions_count_assertion(Some(7))
            .build()
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
        let analysis = PuzzleAnalysis::new(&cube4, NumCores::AllCores).unwrap();
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(2))
            .build()
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

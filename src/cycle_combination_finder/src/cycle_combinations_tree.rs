use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    fmt::{self, Debug, Display},
    num::{NonZeroU16, NonZeroU32, NonZeroUsize},
    ptr::NonNull,
    sync::{
        Arc,
        atomic::{self, AtomicBool, AtomicPtr},
        mpmc,
        mpsc::{RecvError, TryRecvError},
        nonpoison::Mutex,
    },
    time::{Duration, Instant},
};

use core_affinity::CoreId;
use cpu_time::ThreadTime;
use humanize_duration::{Truncate, prelude::DurationExt};
use log::{Level, debug, info, log_enabled, trace};
use tokio::sync::broadcast::error::TryRecvError as TokioTryRecvError;

use crate::{
    finder::{PossibleOrder, ValidatedCycleCombinationFinder, ValidatedNumCores},
    nonemptyvec::{NonemptySlice, NonemptyVec},
    pareto_front::CCParetoFront,
    puzzle::possible_orders_len_cast,
};

#[derive(Clone)]
struct CycleCombinationsTreeShard<'a> {
    fails: u64,
    batch_packed_queue: Vec<u32>,
    sends: u64,
    empty_sends: u64,
    full_sends: u64,
    sender_lens: usize,
    curr_batch_len: usize,
    registers: NonemptyVec<u32>,
    lower_index_cutoff: u32,
    candidates_count: u64,

    candidates_sender: mpmc::Sender<PackedCycleCombinationCandidateQueue>,
    candidates_sender_capacity: usize,
    batch_size: NonZeroUsize,
    pareto_efficient_prunings: &'a AtomicPtr<u32>,
}

#[derive(Debug, Clone)]
struct PackedCycleCombinationCandidateQueue(Box<[u32]>);

#[derive(Debug, Clone, Copy)]
pub struct DisjointRegisters<'a> {
    prefix_registers: &'a [u32],
    last_register: u32,
}

struct SolutionsThreadInfo {
    real_time: Duration,
    cpu_time: Duration,
    processed_candidate_count: u64,
    post_candidate_count: u64,
    cycle_combinations: CCParetoFront,
}

#[derive(Default, Clone)]
struct TreeThreadInfo {
    real_time: Duration,
    cpu_time: Duration,
    candidate_count: u64,
    empty_sends: u64,
    full_sends: u64,
    sends: u64,
    sender_lens: usize,
}

struct TreeProfileInfo {
    candidate_count: u64,
    processed_candidate_count: u64,
    post_candidate_count: u64,
    pruned_orders_percentage: f64,
    sender_len_percentage: f64,
    empty_sends_percentage: f64,
    full_sends_percentage: f64,
    real_time: Duration,
    dfs_cpu_time: Duration,
    dfs_io_time: Duration,
    solutions_cpu_time: Duration,
    solutions_io_time: Duration,
    num_cores: usize,
}

impl Display for TreeProfileInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[allow(clippy::cast_precision_loss)]
        let num_cores = self.num_cores as f64;
        let cpu_time = self.real_time.mul_f64(num_cores);
        f.debug_struct("TreeProfileInfo")
            .field(&format!("{:>25}", "candidate_count"), &self.candidate_count)
            .field(
                &format!("{:>25}", "processed_candidate_count"),
                &self.processed_candidate_count,
            )
            .field(
                &format!("{:>25}", "post_candidate_count"),
                &format_args!(
                    "{} ({} / thread)",
                    self.post_candidate_count,
                    self.post_candidate_count / u64::try_from(self.num_cores).unwrap(),
                ),
            )
            .field(
                &format!("{:>25}", "pruned_orders_percentage"),
                &format_args!("{:05.2}%", self.pruned_orders_percentage * 100.0),
            )
            .field(
                &format!("{:>25}", "sender_len_percentage"),
                &format_args!("{:05.2}%", self.sender_len_percentage * 100.0),
            )
            .field(
                &format!("{:>25}", "empty_sends_percentage"),
                &format_args!("{:05.2}%", self.empty_sends_percentage * 100.0),
            )
            .field(
                &format!("{:>25}", "full_sends_percentage"),
                &format_args!("{:05.2}%", self.full_sends_percentage * 100.0),
            )
            .field(
                &format!("{:>25}", "real_time"),
                &format_args!("{}", self.real_time.human(Truncate::Millis)),
            )
            .field(
                &format!("{:>25}", "single_cpu_time"),
                &format_args!(
                    "{}",
                    (self.dfs_cpu_time + self.solutions_cpu_time)
                        .div_f64(num_cores)
                        .human(Truncate::Millis)
                ),
            )
            .field(
                &format!("{:>25}", "dfs_cpu_time"),
                &format_args!(
                    "{:05.2}% ({})",
                    self.dfs_cpu_time.div_duration_f64(cpu_time) * 100.0,
                    self.dfs_cpu_time.div_f64(num_cores).human(Truncate::Millis)
                ),
            )
            .field(
                &format!("{:>25}", "dfs_io_time"),
                &format_args!(
                    "{:05.2}% ({})",
                    self.dfs_io_time.div_duration_f64(cpu_time) * 100.0,
                    self.dfs_io_time.div_f64(num_cores).human(Truncate::Millis)
                ),
            )
            .field(
                &format!("{:>25}", "solutions_cpu_time"),
                &format_args!(
                    "{:05.2}% ({})",
                    self.solutions_cpu_time.div_duration_f64(cpu_time) * 100.0,
                    self.solutions_cpu_time
                        .div_f64(num_cores)
                        .human(Truncate::Millis)
                ),
            )
            .field(
                &format!("{:>25}", "solutions_io_time"),
                &format_args!(
                    "{:05.2}% ({})",
                    self.solutions_io_time.div_duration_f64(cpu_time) * 100.0,
                    self.solutions_io_time
                        .div_f64(num_cores)
                        .human(Truncate::Millis)
                ),
            )
            .field(&format!("{:>25}", "num_cores"), &self.num_cores)
            .finish()
    }
}

#[must_use]
pub fn dbg_registers<const N: usize>(
    registers: impl IntoIterator<Item = u32>,
    possible_orders: &[PossibleOrder<N>],
) -> String {
    registers
        .into_iter()
        .map(|x| possible_orders[x as usize].order.as_bigint().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[must_use]
pub fn dbg_registers_iter<const N: usize>(
    registers_iter: impl IntoIterator<Item = impl IntoIterator<Item = u32>>,
    possible_orders: &[PossibleOrder<N>],
) -> String {
    registers_iter
        .into_iter()
        .map(|registers| dbg_registers(registers, possible_orders))
        .collect::<Vec<_>>()
        .join("\n")
}

impl DisjointRegisters<'_> {
    pub fn iter(self) -> impl Iterator<Item = u32> {
        self.prefix_registers
            .iter()
            .copied()
            .chain(std::iter::once(self.last_register))
    }

    #[must_use]
    pub fn get(self, i: u16) -> Option<u32> {
        let i = usize::from(i);
        if i == self.prefix_registers.len() {
            Some(self.last_register)
        } else {
            self.prefix_registers.get(i).copied()
        }
    }

    pub fn iter_orders<const N: usize>(
        self,
        possible_orders_except_one: &[PossibleOrder<N>],
    ) -> impl Iterator<Item = &PossibleOrder<N>> {
        self.iter().map(|i| &possible_orders_except_one[i as usize])
    }

    #[must_use]
    pub fn get_order<const N: usize>(
        self,
        i: u16,
        possible_orders_except_one: &[PossibleOrder<N>],
    ) -> Option<&PossibleOrder<N>> {
        self.get(i).map(|i| &possible_orders_except_one[i as usize])
    }
}

impl<'a> From<NonemptySlice<'a, u32>> for DisjointRegisters<'a> {
    fn from(value: NonemptySlice<'a, u32>) -> Self {
        let (&last_register, prefix_registers) = value.split_last();
        DisjointRegisters {
            prefix_registers,
            last_register,
        }
    }
}

/// # Safety
///
/// `pareto_efficient_pruning` must come from the `try_update` method on one of
/// `pareto_efficient_prunings`
unsafe fn try_next_pareto_efficient_prunings(
    maybe_raw_prunings: *mut u32,
    disjoint_registers: DisjointRegisters,
    raw_pruning_len: NonZeroUsize,
) -> Option<NonNull<u32>> {
    if let Some(raw_prunings) = NonNull::new(maybe_raw_prunings) {
        // SAFETY: the called guarantees `pareto_efficient_pruning` is valid. Also later
        // in this block we always initialize `pareto_efficient_pruning` to be of
        // `raw_pruning_len` length.
        let raw_prunings = unsafe {
            NonemptySlice::from_raw_parts(
                NonNull::slice_from_raw_parts(raw_prunings, raw_pruning_len.get())
                    .as_uninit_slice()
                    .assume_init_ref()
                    .as_ptr(),
                raw_pruning_len,
            )
        };
        let (&max_last_register, pareto_efficent_prunes) = raw_prunings.split_first();
        if disjoint_registers.last_register < max_last_register {
            return None;
        }
        if disjoint_registers.last_register == max_last_register {
            let mut maybe_next_pareto_efficient_prunings: Option<Vec<u32>> = None;
            for ((i, &prefix_register), &pareto_efficient_prune) in disjoint_registers
                .prefix_registers
                .iter()
                .enumerate()
                .zip(pareto_efficent_prunes)
            {
                match &mut maybe_next_pareto_efficient_prunings {
                    Some(next_pareto_efficient_pruning) => {
                        next_pareto_efficient_pruning.push(prefix_register);
                    }
                    None => match prefix_register.cmp(&pareto_efficient_prune) {
                        Ordering::Less => return None,
                        Ordering::Equal => (),
                        Ordering::Greater => {
                            let mut next_pareto_efficient_prunings =
                                Vec::with_capacity(raw_pruning_len.get());
                            next_pareto_efficient_prunings.extend(
                                std::iter::once(disjoint_registers.last_register).chain(
                                    disjoint_registers
                                        .prefix_registers
                                        .iter()
                                        .copied()
                                        .take(i + 1),
                                ),
                            );
                            maybe_next_pareto_efficient_prunings =
                                Some(next_pareto_efficient_prunings);
                        }
                    },
                }
            }

            // new can still be None here:
            // A C D can be a solution, followed by B C D
            return maybe_next_pareto_efficient_prunings.map(|next_pareto_efficient_prunings| {
                debug_assert_eq!(next_pareto_efficient_prunings.len(), raw_pruning_len.get());
                Box::into_non_null(next_pareto_efficient_prunings.into_boxed_slice()).cast()
            });
        }
    }
    Some(
        Box::into_non_null(
            std::iter::once(disjoint_registers.last_register)
                .chain(disjoint_registers.prefix_registers.iter().copied())
                .collect::<Box<_>>(),
        )
        .cast(),
    )
}

impl CycleCombinationsTreeShard<'_> {
    fn exact_register_count(&self) -> NonZeroU16 {
        // Cast truncation is fine because `self.registers` is the length of the number
        // of registers, which is a `NonZeroU16`
        #[allow(clippy::cast_possible_truncation)]
        // SAFETY: `self.registers.len()` is not zero
        unsafe {
            NonZeroU16::new_unchecked(self.registers.len().get() as u16)
        }
    }

    fn maybe_send_queue(&mut self, force: bool) {
        self.curr_batch_len += 1;
        if self.curr_batch_len < self.batch_size.get() && !force {
            return;
        }
        if log_enabled!(Level::Debug) {
            let candidate_count = self
                .batch_packed_queue
                .iter()
                .take(self.batch_size.get())
                .map(|&candidate_count| u64::from(candidate_count))
                .sum::<u64>();
            self.candidates_count += candidate_count;
            let payload =
                PackedCycleCombinationCandidateQueue(Box::clone_from_ref(&self.batch_packed_queue));

            let len = self.candidates_sender.len();
            trace!(
                "{:?}: candidates={candidate_count}; mpmc={len}; fails={}",
                std::thread::current().id(),
                self.fails,
            );
            if len == self.candidates_sender_capacity {
                self.full_sends += 1;
            }
            if len == 0 {
                self.empty_sends += 1;
            }
            self.sender_lens += len;
            self.sends += 1;
            self.fails = 0;
            // We can unwrap because the senders is only dropped after all threads are
            // joined.
            self.candidates_sender.send(payload).unwrap();
        } else {
            // We can unwrap because the senders is only dropped after all threads are
            // joined.
            self.candidates_sender
                .send(PackedCycleCombinationCandidateQueue(Box::clone_from_ref(
                    &self.batch_packed_queue,
                )))
                .unwrap();
        }
        self.curr_batch_len = 0;
        self.batch_packed_queue.truncate(self.batch_size.get());
        for candidate_count in &mut self.batch_packed_queue {
            *candidate_count = 0;
        }
    }

    /// # Safety
    ///
    /// `register_index` must be less than `self.exact_register_count()`.
    unsafe fn search_dfs_recur<const N: usize>(
        &mut self,
        possible_orders_except_one: NonemptySlice<'_, PossibleOrder<N>>,
        register_index: NonZeroU16,
        remaining_piece_count: NonZeroU32,
    ) {
        let mut curr_possible_orders_except_one = possible_orders_except_one;
        // It should never overflow, and I don't want a panic path, so use saturating
        // logic
        let next_register_index = register_index.saturating_add(1);
        let mut candidate_count = 0;
        loop {
            let (possible_order, next_possible_orders_except_one) =
                curr_possible_orders_except_one.split_last();
            let i = possible_orders_len_cast(next_possible_orders_except_one.len());
            // TODO: inline this in previous call
            if i < self.lower_index_cutoff {
                break;
            }

            let maybe_raw_prunings = self
                .pareto_efficient_prunings
                .load(atomic::Ordering::Acquire);
            if let Some(raw_pruning) = NonNull::new(maybe_raw_prunings) {
                // SAFETY: `raw_pruning` is guaranteed to point to
                // `self.exact_register_count().get().saturating_sub(1) + 1` u32s. The caller
                // guarantees `register_index` is less than `self.exact_register_count()`;
                // therefore we are in bounds
                let raw_prunings = unsafe {
                    NonemptySlice::from_raw_parts(
                        NonNull::slice_from_raw_parts(
                            raw_pruning,
                            usize::from(next_register_index.get()),
                        )
                        .as_uninit_slice()
                        .assume_init_ref()
                        .as_ptr(),
                        NonZeroUsize::from(next_register_index),
                    )
                };
                let (&max_last_register_order, pareto_efficent_prunes) = raw_prunings.split_first();
                if i <= max_last_register_order
                    && self.registers.iter().zip(pareto_efficent_prunes).all(
                        |(&register, &pareto_efficient_prune)| register <= pareto_efficient_prune,
                    )
                {
                    break;
                }
            }

            if let Some(next_remaining_piece_count) = remaining_piece_count
                .get()
                .checked_sub(u32::from(possible_order.min_piece_count.get()))
            {
                if next_register_index == self.exact_register_count() {
                    if candidate_count == 0 {
                        self.batch_packed_queue
                            .extend(self.registers.split_last().1.iter().copied());
                    }
                    candidate_count += 1;
                    self.batch_packed_queue.push(i);
                } else if let Some(next_remaining_piece_count) =
                    NonZeroU32::new(next_remaining_piece_count)
                {
                    // SAFETY: caller guarantees `register_index < self.exact_register_count()`,
                    // therefore we are in bounds
                    let old = std::mem::replace(
                        unsafe {
                            self.registers
                                .get_unchecked_mut(usize::from(register_index.get()))
                        },
                        i,
                    );
                    // SAFETY: `next_register_index != self.exact_register_count()` in this
                    // branch, and caller guarantees we are less
                    unsafe {
                        self.search_dfs_recur(
                            curr_possible_orders_except_one,
                            next_register_index,
                            next_remaining_piece_count,
                        );
                    }
                    // SAFETY: caller guarantees `register_index < self.exact_register_count()`,
                    // therefore we are in bounds
                    unsafe {
                        *self
                            .registers
                            .get_unchecked_mut(usize::from(register_index.get())) = old;
                    }
                }
            }
            match NonemptySlice::try_from(next_possible_orders_except_one) {
                Ok(next_possible_orders) => {
                    curr_possible_orders_except_one = next_possible_orders;
                }
                Err(()) => {
                    break;
                }
            }
        }
        if next_register_index == self.exact_register_count() {
            if candidate_count != 0 {
                self.batch_packed_queue[self.curr_batch_len] = candidate_count;
                self.maybe_send_queue(false);
            } else if log_enabled!(Level::Debug) {
                self.fails += 1;
            }
        }
    }
}

impl<const N: usize> ValidatedCycleCombinationFinder<'_, N> {
    fn solutions_thread(
        &self,
        core_id: CoreId,
        candidates_receiver: mpmc::Receiver<PackedCycleCombinationCandidateQueue>,
        mut solutions_receiver: tokio::sync::broadcast::Receiver<(CoreId, Arc<[u32]>)>,
        solutions_sender: tokio::sync::broadcast::Sender<(CoreId, Arc<[u32]>)>,
        pareto_efficient_prunings: &AtomicPtr<u32>,
        possible_orders_except_one: &[PossibleOrder<N>],
    ) -> SolutionsThreadInfo {
        if core_affinity::set_for_current(core_id) {
            debug!("Solutions: Pinned {core_id:?}");
        }
        let mut cycle_combinations = CCParetoFront::default();
        let mut solutions_calculator = self.solutions_calculator(possible_orders_except_one);

        let mut processed_candidate_count = 0;
        let mut post_candidate_count = 0;
        let raw_pruning_len =
            NonZeroUsize::new(usize::from(self.register_count.get().saturating_sub(1) + 1))
                .unwrap();
        let real_time = Instant::now();
        let cpu_time = ThreadTime::now();
        while let Ok(PackedCycleCombinationCandidateQueue(batch_packed_queue)) =
            candidates_receiver.recv()
        {
            loop {
                match solutions_receiver.try_recv() {
                    Ok((c, s)) => {
                        if c != core_id {
                            cycle_combinations.push(s);
                        }
                    }
                    Err(TokioTryRecvError::Closed) => panic!(),
                    Err(TokioTryRecvError::Empty | TokioTryRecvError::Lagged(_)) => break,
                }
            }
            
            let (candidate_counts, mut packed_candidates) =
                batch_packed_queue.split_at(self.mss_batch_size.get());
            for &candidate_count in candidate_counts {
                if candidate_count == 0 {
                    break;
                }
                let candidate_count = candidate_count as usize;
                let (prefix_registers, last_registers_and_next_packed_candidates) =
                    packed_candidates.split_at(usize::from(self.register_count.get() - 1));
                let (last_registers, next_packed_candidates) =
                    last_registers_and_next_packed_candidates.split_at(candidate_count);
                packed_candidates = next_packed_candidates;

                for &last_register in last_registers {
                    processed_candidate_count += 1;
                    let disjoint_registers = DisjointRegisters {
                        prefix_registers,
                        last_register,
                    };

                    if !cycle_combinations.push_and_dominating_check(
                        disjoint_registers,
                        |dominating_registers| {
                            post_candidate_count += 1;
                            if !solutions_calculator.existence(dominating_registers) {
                                return None;
                            }
                            let possible_registers =
                                dominating_registers.iter().collect::<Arc<[_]>>();
                            assert!(
                                solutions_sender
                                    .send((core_id, Arc::clone(&possible_registers)))
                                    .is_ok()
                            );
                            Some(possible_registers)
                        },
                    ) {
                        continue;
                    }
                    // Note that we are allowed to set
                    // `max_last_register_order_reverse_index` to potentially dominated
                    // solutions. If something is the maximum in our atomic variable,
                    // then it must either be in the front or the atomic variable is an
                    // underestimate, which is permitted since our bound is admissible

                    let mut maybe_raw_prunings =
                        pareto_efficient_prunings.load(atomic::Ordering::Acquire);
                    while let Some(next_raw_prunings) = unsafe {
                        try_next_pareto_efficient_prunings(
                            maybe_raw_prunings,
                            disjoint_registers,
                            raw_pruning_len,
                        )
                    } {
                        match pareto_efficient_prunings.compare_exchange(
                            maybe_raw_prunings,
                            next_raw_prunings.as_ptr(),
                            atomic::Ordering::Release,
                            atomic::Ordering::Acquire,
                        ) {
                            Ok(maybe_curr_raw_prunings) => {
                                if let Some(curr_raw_prunings) =
                                    NonNull::new(maybe_curr_raw_prunings)
                                {
                                    unsafe { drop(Box::from_raw(curr_raw_prunings.as_ptr())) }
                                }
                            }
                            Err(curr_raw_prunings) => {
                                unsafe { drop(Box::from_raw(next_raw_prunings.as_ptr())) }
                                maybe_raw_prunings = curr_raw_prunings;
                            }
                        }
                    }
                    break;
                }
            }
        }
        drop(solutions_sender);
        drop(candidates_receiver);
        SolutionsThreadInfo {
            cpu_time: cpu_time.elapsed(),
            real_time: real_time.elapsed(),
            processed_candidate_count,
            post_candidate_count,
            cycle_combinations,
        }
    }

    fn dfs_shard_thread(
        &self,
        core_id: CoreId,
        thread_index: usize,
        num_cores: usize,
        exact_piece_count: NonZeroU32,
        mut shard: CycleCombinationsTreeShard,
        possible_orders_except_one: &[PossibleOrder<N>],
        old_bucket: &Mutex<usize>,
        time_limit_reached: &AtomicBool,
    ) -> TreeThreadInfo {
        if core_affinity::set_for_current(core_id) {
            debug!("DFS: Pinned {core_id:?}");
        }
        let real_time = Instant::now();
        let cpu_time = ThreadTime::now();
        let (maybe_min_order_ratio, maybe_max_order_ratio) =
            self.optimality.maybe_min_max_order_ratio();

        let mut candidate_count = 0;
        for (i, possible_order) in possible_orders_except_one
            .iter()
            .enumerate()
            .rev()
            .skip(thread_index)
            .step_by(num_cores)
        {
            if thread_index == 0
                && let Some(time_limit) = self.maybe_time_limit
                && real_time.elapsed() >= time_limit
            {
                eprintln!("Time limit reached!");
                time_limit_reached.store(true, atomic::Ordering::Relaxed);
            }
            if time_limit_reached.load(atomic::Ordering::Relaxed) {
                break;
            }
            // TODO
            // 9 8 7 6
            // 6
            // 16 8 4 2
            // 4
            // i * r^(c - 1)
            let i_u32 = possible_orders_len_cast(i);

            // Synchronize with the data in the try_update CAS loop
            let maybe_raw_prunings = shard
                .pareto_efficient_prunings
                .load(atomic::Ordering::Acquire);
            if let Some(raw_prunings) = NonNull::new(maybe_raw_prunings) {
                // SAFETY: `solutions_thread` guarantees `raw_pruning` points to at least one
                // element
                let max_last_register = unsafe { raw_prunings.read() };
                if i_u32 <= max_last_register {
                    break;
                }
            }

            // We validated `possible_orders` to be of len `u32` or less
            if log_enabled!(Level::Debug) {
                const PERCENT: usize = 1;

                let num = possible_orders_except_one.len() - i;
                // We don't subtract `max_last_register` here. Cores with large
                // `max_last_register` values are going to exist early, while those with lower
                // values will persist and perform this logging, so the % meter typically goes
                // up to 100%.
                let den = possible_orders_except_one.len();
                let new_bucket = num * 100 / (PERCENT * den);
                let mut bucket = old_bucket.lock();
                if new_bucket > *bucket {
                    *bucket = new_bucket;
                    debug!("DFS: {}% complete", num * 100 / den);
                }
            }

            let Some(next_remaining_piece_count) = exact_piece_count
                .get()
                .checked_sub(u32::from(possible_order.min_piece_count.get()))
            else {
                continue;
            };

            if shard.exact_register_count().get() == 1 {
                if candidate_count == 0 {
                    shard
                        .batch_packed_queue
                        .extend(shard.registers.split_last().1.iter().copied());
                }
                candidate_count += 1;
                shard.batch_packed_queue.push(i_u32);
                continue;
            }

            if let Some(next_remaining_piece_count) = NonZeroU32::new(next_remaining_piece_count)
                && let Ok(next_possible_orders_except_one) =
                    NonemptySlice::try_from(&possible_orders_except_one[..=i])
            {
                *shard.registers.first_mut() = i_u32;
                if let Some(max_order_ratio) = maybe_max_order_ratio {
                    shard.lower_index_cutoff = possible_orders_len_cast(
                        next_possible_orders_except_one.partition_point(|possible_order| {
                            possible_order.order.ln() + max_order_ratio.ln()
                                < next_possible_orders_except_one.last().order.ln()
                        }),
                    );
                }
                unsafe {
                    shard.search_dfs_recur(
                        next_possible_orders_except_one,
                        NonZeroU16::new(1).unwrap(),
                        next_remaining_piece_count,
                    );
                }
            }
        }

        if shard.exact_register_count().get() == 1 && candidate_count != 0 {
            shard.batch_packed_queue[shard.curr_batch_len] = candidate_count;
        }
        shard.maybe_send_queue(true);

        debug!("DFS: {core_id:?} finished");

        TreeThreadInfo {
            real_time: real_time.elapsed(),
            cpu_time: cpu_time.elapsed(),
            candidate_count: shard.candidates_count,
            empty_sends: shard.empty_sends,
            full_sends: shard.full_sends,
            sends: shard.sends,
            sender_lens: shard.sender_lens,
        }
    }

    pub(crate) fn search_dfs(
        &self,
        possible_orders_except_one: &[PossibleOrder<N>],
    ) -> Vec<Arc<[u32]>> {
        // If we return a None here then /shrug
        #[allow(clippy::missing_panics_doc)]
        let mut core_ids = core_affinity::get_core_ids().unwrap();
        if let ValidatedNumCores::Num(num_cores) = self.num_cores {
            core_ids.truncate(num_cores.get());
        }
        let num_cores = core_ids.len();

        // We do not use `0` as to allow a buffer for every core to prevent starvation
        let candidates_sender_capacity = num_cores * 2;
        let (candidates_sender, candidates_receiver) =
            mpmc::sync_channel::<PackedCycleCombinationCandidateQueue>(candidates_sender_capacity);
        // I will only send at most `batch_size` solutions before receiving the queue,
        // so I can make the capacity equal to this
        let (solutions_sender, _) =
            tokio::sync::broadcast::channel(num_cores * self.mss_batch_size.get());

        let pareto_efficient_prunings = AtomicPtr::default();

        // We can unwrap because `exact_register_count` is NonZero.
        #[allow(clippy::missing_panics_doc)]
        let base_shard = CycleCombinationsTreeShard {
            fails: 0,
            batch_packed_queue: vec![],
            sends: 0,
            empty_sends: 0,
            full_sends: 0,
            sender_lens: 0,
            curr_batch_len: 0,
            registers: NonemptyVec::try_from(vec![0; usize::from(self.register_count.get())])
                .unwrap(),
            lower_index_cutoff: 0,
            candidates_count: 0,

            candidates_sender,
            candidates_sender_capacity,
            batch_size: self.mss_batch_size,
            pareto_efficient_prunings: &AtomicPtr::null(),
        };

        let mut candidate_count = 0;
        let mut dfs_real_time = Duration::default();
        let mut dfs_cpu_time = Duration::default();

        let mut solutions_real_time = Duration::default();
        let mut solutions_cpu_time = Duration::default();
        let mut processed_candidate_count = 0;
        let mut post_candidate_count = 0;
        let mut sends = 0;
        let mut empty_sends = 0;
        let mut full_sends = 0;
        let mut sender_lens = 0;
        let mut smallest_fronts = BinaryHeap::new();

        // We are allowed to unwrap because `orbit_defs` is non-empty, and `piece_count`
        // is a NonZero. Therefore the sum must be non-zero.
        let exact_piece_count = NonZeroU32::new(
            self.puzzle_def
                .orbit_defs()
                .iter()
                .map(|&orbit_def| u32::from(orbit_def.piece_count.get()))
                .sum::<u32>(),
        )
        .unwrap();
        let real_time = Instant::now();
        let old_bucket = Mutex::new(0);
        let time_limit_reached = AtomicBool::new(false);
        std::thread::scope(|s| {
            let handles = core_ids
                .into_iter()
                .enumerate()
                .map(|(thread_index, core_id)| {
                    let mut shard = base_shard.clone();
                    shard
                        .batch_packed_queue
                        .extend(std::iter::repeat_n(0, self.mss_batch_size.get()));
                    shard.pareto_efficient_prunings = &pareto_efficient_prunings;

                    let old_bucket = &old_bucket;
                    let time_limit_reached = &time_limit_reached;
                    let tree_thread_handle = s.spawn(move || {
                        self.dfs_shard_thread(
                            core_id,
                            thread_index,
                            num_cores,
                            exact_piece_count,
                            shard,
                            possible_orders_except_one,
                            old_bucket,
                            time_limit_reached,
                        )
                    });
                    let candidates_receiver = candidates_receiver.clone();
                    let solutions_receiver = solutions_sender.subscribe();
                    let solutions_sender = solutions_sender.clone();
                    let pareto_efficient_prunings = &pareto_efficient_prunings;
                    let solutions_thread_handle = s.spawn(move || {
                        self.solutions_thread(
                            core_id,
                            candidates_receiver,
                            solutions_receiver,
                            solutions_sender,
                            pareto_efficient_prunings,
                            possible_orders_except_one,
                        )
                    });
                    (tree_thread_handle, solutions_thread_handle)
                })
                .collect::<Vec<_>>();
            drop(base_shard);
            drop(solutions_sender);

            for (tree_thread_info, solutions_thread_info) in
                handles
                    .into_iter()
                    .map(|(tree_thread_handle, solutions_thread_handle)| {
                        (
                            tree_thread_handle.join().unwrap(),
                            solutions_thread_handle.join().unwrap(),
                        )
                    })
            {
                candidate_count += tree_thread_info.candidate_count;
                dfs_real_time += tree_thread_info.real_time;
                dfs_cpu_time += tree_thread_info.cpu_time;
                sends += tree_thread_info.sends;
                empty_sends += tree_thread_info.empty_sends;
                full_sends += tree_thread_info.full_sends;
                sender_lens += tree_thread_info.sender_lens;

                solutions_cpu_time += solutions_thread_info.cpu_time;
                solutions_real_time += solutions_thread_info.real_time;
                processed_candidate_count += solutions_thread_info.processed_candidate_count;
                post_candidate_count += solutions_thread_info.post_candidate_count;
                smallest_fronts.push(solutions_thread_info.cycle_combinations);
            }
        });

        let mut combined_cycle_combinations = CCParetoFront::default();
        trace!(
            "{}",
            smallest_fronts
                .iter()
                .filter_map(|x| {
                    let s = dbg_registers_iter(
                        x.possible_registers
                            .iter()
                            .map(|combination| combination.iter().copied()),
                        possible_orders_except_one,
                    );
                    if s.is_empty() { None } else { Some(s) }
                })
                .collect::<Vec<_>>()
                .join("\n\n")
        );
        while let Some(mut smallest_front) = smallest_fronts.pop() {
            if let Some(smaller_front) = smallest_fronts.pop() {
                smallest_front.merge(smaller_front);
                smallest_fronts.push(smallest_front);
            } else {
                combined_cycle_combinations = smallest_front;
            }
        }

        let real_time = real_time.elapsed();

        let maybe_raw_prunings = pareto_efficient_prunings.into_inner();
        let max_last_register = if let Some(raw_prunings) = NonNull::new(maybe_raw_prunings) {
            // SAFETY: `solutions_thread` guarantees `raw_pruning` points to at least one
            // element
            unsafe { raw_prunings.read() }
        } else {
            0
        };

        #[allow(clippy::cast_precision_loss)]
        let pruned_orders_percentage =
            f64::from(max_last_register) / ((possible_orders_except_one.len() * num_cores) as f64);

        #[allow(clippy::cast_precision_loss)]
        let full_sends_percentage = full_sends as f64 / sends as f64;

        #[allow(clippy::cast_precision_loss)]
        let empty_sends_percentage = empty_sends as f64 / sends as f64;

        #[allow(clippy::cast_precision_loss)]
        let sender_len_percentage =
            sender_lens as f64 / (candidates_sender_capacity as u64 * sends) as f64;

        let dfs_io_time = dfs_real_time.saturating_sub(dfs_cpu_time);
        let solutions_io_time = solutions_real_time.saturating_sub(solutions_cpu_time);

        let tree_profile_info = TreeProfileInfo {
            candidate_count,
            processed_candidate_count,
            post_candidate_count,
            pruned_orders_percentage,
            sender_len_percentage,
            empty_sends_percentage,
            full_sends_percentage,
            real_time,
            dfs_cpu_time,
            dfs_io_time,
            solutions_cpu_time,
            solutions_io_time,
            num_cores,
        };

        debug!("Search tree complete: {tree_profile_info:#}");
        info!("Search tree took {}", real_time.human(Truncate::Micro));

        combined_cycle_combinations.into()
    }
}

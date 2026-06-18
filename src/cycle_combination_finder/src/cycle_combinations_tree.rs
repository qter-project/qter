use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    fmt::{self, Debug},
    num::{NonZeroU16, NonZeroU32, NonZeroUsize},
    sync::{
        Arc,
        atomic::{self, AtomicPtr},
        mpmc,
    },
    time::{Duration, Instant},
};

use core_affinity::CoreId;
use cpu_time::ThreadTime;
use humanize_duration::{Truncate, prelude::DurationExt};
use log::{Level, debug, log_enabled, trace};

use crate::{
    cycle_combination_details::CycleCombinationDetails,
    finder::{CycleCombination, PossibleOrder},
    nonemptyvec::{NonemptySlice, NonemptyVec},
    pareto_front::CCParetoFront,
    puzzle::OrbitDef,
};

// 7+ is where the performance plateaus from some stupid testing
const BATCH_SIZE: NonZeroUsize = NonZeroUsize::new(7).unwrap();

pub(crate) struct CycleCombinationsTree<const N: usize> {
    possible_orders_except_one: Arc<[PossibleOrder<N>]>,
    exact_register_count: NonZeroU16,
    exact_piece_count: NonZeroU32,
}

#[derive(Clone)]
struct CycleCombinationsTreeMutable {
    batch_packed_queue: Vec<u32>,
    curr_batch_len: usize,
    registers: NonemptyVec<u32>,
    sender: mpmc::Sender<PackedCycleCombinationCandidateQueue>,
    alloc_time: Duration,
    candidate_count: u64,
}

#[derive(Debug, Clone)]
struct PackedCycleCombinationCandidateQueue(Box<[u32]>);

#[derive(Clone, Copy)]
pub struct DisjointRegisters<'a> {
    prefix_registers: &'a [u32],
    last_register: u32,
}

struct DetailsThreadInfo {
    mkp_real_time: Duration,
    mkp_cpu_time: Duration,
    mkp_alloc_time: Duration,
    processed_candidate_count: u64,
    post_candidate_count: u64,
    cycle_combinations: CCParetoFront,
}

#[derive(Default, Clone)]
struct TreeThreadInfo {
    real_time: Duration,
    cpu_time: Duration,
    alloc_time: Duration,
    candidate_count: u64,
}

struct ProfileInfo {
    candidate_count: u64,
    processed_candidate_count: u64,
    post_candidate_count: u64,
    pruned_orders_percentage: f64,
    real_time: Duration,
    dfs_alloc_time: Duration,
    dfs_cpu_time: Duration,
    dfs_io_time: Duration,
    mkp_alloc_time: Duration,
    mkp_cpu_time: Duration,
    mkp_io_time: Duration,
    num_cores: usize,
}

impl CycleCombinationsTreeMutable {
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
        if self.curr_batch_len < BATCH_SIZE.get() && !force {
            return;
        }
        if log_enabled!(Level::Debug) {
            self.candidate_count += self
                .batch_packed_queue
                .iter()
                .skip(1)
                .take(BATCH_SIZE.get())
                .map(|&candidate_count| u64::from(candidate_count))
                .sum::<u64>();
            let now = Instant::now();
            let payload =
                PackedCycleCombinationCandidateQueue(Box::clone_from_ref(&self.batch_packed_queue));
            self.alloc_time += now.elapsed();
            // We can unwrap because the senders is only dropped after all threads are
            // joined.
            self.sender.send(payload).unwrap();
        } else {
            // We can unwrap because the senders is only dropped after all threads are
            // joined.
            self.sender
                .send(PackedCycleCombinationCandidateQueue(Box::clone_from_ref(
                    &self.batch_packed_queue,
                )))
                .unwrap();
        }
        self.curr_batch_len = 0;
        self.batch_packed_queue.truncate(BATCH_SIZE.get() + 1);
        for b in self.batch_packed_queue.iter_mut().skip(1) {
            *b = 0;
        }
    }
}

impl Debug for ProfileInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[allow(clippy::cast_precision_loss)]
        let num_cores = self.num_cores as f64;
        let cpu_time = self.real_time.mul_f64(num_cores);
        f.debug_struct("ProfileInfo")
            .field(&format!("{:>25}", "candidate_count"), &self.candidate_count)
            .field(
                &format!("{:>25}", "processed_candidate_count"),
                &self.processed_candidate_count,
            )
            .field(
                &format!("{:>25}", "post_candidate_count"),
                &format!(
                    "{} ({} / thread)",
                    self.post_candidate_count,
                    self.post_candidate_count / u64::try_from(self.num_cores).unwrap(),
                ),
            )
            .field(
                &format!("{:>25}", "pruned_orders_percentage"),
                &format!("{:05.2}%", self.pruned_orders_percentage * 100.0),
            )
            .field(
                &format!("{:>25}", "real_time"),
                &format!("{}", self.real_time.human(Truncate::Millis)),
            )
            .field(
                &format!("{:>25}", "single_cpu_time"),
                &format!(
                    "{}",
                    (self.dfs_cpu_time + self.mkp_cpu_time)
                        .div_f64(num_cores)
                        .human(Truncate::Millis)
                ),
            )
            .field(
                &format!("{:>25}", "dfs_alloc_time"),
                &format!(
                    "{:05.2}% ({})",
                    self.dfs_alloc_time.div_duration_f64(cpu_time) * 100.0,
                    self.dfs_alloc_time
                        .div_f64(num_cores)
                        .human(Truncate::Millis)
                ),
            )
            .field(
                &format!("{:>25}", "dfs_cpu_time"),
                &format!(
                    "{:05.2}% ({})",
                    self.dfs_cpu_time.div_duration_f64(cpu_time) * 100.0,
                    self.dfs_cpu_time.div_f64(num_cores).human(Truncate::Millis)
                ),
            )
            .field(
                &format!("{:>25}", "dfs_io_time"),
                &format!(
                    "{:05.2}% ({})",
                    self.dfs_io_time.div_duration_f64(cpu_time) * 100.0,
                    self.dfs_io_time.div_f64(num_cores).human(Truncate::Millis)
                ),
            )
            .field(
                &format!("{:>25}", "mkp_alloc_time"),
                &format!(
                    "{:05.2}% ({})",
                    self.mkp_alloc_time.div_duration_f64(cpu_time) * 100.0,
                    self.mkp_alloc_time
                        .div_f64(num_cores)
                        .human(Truncate::Millis)
                ),
            )
            .field(
                &format!("{:>25}", "mkp_cpu_time"),
                &format!(
                    "{:05.2}% ({})",
                    self.mkp_cpu_time.div_duration_f64(cpu_time) * 100.0,
                    self.mkp_cpu_time.div_f64(num_cores).human(Truncate::Millis)
                ),
            )
            .field(
                &format!("{:>25}", "mkp_io_time"),
                &format!(
                    "{:05.2}% ({})",
                    self.mkp_io_time.div_duration_f64(cpu_time) * 100.0,
                    self.mkp_io_time.div_f64(num_cores).human(Truncate::Millis)
                ),
            )
            .field(&format!("{:>25}", "num_cores"), &self.num_cores)
            .finish()
    }
}

fn possible_orders_len_cast(len: usize) -> u32 {
    #[allow(clippy::cast_possible_truncation)]
    let len = len as u32;
    len
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
    pub fn iter(&self) -> impl Iterator<Item = u32> {
        self.prefix_registers
            .iter()
            .copied()
            .chain(std::iter::once(self.last_register))
    }

    #[must_use]
    pub fn get(&self, i: usize) -> Option<u32> {
        if i == self.prefix_registers.len() {
            Some(self.last_register)
        } else {
            self.prefix_registers.get(i).copied()
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn details_thread<const N: usize>(
    core_id: CoreId,
    receiver: mpmc::Receiver<PackedCycleCombinationCandidateQueue>,
    pareto_efficient_prunings: Arc<[AtomicPtr<u32>]>,
    possible_orders_except_one: &[PossibleOrder<N>],
    exact_register_count: NonZeroU16,
) -> DetailsThreadInfo {
    core_affinity::set_for_current(core_id);
    let mut cycle_combinations = CCParetoFront::default();
    let mut processed_candidate_count = 0;
    let mut post_candidate_count = 0;
    let raw_pruning_len = NonZeroUsize::new(usize::from(
        exact_register_count.get().saturating_sub(2) + 1,
    ))
    .unwrap();
    let real_time = Instant::now();
    let cpu_time = ThreadTime::now();
    let mut alloc_time = Duration::default();
    while let Ok(batch_packed_queue) = receiver.recv() {
        let (thread_index_and_candidate_counts, mut candidates) =
            batch_packed_queue.0.split_at(BATCH_SIZE.get() + 1);
        let (&thread_index, candidate_counts) =
            thread_index_and_candidate_counts.split_first().unwrap();
        let thread_index = thread_index as usize;
        for &candidate_count in candidate_counts {
            let candidate_count = candidate_count as usize;
            if candidate_count == 0 {
                break;
            }
            let (prefix_and_last_registers, next_candidates) =
                candidates.split_at(usize::from(exact_register_count.get() - 1) + candidate_count);
            candidates = next_candidates;
            let (prefix_registers, last_registers) =
                prefix_and_last_registers.split_at(usize::from(exact_register_count.get() - 1));

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
                        CycleCombinationDetails::new(
                            dominating_registers,
                            possible_orders_except_one,
                        )
                        .map(|details| {
                            let registers = if log_enabled!(Level::Debug) {
                                let now = Instant::now();
                                let registers = dominating_registers.iter().collect::<Box<_>>();
                                alloc_time += now.elapsed();
                                registers
                            } else {
                                dominating_registers.iter().collect::<Box<_>>()
                            };
                            CycleCombination { registers, details }
                        })
                    },
                ) {
                    continue;
                }
                // Note that we are allowed to set
                // `max_last_register_order_reverse_index` to potentially dominated
                // solutions. If something is the maximum in our atomic variable,
                // then it must either be in the front or the atomic variable is an
                // underestimate, which is permitted since our bound is admissible
                let _ = pareto_efficient_prunings[thread_index].try_update(
                    atomic::Ordering::Release,
                    atomic::Ordering::Acquire,
                    |pareto_efficient_pruning| {
                        if !pareto_efficient_pruning.is_null() {
                            // SAFETY: later in this block we always initialize
                            // `pareto_efficient_pruning` to be of `raw_pruning_len` length.
                            let raw_pruning = unsafe {
                                NonemptySlice::from_raw_parts(
                                    pareto_efficient_pruning,
                                    raw_pruning_len,
                                )
                            };
                            // let mut raw_pruning_iter = raw_pruning.iter().copied();
                            // We can unwrap because we have at least one element
                            // let max_last_register = raw_pruning_iter.next().unwrap();
                            let (&max_last_register, pareto_efficent_prunes) =
                                raw_pruning.split_first();
                            if last_register < max_last_register {
                                return None;
                            }
                            if last_register == max_last_register {
                                let mut maybe_next_pareto_efficient_pruning: Option<Vec<u32>> =
                                    None;
                                for ((i, &prefix_register), pareto_efficient_prune) in
                                    prefix_registers
                                        .iter()
                                        .enumerate()
                                        .skip(1)
                                        .zip(pareto_efficent_prunes)
                                {
                                    match &mut maybe_next_pareto_efficient_pruning {
                                        Some(next_pareto_efficient_pruning) => {
                                            next_pareto_efficient_pruning.push(prefix_register);
                                        }
                                        None => match prefix_register.cmp(pareto_efficient_prune) {
                                            Ordering::Less => return None,
                                            Ordering::Equal => (),
                                            Ordering::Greater => {
                                                let now = Instant::now();
                                                let mut next_pareto_efficient_pruning =
                                                    Vec::with_capacity(raw_pruning_len.get());
                                                alloc_time += now.elapsed();
                                                next_pareto_efficient_pruning.extend(
                                                    std::iter::once(last_register).chain(
                                                        prefix_registers
                                                            .iter()
                                                            .copied()
                                                            .skip(1)
                                                            .take(i),
                                                    ),
                                                );
                                                maybe_next_pareto_efficient_pruning =
                                                    Some(next_pareto_efficient_pruning);
                                            }
                                        },
                                    }
                                }

                                // new can still be None here:
                                // A C D can be a solution, followed by B C D
                                return maybe_next_pareto_efficient_pruning.map(
                                    |next_pareto_efficient_pruning| {
                                        debug_assert_eq!(
                                            next_pareto_efficient_pruning.len(),
                                            raw_pruning_len.get(),
                                        );
                                        Box::into_raw(
                                            next_pareto_efficient_pruning.into_boxed_slice(),
                                        )
                                        .as_mut_ptr()
                                    },
                                );
                            }
                        }
                        Some(
                            Box::into_raw(
                                std::iter::once(last_register)
                                    .chain(prefix_registers.iter().copied().skip(1))
                                    .collect::<Box<_>>(),
                            )
                            .as_mut_ptr(),
                        )
                    },
                );
                break;
            }
        }
    }
    DetailsThreadInfo {
        mkp_cpu_time: cpu_time.elapsed(),
        mkp_real_time: real_time.elapsed(),
        mkp_alloc_time: alloc_time,
        processed_candidate_count,
        post_candidate_count,
        cycle_combinations,
    }
}

#[allow(clippy::needless_pass_by_value)]
fn dfs_thread<const N: usize>(
    core_id: CoreId,
    thread_index: usize,
    num_cores: usize,
    exact_piece_count: NonZeroU32,
    mut mutable: CycleCombinationsTreeMutable,
    pareto_efficient_pruning: &AtomicPtr<u32>,
    possible_orders_except_one: &[PossibleOrder<N>],
) -> TreeThreadInfo {
    core_affinity::set_for_current(core_id);
    let real_time = Instant::now();
    let cpu_time = ThreadTime::now();

    let mut old_bucket = 0;
    let mut candidate_count = 0;
    for (i, possible_order) in possible_orders_except_one
        .iter()
        .enumerate()
        .rev()
        .skip(thread_index)
        .step_by(num_cores)
    {
        let i_u32 = possible_orders_len_cast(i);
        // Synchronize with the data in the try_update CAS loop
        let raw_pruning = pareto_efficient_pruning.load(atomic::Ordering::Acquire);
        let max_last_register = if raw_pruning.is_null() {
            0
        } else {
            // SAFETY: `details_thread` guarantees `raw_pruning` points to at least one
            // element
            let max_last_register = unsafe { *raw_pruning };
            if i_u32 <= max_last_register {
                break;
            }
            max_last_register
        };
        if thread_index == 0 {
            // We validated `possible_orders` to be of len `u32` or less
            let len = possible_orders_len_cast(possible_orders_except_one.len());
            let new_percent = f64::from(len - i_u32) / f64::from(len - max_last_register);
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            let new_bucket = (new_percent * 20.0).floor() as u8;
            if new_bucket > old_bucket {
                debug!("DFS: {}% complete", (new_percent * 100.0).floor());
                old_bucket = new_bucket;
            }
        }

        let Some(next_remaining_piece_count) = exact_piece_count
            .get()
            .checked_sub(possible_order.min_piece_count.get())
        else {
            continue;
        };

        if mutable.exact_register_count().get() == 1 {
            if candidate_count == 0 {
                mutable
                    .batch_packed_queue
                    .extend(mutable.registers.split_last().1.iter().copied());
            }
            candidate_count += 1;
            mutable.batch_packed_queue.push(i_u32);
            continue;
        }

        if let Some(next_remaining_piece_count) = NonZeroU32::new(next_remaining_piece_count)
            && let Ok(next_possible_orders) =
                NonemptySlice::try_from(&possible_orders_except_one[..=i])
        {
            *mutable.registers.first_mut() = i_u32;
            unsafe {
                search_dfs_helper(
                    &mut mutable,
                    pareto_efficient_pruning,
                    next_possible_orders,
                    NonZeroU16::new(1).unwrap(),
                    next_remaining_piece_count,
                );
            }
        }
    }

    if mutable.exact_register_count().get() == 1 && candidate_count != 0 {
        mutable.batch_packed_queue[mutable.curr_batch_len + 1] = candidate_count;
    }
    mutable.maybe_send_queue(true);

    TreeThreadInfo {
        real_time: real_time.elapsed(),
        cpu_time: cpu_time.elapsed(),
        alloc_time: mutable.alloc_time,
        candidate_count: mutable.candidate_count,
    }
}

/// # Safety
///
/// `register_index` must be less than `mutable.exact_register_count()`.
unsafe fn search_dfs_helper<const N: usize>(
    mutable: &mut CycleCombinationsTreeMutable,
    pareto_efficient_pruning: &AtomicPtr<u32>,
    possible_orders: NonemptySlice<'_, PossibleOrder<N>>,
    register_index: NonZeroU16,
    remaining_piece_count: NonZeroU32,
) {
    let mut curr_possible_orders = possible_orders;
    // It should never overflow, and I don't want a panic path, so use saturating
    // logic
    let next_register_index = register_index.saturating_add(1);
    let mut candidate_count = 0;
    loop {
        let (possible_order, next_possible_orders) = curr_possible_orders.split_last();
        let i = possible_orders_len_cast(next_possible_orders.len());

        // Synchronize with the data in the try_update CAS loop
        let raw_pruning = pareto_efficient_pruning.load(atomic::Ordering::Acquire);
        if !raw_pruning.is_null() {
            // SAFETY: `raw_pruning` is guaranteed to point to
            // `mutable.exact_register_count().get().saturating_sub(2) + 1` u32s. The caller
            // guarantees `register_index` is less than `mutable.exact_register_count()`;
            // therefore we are in bounds
            let raw_pruning = unsafe {
                NonemptySlice::from_raw_parts(raw_pruning, NonZeroUsize::from(register_index))
            };
            let (&max_last_register_order, pareto_efficent_prunes) = raw_pruning.split_first();
            if i <= max_last_register_order
                && mutable
                    .registers
                    .iter()
                    .skip(1)
                    .zip(pareto_efficent_prunes)
                    .all(|(register, pareto_efficient_prune)| register <= pareto_efficient_prune)
            {
                break;
            }
        }

        if let Some(next_remaining_piece_count) = remaining_piece_count
            .get()
            .checked_sub(possible_order.min_piece_count.get())
        {
            if next_register_index == mutable.exact_register_count() {
                if candidate_count == 0 {
                    mutable
                        .batch_packed_queue
                        .extend(mutable.registers.split_last().1.iter().copied());
                }
                candidate_count += 1;
                mutable.batch_packed_queue.push(i);
            } else if let Some(next_remaining_piece_count) =
                NonZeroU32::new(next_remaining_piece_count)
            {
                // SAFETY: caller guarantees `register_index < mutable.exact_register_count()`,
                // therefore we are in bounds
                let old = std::mem::replace(
                    unsafe {
                        mutable
                            .registers
                            .get_unchecked_mut(usize::from(register_index.get()))
                    },
                    i,
                );
                // SAFETY: `next_register_index != mutable.exact_register_count()` in this
                // branch, and caller guarantees we are less
                unsafe {
                    search_dfs_helper(
                        mutable,
                        pareto_efficient_pruning,
                        curr_possible_orders,
                        next_register_index,
                        next_remaining_piece_count,
                    );
                }
                // SAFETY: caller guarantees `register_index < mutable.exact_register_count()`,
                // therefore we are in bounds
                unsafe {
                    *mutable
                        .registers
                        .get_unchecked_mut(usize::from(register_index.get())) = old;
                };
            }
        }
        match NonemptySlice::try_from(next_possible_orders) {
            Ok(next_possible_orders) => {
                curr_possible_orders = next_possible_orders;
            }
            Err(()) => {
                break;
            }
        }
    }
    if next_register_index == mutable.exact_register_count() && candidate_count != 0 {
        mutable.batch_packed_queue[mutable.curr_batch_len + 1] = candidate_count;
        mutable.maybe_send_queue(false);
    }
}

impl<const N: usize> CycleCombinationsTree<N> {
    #[must_use]
    pub fn new(
        exact_register_count: NonZeroU16,
        possible_orders_except_one: Arc<[PossibleOrder<N>]>,
        orbit_defs: NonemptySlice<'_, OrbitDef>,
    ) -> Self {
        #[allow(clippy::missing_panics_doc)]
        // We are allowed to unwrap because `orbit_defs` is non-empty, and `piece_count` is a
        // NonZero. Therefore the sum must be non-zero.
        let exact_piece_count = NonZeroU32::new(
            orbit_defs
                .iter()
                .map(|&orbit_def| u32::from(orbit_def.piece_count.get()))
                .sum::<u32>(),
        )
        .unwrap();

        Self {
            possible_orders_except_one,
            exact_register_count,
            exact_piece_count,
        }
    }

    #[must_use]
    pub(crate) fn search_dfs(self) -> (Vec<CycleCombination>, Arc<[PossibleOrder<N>]>) {
        // If we return a None here then /shrug
        #[allow(clippy::missing_panics_doc)]
        let core_ids = core_affinity::get_core_ids().unwrap();
        let num_cores = core_ids.len();

        // We do not use `0` as to allow a buffer for every core to prevent starvation
        let (sender, receiver) =
            mpmc::sync_channel::<PackedCycleCombinationCandidateQueue>(core_ids.len() * 10);

        // We can unwrap because `exact_register_count` is NonZero.
        #[allow(clippy::missing_panics_doc)]
        let mutable = CycleCombinationsTreeMutable {
            batch_packed_queue: vec![],
            curr_batch_len: 0,
            registers: NonemptyVec::try_from(vec![0; usize::from(self.exact_register_count.get())])
                .unwrap(),
            sender,
            alloc_time: Duration::default(),
            candidate_count: 0,
        };

        let mut candidate_count = 0;
        let mut dfs_real_time = Duration::default();
        let mut dfs_cpu_time = Duration::default();
        let mut dfs_alloc_time = Duration::default();

        let mut mkp_real_time = Duration::default();
        let mut mkp_cpu_time = Duration::default();
        let mut mkp_alloc_time = Duration::default();
        let mut processed_candidate_count = 0;
        let mut post_candidate_count = 0;
        let mut smallest_fronts = BinaryHeap::new();

        let pareto_efficient_prunings: Arc<[_]> = Arc::from(
            (0..num_cores)
                .map(|_| AtomicPtr::default())
                .collect::<Box<[_]>>(),
        );
        let real_time = Instant::now();
        std::thread::scope(|s| {
            let possible_orders_except_one = &self.possible_orders_except_one;

            let handles = core_ids
                .into_iter()
                .enumerate()
                .zip(pareto_efficient_prunings.iter())
                .map(|((thread_index, core_id), pareto_efficient_pruning)| {
                    let mut mutable = mutable.clone();
                    mutable
                        .batch_packed_queue
                        .push(u32::try_from(thread_index).expect("You have too many threads."));
                    mutable
                        .batch_packed_queue
                        .extend(std::iter::repeat_n(0, BATCH_SIZE.get()));
                    let tree_thread_handle = s.spawn(move || {
                        dfs_thread(
                            core_id,
                            thread_index,
                            num_cores,
                            self.exact_piece_count,
                            mutable,
                            pareto_efficient_pruning,
                            possible_orders_except_one,
                        )
                    });
                    let receiver = receiver.clone();
                    let pareto_efficient_prunings = Arc::clone(&pareto_efficient_prunings);
                    let details_thread_handle = s.spawn(move || {
                        details_thread(
                            core_id,
                            receiver,
                            pareto_efficient_prunings,
                            possible_orders_except_one,
                            self.exact_register_count,
                        )
                    });
                    (tree_thread_handle, details_thread_handle)
                })
                .collect::<Vec<_>>();
            drop(mutable);

            for (tree_thread_info, details_thread_info) in
                handles
                    .into_iter()
                    .map(|(tree_thread_handle, details_thread_handle)| {
                        (
                            tree_thread_handle.join().unwrap(),
                            details_thread_handle.join().unwrap(),
                        )
                    })
            {
                candidate_count += tree_thread_info.candidate_count;
                dfs_real_time += tree_thread_info.real_time;
                dfs_cpu_time += tree_thread_info.cpu_time;
                dfs_alloc_time += tree_thread_info.alloc_time;

                mkp_cpu_time += details_thread_info.mkp_cpu_time;
                mkp_real_time += details_thread_info.mkp_real_time;
                mkp_alloc_time += details_thread_info.mkp_alloc_time;
                processed_candidate_count += details_thread_info.processed_candidate_count;
                post_candidate_count += details_thread_info.post_candidate_count;
                smallest_fronts.push(details_thread_info.cycle_combinations);
            }
        });

        let mut combined_cycle_combinations = CCParetoFront::default();
        trace!(
            "{}",
            smallest_fronts
                .iter()
                .filter_map(|x| {
                    let s = dbg_registers_iter(
                        x.inner
                            .iter()
                            .map(|combination| combination.registers.iter().copied()),
                        &self.possible_orders_except_one,
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

        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        let pruned_orders_percentage = (pareto_efficient_prunings
            .iter()
            .map(|max_last_register| {
                // Synchronize with the data in the try_update CAS loop
                let max_last_register = max_last_register.load(atomic::Ordering::Acquire);
                u64::from(if max_last_register.is_null() {
                    0
                } else {
                    // SAFETY: `details_thread` guarantees `raw_pruning` points to at least one
                    // element
                    unsafe { *max_last_register }
                })
            })
            .sum::<u64>() as f64)
            / ((self.possible_orders_except_one.len() * num_cores) as f64);

        let dfs_io_time = dfs_real_time
            .saturating_sub(dfs_cpu_time)
            .saturating_sub(dfs_alloc_time);
        let mkp_io_time = mkp_real_time
            .saturating_sub(mkp_cpu_time)
            .saturating_sub(mkp_alloc_time);

        let profile_info = ProfileInfo {
            candidate_count,
            processed_candidate_count,
            post_candidate_count,
            pruned_orders_percentage,
            real_time,
            dfs_alloc_time,
            dfs_cpu_time,
            dfs_io_time,
            mkp_alloc_time,
            mkp_cpu_time,
            mkp_io_time,
            num_cores,
        };

        debug!("Search tree complete");
        debug!("{profile_info:#?}");
        (
            combined_cycle_combinations.into(),
            self.possible_orders_except_one,
        )
    }
}

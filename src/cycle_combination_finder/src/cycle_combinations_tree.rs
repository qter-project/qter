use std::{
    collections::BinaryHeap,
    fmt::{self, Debug},
    num::{NonZeroU16, NonZeroU32, NonZeroUsize},
    sync::{
        Arc,
        atomic::{self, AtomicUsize},
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

pub(crate) struct CycleCombinationsTree<const N: usize> {
    possible_orders_except_one: Arc<[PossibleOrder<N>]>,
    exact_register_count: NonZeroU16,
    exact_piece_count: NonZeroU32,
}

#[derive(Clone)]
struct CycleCombinationsTreeMutable {
    prefix_and_last_registers: Vec<usize>,
    registers: NonemptyVec<usize>,
    sender: mpmc::Sender<PackedCycleCombinationCandidateQueue>,
    alloc_time: Duration,
    candidate_count: u32,
}

#[derive(Default)]
pub struct CycleCombinationsTreeConcurrent {
    max_last_register_order: AtomicUsize,
}

#[derive(Debug, Clone)]
struct PackedCycleCombinationCandidateQueue {
    prefix_and_last_registers: Box<[usize]>,
}

#[derive(Clone, Copy)]
pub struct DisjointRegisters<'a> {
    prefix_registers: &'a [usize],
    last_register: usize,
}

struct DetailsThreadInfo {
    mkp_real_time: Duration,
    mkp_cpu_time: Duration,
    post_candidate_count: u32,
    cycle_combinations: CCParetoFront,
}

#[derive(Default, Clone)]
struct TreeThreadInfo {
    real_time: Duration,
    cpu_time: Duration,
    alloc_time: Duration,
    candidate_count: u32,
}

struct ProfileInfo {
    candidate_count: u32,
    post_candidate_count: u32,
    pruned_orders_percentage: f64,
    real_time: Duration,
    dfs_alloc_time: Duration,
    dfs_cpu_time: Duration,
    dfs_io_time: Duration,
    mkp_cpu_time: Duration,
    mkp_io_time: Duration,
    num_cores: usize,
}

impl CycleCombinationsTreeMutable {
    fn exact_register_count(&self) -> NonZeroU16 {
        // Cast truncation is fine because `self.registers` is the length of the number
        // of registers, which is a `NonZeruU16`
        #[allow(clippy::cast_possible_truncation)]
        unsafe {
            NonZeroU16::new_unchecked(self.registers.len().get() as u16)
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
                &format!("{:>25}", "post_candidate_count"),
                &format!(
                    "{} ({} total)",
                    self.post_candidate_count / u32::try_from(self.num_cores).unwrap(),
                    self.post_candidate_count
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

impl DisjointRegisters<'_> {
    pub fn iter(&self) -> impl Iterator<Item = usize> {
        self.prefix_registers
            .iter()
            .copied()
            .chain(std::iter::once(self.last_register))
    }

    #[must_use]
    pub fn get(&self, i: usize) -> Option<usize> {
        if i == self.prefix_registers.len() {
            Some(self.last_register)
        } else {
            self.prefix_registers.get(i).copied()
        }
    }
}

#[must_use]
pub fn dbg_registers<const N: usize>(
    registers: impl IntoIterator<Item = usize>,
    possible_orders: &[PossibleOrder<N>],
) -> String {
    registers
        .into_iter()
        .map(|x| possible_orders[x].order.as_bigint().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[must_use]
pub fn dbg_registers_iter<const N: usize>(
    registers_iter: impl IntoIterator<Item = impl IntoIterator<Item = usize>>,
    possible_orders: &[PossibleOrder<N>],
) -> String {
    registers_iter
        .into_iter()
        .map(|registers| dbg_registers(registers, possible_orders))
        .collect::<Vec<_>>()
        .join("\n")
}

#[allow(clippy::needless_pass_by_value)]
fn details_thread<const N: usize>(
    core_id: CoreId,
    receiver: mpmc::Receiver<PackedCycleCombinationCandidateQueue>,
    concurrent: Arc<CycleCombinationsTreeConcurrent>,
    possible_orders_except_one: Arc<[PossibleOrder<N>]>,
    exact_register_count: NonZeroU16,
) -> DetailsThreadInfo {
    core_affinity::set_for_current(core_id);
    let mut cycle_combinations = CCParetoFront::default();
    let mut post_candidate_count = 0;
    let real_time = Instant::now();
    let cpu_time = ThreadTime::now();
    while let Ok(packed_queue) = receiver.recv() {
        let (prefix_registers, last_registers) = packed_queue
            .prefix_and_last_registers
            .split_at(usize::from(exact_register_count.get() - 1));
        for &last_register in last_registers {
            let disjoint_registers = DisjointRegisters {
                prefix_registers,
                last_register,
            };
            if cycle_combinations.push_and_dominating_check(
                disjoint_registers,
                |dominating_registers| {
                    post_candidate_count += 1;
                    CycleCombinationDetails::new(dominating_registers, &possible_orders_except_one)
                        .map(|details| CycleCombination {
                            registers: dominating_registers.iter().collect::<Box<_>>(),
                            details,
                        })
                },
            ) {
                // Note that we are allowed to set
                // `max_last_register_order_reverse_index` to potentially dominated
                // solutions. If something is the maximum in our atomic variable,
                // then it must either be in the front or the atomic variable is an
                // underestimate, which is permitted since our bound is admissible
                concurrent
                    .max_last_register_order
                    .fetch_max(last_register, atomic::Ordering::Relaxed);
                break;
            }
        }
    }
    DetailsThreadInfo {
        cycle_combinations,
        mkp_cpu_time: cpu_time.elapsed(),
        mkp_real_time: real_time.elapsed(),
        post_candidate_count,
    }
}

/// # Safety
///
/// `remaining_register_count` must be less than or equal to
/// `mutable.registers.len()`.
unsafe fn search_dfs_helper<const N: usize>(
    mutable: &mut CycleCombinationsTreeMutable,
    concurrent: &Arc<CycleCombinationsTreeConcurrent>,
    possible_orders: NonemptySlice<'_, PossibleOrder<N>>,
    remaining_register_count: NonZeroU16,
    remaining_piece_count: NonZeroU32,
) {
    let register_index = mutable.exact_register_count().get() - remaining_register_count.get();
    let mut curr_possible_orders = possible_orders;
    let maybe_next_remaining_register_count = NonZeroU16::new(remaining_register_count.get() - 1);
    if maybe_next_remaining_register_count.is_none() {
        mutable.prefix_and_last_registers.clear();
        mutable
            .prefix_and_last_registers
            .extend(mutable.registers.split_last().1.iter().copied());
    }
    loop {
        let (possible_order, next_possible_orders) = curr_possible_orders.split_last();
        let i = next_possible_orders.len();
        if register_index == 0
            && i <= concurrent
                .max_last_register_order
                .load(atomic::Ordering::Relaxed)
        {
            break;
        }

        if let Some(next_remaining_piece_count) = remaining_piece_count
            .get()
            .checked_sub(possible_order.min_piece_count.get())
        {
            if let Some(next_remaining_register_count) = maybe_next_remaining_register_count {
                if let Some(next_remaining_piece_count) =
                    NonZeroU32::new(next_remaining_piece_count)
                {
                    // SAFETY: caller guarantees `mutable.registers.len()` <=
                    // `remaining_register_count`, and `remaining_register_count` != 0.
                    // `register_index` must thus be in bounds of `mutable.registers`.
                    let old = std::mem::replace(
                        unsafe { mutable.registers.get_unchecked_mut(register_index as usize) },
                        i,
                    );
                    // SAFETY: `remaining_register_count` only ever decreases.
                    unsafe {
                        search_dfs_helper(
                            mutable,
                            concurrent,
                            curr_possible_orders,
                            next_remaining_register_count,
                            next_remaining_piece_count,
                        );
                    }
                    // SAFETY: see above.
                    *unsafe { mutable.registers.get_unchecked_mut(register_index as usize) } = old;
                }
            } else {
                mutable.prefix_and_last_registers.push(i);
            }
        }
        match NonemptySlice::try_from(next_possible_orders) {
            Ok(ret) => {
                curr_possible_orders = ret;
            }
            Err(()) => {
                break;
            }
        }
    }

    if maybe_next_remaining_register_count.is_none() {
        mutable.candidate_count +=
            (mutable.prefix_and_last_registers.len() + 1 - mutable.registers.len().get()) as u32;
        let maybe_now = log_enabled!(Level::Debug).then(Instant::now);
        let payload = PackedCycleCombinationCandidateQueue {
            prefix_and_last_registers: Box::clone_from_ref(&mutable.prefix_and_last_registers),
        };
        if let Some(now) = maybe_now {
            mutable.alloc_time += now.elapsed();
        }
        mutable.sender.send(payload).unwrap();
    }
}

// TODO: construct mutable in each thread
unsafe fn search_dfs_helper_helper<const N: usize>(
    core_ids: Vec<CoreId>,
    mutable: CycleCombinationsTreeMutable,
    concurrent: &Arc<CycleCombinationsTreeConcurrent>,
    possible_orders_except_one: &Arc<[PossibleOrder<N>]>,
    exact_piece_count: NonZeroU32,
) -> impl Iterator<Item = TreeThreadInfo> {
    let core_ids_len = core_ids.len();
    let tree_thread_handles = core_ids
        .into_iter()
        .enumerate()
        .map(|(thread_index, core_id)| {
            core_affinity::set_for_current(core_id);
            let mut mutable = mutable.clone();
            let concurrent = Arc::clone(concurrent);
            let possible_orders_except_one = Arc::clone(possible_orders_except_one);
            std::thread::spawn(move || {
                let real_time = Instant::now();
                let cpu_time = ThreadTime::now();

                let exact_register_count = mutable.exact_register_count();
                let maybe_next_remaining_register_count =
                    NonZeroU16::new(exact_register_count.get() - 1);
                if maybe_next_remaining_register_count.is_none() {
                    mutable.prefix_and_last_registers.clear();
                    mutable
                        .prefix_and_last_registers
                        .extend(mutable.registers.split_last().1.iter().copied());
                }
                for (i, possible_order) in possible_orders_except_one
                    .iter()
                    .enumerate()
                    .rev()
                    .skip(thread_index)
                    .step_by(core_ids_len)
                {
                    if i <= concurrent
                        .max_last_register_order
                        .load(atomic::Ordering::Relaxed)
                    {
                        break;
                    }

                    let Some(next_remaining_piece_count) = exact_piece_count
                        .get()
                        .checked_sub(possible_order.min_piece_count.get())
                    else {
                        continue;
                    };

                    let Some(next_remaining_register_count) = maybe_next_remaining_register_count
                    else {
                        mutable.prefix_and_last_registers.push(i);
                        continue;
                    };

                    if let Some(next_remaining_piece_count) =
                        NonZeroU32::new(next_remaining_piece_count)
                        && let Ok(next_possible_orders) =
                            NonemptySlice::try_from(&possible_orders_except_one[..=i])
                    {
                        *mutable.registers.first_mut() = i;
                        unsafe {
                            search_dfs_helper(
                                &mut mutable,
                                &concurrent,
                                next_possible_orders,
                                next_remaining_register_count,
                                next_remaining_piece_count,
                            );
                        }
                    }
                }

                if maybe_next_remaining_register_count.is_none() {
                    mutable.candidate_count += (mutable.prefix_and_last_registers.len() + 1
                        - mutable.registers.len().get())
                        as u32;
                    let maybe_now = log_enabled!(Level::Debug).then(Instant::now);
                    let payload = PackedCycleCombinationCandidateQueue {
                        prefix_and_last_registers: Box::clone_from_ref(
                            &mutable.prefix_and_last_registers,
                        ),
                    };
                    if let Some(now) = maybe_now {
                        mutable.alloc_time += now.elapsed();
                    }
                    mutable.sender.send(payload).unwrap();
                }

                TreeThreadInfo {
                    real_time: real_time.elapsed(),
                    cpu_time: cpu_time.elapsed(),
                    alloc_time: mutable.alloc_time,
                    candidate_count: mutable.candidate_count,
                }
            })
        })
        .collect::<Vec<_>>();
    drop(mutable);
    tree_thread_handles
        .into_iter()
        .map(|tree_thread_handle| tree_thread_handle.join().unwrap())
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
        #[allow(clippy::missing_panics_doc)]
        let core_ids = core_affinity::get_core_ids().unwrap();
        let num_cores = core_ids.len();

        // We do not use `0` as to allow a buffer for every core to prevent starvation
        let (sender, receiver) =
            mpmc::sync_channel::<PackedCycleCombinationCandidateQueue>(core_ids.len() * 10);

        let concurrent = Arc::default();

        // We can unwrap because `exact_register_count` is NonZero.
        #[allow(clippy::missing_panics_doc)]
        let mutable = CycleCombinationsTreeMutable {
            prefix_and_last_registers: vec![],
            registers: NonemptyVec::try_from(vec![0; usize::from(self.exact_register_count.get())])
                .unwrap(),
            sender,
            alloc_time: Duration::default(),
            candidate_count: 0,
        };

        let worker_thread_handles = core_ids
            .iter()
            .map(|&core_id| {
                let receiver = receiver.clone();
                let concurrent = Arc::clone(&concurrent);
                let possible_orders_except_one = Arc::clone(&self.possible_orders_except_one);
                std::thread::spawn(move || {
                    details_thread(
                        core_id,
                        receiver,
                        concurrent,
                        possible_orders_except_one,
                        self.exact_register_count,
                    )
                })
            })
            .collect::<Vec<_>>();

        let real_time = Instant::now();

        let tree_thread_infos = unsafe {
            search_dfs_helper_helper(
                core_ids,
                mutable,
                &concurrent,
                &self.possible_orders_except_one,
                self.exact_piece_count,
            )
        };

        let mut candidate_count = 0;
        let mut dfs_real_time = Duration::default();
        let mut dfs_cpu_time = Duration::default();
        let mut dfs_alloc_time = Duration::default();
        for tree_thread_info in tree_thread_infos {
            candidate_count += tree_thread_info.candidate_count;
            dfs_real_time += tree_thread_info.real_time;
            dfs_cpu_time += tree_thread_info.cpu_time;
            dfs_alloc_time += tree_thread_info.alloc_time;
        }

        let mut smallest_fronts = BinaryHeap::new();
        let mut mkp_cpu_time = Duration::default();
        let mut mkp_real_time = Duration::default();
        let mut post_candidate_count = 0;
        for handle in worker_thread_handles {
            #[allow(clippy::missing_panics_doc)]
            let thread_info = handle.join().unwrap();

            mkp_cpu_time += thread_info.mkp_cpu_time;
            mkp_real_time += thread_info.mkp_real_time;
            post_candidate_count += thread_info.post_candidate_count;
            smallest_fronts.push(thread_info.cycle_combinations);
        }

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

        #[allow(clippy::missing_panics_doc)]
        let exclusive = Arc::into_inner(concurrent).unwrap();

        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        let pruned_orders_percentage = ((exclusive.max_last_register_order.into_inner() + 1)
            as f64)
            / (self.possible_orders_except_one.len() as f64);
        let dfs_io_time = dfs_real_time
            .saturating_sub(dfs_cpu_time)
            .saturating_sub(dfs_alloc_time);
        let mkp_io_time = mkp_real_time.saturating_sub(mkp_cpu_time);

        let profile_info = ProfileInfo {
            candidate_count,
            post_candidate_count,
            pruned_orders_percentage,
            real_time,
            dfs_alloc_time,
            dfs_cpu_time,
            dfs_io_time,
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

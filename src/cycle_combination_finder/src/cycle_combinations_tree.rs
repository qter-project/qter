use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    fmt::{self, Debug},
    num::{NonZeroU16, NonZeroU32},
    slice,
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

pub(crate) struct CycleCombinationsTree<const N: usize> {
    possible_orders_except_one: Arc<[PossibleOrder<N>]>,
    exact_register_count: NonZeroU16,
    exact_piece_count: NonZeroU32,
}

#[derive(Clone)]
struct CycleCombinationsTreeMutable {
    packed_queue: Vec<u32>,
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

    fn send_queue(&mut self) {
        if log_enabled!(Level::Debug) {
            self.candidate_count +=
                (self.packed_queue.len() - usize::from(self.exact_register_count().get())) as u64;
            let now = Instant::now();
            let payload =
                PackedCycleCombinationCandidateQueue(Box::clone_from_ref(&self.packed_queue));
            self.alloc_time += now.elapsed();
            self.sender.send(payload).unwrap();
        } else {
            self.sender
                .send(PackedCycleCombinationCandidateQueue(Box::clone_from_ref(
                    &self.packed_queue,
                )))
                .unwrap();
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
    mut max_last_register_orders: Arc<[AtomicPtr<u32>]>,
    possible_orders_except_one: &[PossibleOrder<N>],
    exact_register_count: NonZeroU16,
) -> DetailsThreadInfo {
    core_affinity::set_for_current(core_id);
    let mut cycle_combinations = CCParetoFront::default();
    let mut processed_candidate_count = 0;
    let mut post_candidate_count = 0;
    let real_time = Instant::now();
    let cpu_time = ThreadTime::now();
    while let Ok(packed_queue) = receiver.recv() {
        let (thread_index_and_prefix_registers, last_registers) = packed_queue
            .0
            .split_at(usize::from(exact_register_count.get()));
        let (&thread_index, prefix_registers) =
            thread_index_and_prefix_registers.split_first().unwrap();
        let thread_index = thread_index as usize;
        for &last_register in last_registers {
            processed_candidate_count += 1;
            let disjoint_registers = DisjointRegisters {
                prefix_registers,
                last_register,
            };
            if cycle_combinations.push_and_dominating_check(
                disjoint_registers,
                |dominating_registers| {
                    post_candidate_count += 1;
                    CycleCombinationDetails::new(dominating_registers, possible_orders_except_one)
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
                let _ = max_last_register_orders[thread_index].try_update(
                    atomic::Ordering::Relaxed,
                    atomic::Ordering::Relaxed,
                    |max_last_register_order| {
                        if !max_last_register_order.is_null() {
                            let max_last_register_order = unsafe {
                                slice::from_raw_parts(
                                    max_last_register_order,
                                    usize::from(exact_register_count.get() - 1),
                                )
                            };
                            let mut max_last_register_order =
                                max_last_register_order.iter().copied();
                            let b = max_last_register_order.next().unwrap();
                            if last_register < b {
                                return None;
                            }
                            if last_register == b {
                                let mut new: Option<Vec<u32>> = None;
                                let mut f = false;
                                for ((i, &p), m) in prefix_registers
                                    .iter()
                                    .enumerate()
                                    .skip(1)
                                    .zip(max_last_register_order)
                                {
                                    if f {
                                        new.as_mut().unwrap().push(p);
                                        continue;
                                    }
                                    match p.cmp(&m) {
                                        Ordering::Less => return None,
                                        Ordering::Equal => (),
                                        Ordering::Greater => {
                                            f = true;
                                            let mut r = Vec::with_capacity(usize::from(
                                                exact_register_count.get() - 1,
                                            ));
                                            r.extend(
                                                std::iter::once(last_register)
                                                    .chain(prefix_registers[1..=i].iter().copied()),
                                            );
                                            new = Some(r);
                                        }
                                    }
                                }

                                // new can still be None here:
                                // A C D can be a solution, followed by B C D
                                return new.map(|new| {
                                    debug_assert_eq!(
                                        new.len(),
                                        usize::from(exact_register_count.get() - 1)
                                    );
                                    Box::into_raw(new.into_boxed_slice()).as_mut_ptr()
                                });
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
    max_last_register_orders: &AtomicPtr<u32>,
    possible_orders_except_one: &[PossibleOrder<N>],
) -> TreeThreadInfo {
    core_affinity::set_for_current(core_id);
    let real_time = Instant::now();
    let cpu_time = ThreadTime::now();

    let exact_register_count = mutable.exact_register_count();
    let maybe_next_remaining_register_count = NonZeroU16::new(exact_register_count.get() - 1);
    if maybe_next_remaining_register_count.is_none() {
        mutable.packed_queue.truncate(1);
        mutable
            .packed_queue
            .extend(mutable.registers.split_last().1.iter().copied());
    }
    let mut old_bucket = 0;
    for (i, possible_order) in possible_orders_except_one
        .iter()
        .enumerate()
        .rev()
        .skip(thread_index)
        .step_by(num_cores)
    {
        let i_u32 = possible_orders_len_cast(i);
        let b = max_last_register_orders.load(atomic::Ordering::Relaxed);
        let max_last_register_order = if b.is_null() {
            0
        } else {
            let max_last_register_order = unsafe { *b };
            if i_u32 <= max_last_register_order {
                break;
            }
            max_last_register_order
        };
        if thread_index == 0 {
            // We validated `possible_orders` to be of len `u32` or less
            let len = possible_orders_len_cast(possible_orders_except_one.len());
            let new_percent = f64::from(len - i_u32) / f64::from(len - max_last_register_order);
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

        let Some(next_remaining_register_count) = maybe_next_remaining_register_count else {
            mutable.packed_queue.push(i_u32);
            continue;
        };

        if let Some(next_remaining_piece_count) = NonZeroU32::new(next_remaining_piece_count)
            && let Ok(next_possible_orders) =
                NonemptySlice::try_from(&possible_orders_except_one[..=i])
        {
            *mutable.registers.first_mut() = i_u32;
            unsafe {
                search_dfs_helper(
                    &mut mutable,
                    max_last_register_orders,
                    next_possible_orders,
                    next_remaining_register_count,
                    next_remaining_piece_count,
                );
            }
        }
    }

    if maybe_next_remaining_register_count.is_none() {
        mutable.send_queue();
    }

    TreeThreadInfo {
        real_time: real_time.elapsed(),
        cpu_time: cpu_time.elapsed(),
        alloc_time: mutable.alloc_time,
        candidate_count: mutable.candidate_count,
    }
}

/// # Safety
///
/// `remaining_register_count` must be less than or equal to
/// `mutable.registers.len()`.
unsafe fn search_dfs_helper<const N: usize>(
    mutable: &mut CycleCombinationsTreeMutable,
    max_last_register_order: &AtomicPtr<u32>,
    possible_orders: NonemptySlice<'_, PossibleOrder<N>>,
    remaining_register_count: NonZeroU16,
    remaining_piece_count: NonZeroU32,
) {
    let register_index = mutable.exact_register_count().get() - remaining_register_count.get();
    let mut curr_possible_orders = possible_orders;
    let maybe_next_remaining_register_count = NonZeroU16::new(remaining_register_count.get() - 1);
    if maybe_next_remaining_register_count.is_none() {
        mutable.packed_queue.truncate(1);
        mutable
            .packed_queue
            .extend(mutable.registers.split_last().1.iter().copied());
    }
    loop {
        let (possible_order, next_possible_orders) = curr_possible_orders.split_last();
        let i = possible_orders_len_cast(next_possible_orders.len());

        let b = max_last_register_order.load(atomic::Ordering::Relaxed);
        if !b.is_null() {
            let l = unsafe { slice::from_raw_parts(b, usize::from(register_index)) };
            let mut l = l.iter();
            if i <= *l.next().unwrap()
                && mutable
                    .registers
                    .iter()
                    .skip(1)
                    .zip(l)
                    .all(|(&r, &l_)| r <= l_)
            {
                break;
            }
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
                        unsafe {
                            mutable
                                .registers
                                .get_unchecked_mut(usize::from(register_index))
                        },
                        i,
                    );
                    // SAFETY: `remaining_register_count` only ever decreases.
                    unsafe {
                        search_dfs_helper(
                            mutable,
                            max_last_register_order,
                            curr_possible_orders,
                            next_remaining_register_count,
                            next_remaining_piece_count,
                        );
                    }
                    // SAFETY: see above.
                    unsafe {
                        *mutable
                            .registers
                            .get_unchecked_mut(usize::from(register_index)) = old;
                    };
                }
            } else {
                mutable.packed_queue.push(i);
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
        mutable.send_queue();
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
        #[allow(clippy::missing_panics_doc)]
        let core_ids = core_affinity::get_core_ids().unwrap();
        let num_cores = core_ids.len();

        // We do not use `0` as to allow a buffer for every core to prevent starvation
        let (sender, receiver) =
            mpmc::sync_channel::<PackedCycleCombinationCandidateQueue>(core_ids.len() * 10);

        // We can unwrap because `exact_register_count` is NonZero.
        #[allow(clippy::missing_panics_doc)]
        let mutable = CycleCombinationsTreeMutable {
            packed_queue: vec![],
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
        let mut processed_candidate_count = 0;
        let mut post_candidate_count = 0;
        let mut smallest_fronts = BinaryHeap::new();

        let max_last_register_orders: Arc<[AtomicPtr<u32>]> = Arc::from(
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
                .zip(max_last_register_orders.iter())
                .map(|((thread_index, core_id), max_last_register_order)| {
                    let mut mutable = mutable.clone();
                    mutable
                        .packed_queue
                        .push(u32::try_from(thread_index).expect("You have too many threads."));
                    let tree_thread_handle = s.spawn(move || {
                        dfs_thread(
                            core_id,
                            thread_index,
                            num_cores,
                            self.exact_piece_count,
                            mutable,
                            max_last_register_order,
                            possible_orders_except_one,
                        )
                    });
                    let receiver = receiver.clone();
                    let max_last_register_orders = Arc::clone(&max_last_register_orders);
                    let details_thread_handle = s.spawn(move || {
                        details_thread(
                            core_id,
                            receiver,
                            max_last_register_orders,
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
        let pruned_orders_percentage = (max_last_register_orders
            .iter()
            .map(|max_last_register_order| {
                let max_last_register_order =
                    max_last_register_order.load(atomic::Ordering::Relaxed);
                u64::from(if max_last_register_order.is_null() {
                    0
                } else {
                    unsafe { *max_last_register_order }
                })
            })
            .sum::<u64>() as f64)
            / ((self.possible_orders_except_one.len() * num_cores) as f64);

        let dfs_io_time = dfs_real_time
            .saturating_sub(dfs_cpu_time)
            .saturating_sub(dfs_alloc_time);
        let mkp_io_time = mkp_real_time.saturating_sub(mkp_cpu_time);

        let profile_info = ProfileInfo {
            candidate_count,
            processed_candidate_count,
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

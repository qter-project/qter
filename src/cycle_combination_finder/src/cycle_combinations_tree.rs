use std::{
    num::{NonZeroU16, NonZeroU32, NonZeroUsize},
    sync::{
        Arc,
        atomic::{self, AtomicUsize},
        mpsc::{self, Sender},
        nonpoison::{Condvar, Mutex},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use humanize_duration::{Truncate, prelude::DurationExt};
use log::{debug, trace};

use crate::{
    cycle_combination_details::CycleCombinationDetails,
    finder::{CycleCombination, PossibleOrder},
    nonemptyvec::{NonemptySlice, NonemptyVec},
    pareto_front::concurrent_pareto_front::ConcurrentCCParetoFront,
    puzzle::OrbitDef,
};

pub struct CycleCombinationsTree<const N: usize> {
    possible_orders_except_one: Vec<PossibleOrder<N>>,
    exact_register_count: NonZeroU16,
    exact_piece_count: NonZeroU32,
}

pub struct CycleCombinationsTreeImmutable<const N: usize> {
    sender: Sender<PackedCycleCombinationCandidateQueue<N>>,
    receiver_thread: JoinHandle<()>,
}

pub struct CycleCombinationsTreeMutable<const N: usize> {
    prefix_and_last_registers: Vec<(PossibleOrder<N>, usize)>,
    registers: NonemptyVec<PossibleOrder<N>>,
    candidate_count: u64,
    waiting_time: Duration,
}

pub struct CycleCombinationsTreeConcurrent<const N: usize> {
    cycle_combinations: ConcurrentCCParetoFront<N>,
    max_last_register_order_reverse_index: AtomicUsize,
    permits: Mutex<usize>,
    search_progression: Condvar,
}

#[derive(Debug, Clone)]
struct PackedCycleCombinationCandidateQueue<const N: usize> {
    prefix_and_last_registers: Box<[(PossibleOrder<N>, usize)]>,
}

#[derive(Clone, Copy)]
pub struct DisjointRegisters<'a, const N: usize> {
    prefix_registers: &'a [(PossibleOrder<N>, usize)],
    last_register: &'a PossibleOrder<N>,
}

impl<const N: usize> DisjointRegisters<'_, N> {
    pub fn iter(&self) -> impl Iterator<Item = &PossibleOrder<N>> {
        self.prefix_registers
            .iter()
            .map(|(prefix_register, _)| prefix_register)
            .chain(std::iter::once(self.last_register))
    }
}

#[allow(unused)]
fn dbg_registers<const N: usize>(registers: &[PossibleOrder<N>]) -> String {
    registers
        .iter()
        .map(|x| u64::try_from(x.order.as_bigint()).unwrap().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

impl<const N: usize> CycleCombinationsTree<N> {
    #[must_use]
    pub fn new(
        exact_register_count: NonZeroU16,
        possible_orders_except_one: Vec<PossibleOrder<N>>,
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

    unsafe fn search_dfs_helper(
        immutable: &CycleCombinationsTreeImmutable<N>,
        mutable: &mut CycleCombinationsTreeMutable<N>,
        concurrent: &Arc<CycleCombinationsTreeConcurrent<N>>,
        possible_orders: &[PossibleOrder<N>],
        remaining_register_count: NonZeroUsize,
        remaining_piece_count: NonZeroU32,
    ) {
        let register_index = mutable.registers.len() - remaining_register_count.get();
        let mut curr_possible_orders = possible_orders;
        let maybe_next_remaining_register_count =
            NonZeroUsize::new(remaining_register_count.get() - 1);
        // let mut good;
        if maybe_next_remaining_register_count.is_none() {
            // TODO: can this be extended and this not used, wasting time?
            mutable.prefix_and_last_registers.clear();
            mutable.prefix_and_last_registers.extend(
                mutable
                    .registers
                    .split_last()
                    .1
                    .iter()
                    .map(|register| (register.clone(), 0)),
            );
        }
        while let Some((possible_order, next_possible_orders)) = curr_possible_orders.split_first()
        {
            if register_index <= 1
                && next_possible_orders.len()
                    <= concurrent
                        .max_last_register_order_reverse_index
                        .load(atomic::Ordering::Relaxed)
            {
                break;
            }

            let Some(next_remaining_piece_count) = remaining_piece_count
                .get()
                .checked_sub(possible_order.min_piece_count.get())
            else {
                curr_possible_orders = next_possible_orders;
                continue;
            };

            if let Some(next_remaining_register_count) = maybe_next_remaining_register_count {
                if let Some(next_remaining_piece_count) =
                    NonZeroU32::new(next_remaining_piece_count)
                {
                    let old = std::mem::replace(
                        unsafe { mutable.registers.get_unchecked_mut(register_index) },
                        possible_order.clone(),
                    );
                    unsafe {
                        Self::search_dfs_helper(
                            immutable,
                            mutable,
                            concurrent,
                            curr_possible_orders,
                            next_remaining_register_count,
                            next_remaining_piece_count,
                        );
                    }
                    *unsafe { mutable.registers.get_unchecked_mut(register_index) } = old;
                }
            } else {
                mutable.candidate_count += 1;
                mutable
                    .prefix_and_last_registers
                    .push((possible_order.clone(), next_possible_orders.len()));
            }
            curr_possible_orders = next_possible_orders;
        }

        if maybe_next_remaining_register_count.is_none() {
            // TODO: should this be inside?
            let mut lock = concurrent.permits.lock();
            if *lock == 0 {
                let now = Instant::now();
                concurrent.search_progression.wait(&mut lock);
                mutable.waiting_time += now.elapsed();
            }
            immutable
                .sender
                .send(PackedCycleCombinationCandidateQueue {
                    prefix_and_last_registers: Box::clone_from_ref(
                        &mutable.prefix_and_last_registers,
                    ),
                })
                .unwrap();
        }
    }

    #[must_use]
    pub fn search_dfs(self) -> Vec<CycleCombination<N>> {
        // We can unwrap because `exact_register_count` is NonZero.
        #[allow(clippy::missing_panics_doc)]
        let mut mutable = CycleCombinationsTreeMutable {
            prefix_and_last_registers: vec![],
            registers: NonemptyVec::try_from(vec![
                PossibleOrder::initialized();
                usize::from(self.exact_register_count.get())
            ])
            .unwrap(),
            candidate_count: 0,
            waiting_time: Duration::default(),
        };

        let mut concurrent = Arc::new(CycleCombinationsTreeConcurrent {
            cycle_combinations: ConcurrentCCParetoFront::default(),
            max_last_register_order_reverse_index: AtomicUsize::new(0),
            permits: Mutex::new(rayon::current_num_threads()),
            search_progression: Condvar::new(),
        });

        let (sender, receiver) = mpsc::channel::<PackedCycleCombinationCandidateQueue<N>>();
        let receiver_thread = {
            let concurrent = Arc::clone(&concurrent);
            std::thread::spawn(move || {
                for packed_queue in receiver {
                    let concurrent = Arc::clone(&concurrent);
                    rayon::spawn(move || {
                        {
                            let mut l = concurrent.permits.lock();
                            trace!(
                                "{:?} just acq: {} -> {}",
                                std::thread::current().id(),
                                l,
                                *l - 1
                            );
                            *l -= 1;
                        }

                        // TODO: unchecked?
                        let (prefix_registers, last_registers) = packed_queue
                            .prefix_and_last_registers
                            .split_at(usize::from(self.exact_register_count.get() - 1));
                        for &(ref last_register, last_register_order_reverse_index) in
                            last_registers
                        {
                            let disjoint_registers = DisjointRegisters {
                                prefix_registers,
                                last_register,
                            };
                            if concurrent.cycle_combinations.push_and_dominating_check(
                                disjoint_registers,
                                |dominating_registers| {
                                    CycleCombinationDetails::new(dominating_registers).map(
                                        |details| CycleCombination {
                                            registers: dominating_registers
                                                .iter()
                                                .cloned()
                                                .collect::<Box<_>>(),
                                            details,
                                        },
                                    )
                                },
                            ) {
                                concurrent.max_last_register_order_reverse_index.fetch_max(
                                    last_register_order_reverse_index,
                                    atomic::Ordering::Relaxed,
                                );
                                break;
                            }
                        }
                        {
                            let mut l = concurrent.permits.lock();
                            if *l == 0 {
                                concurrent.search_progression.notify_one();
                            }
                            trace!(
                                "{:?} just released: {} -> {}",
                                std::thread::current().id(),
                                l,
                                *l + 1
                            );
                            *l += 1;
                        }
                    });
                }
            })
        };

        let immutable = CycleCombinationsTreeImmutable {
            sender,
            receiver_thread,
        };

        let now = Instant::now();

        unsafe {
            Self::search_dfs_helper(
                &immutable,
                &mut mutable,
                &concurrent,
                &self.possible_orders_except_one,
                NonZeroUsize::from(self.exact_register_count),
                self.exact_piece_count,
            );
        }

        debug!(
            "Search tree: iterations={} total={} waiting={}",
            mutable.candidate_count,
            now.elapsed().human(Truncate::Micro),
            mutable.waiting_time.human(Truncate::Micro)
        );

        drop(immutable.sender);

        #[allow(clippy::missing_panics_doc)]
        immutable.receiver_thread.join().unwrap();

        loop {
            match Arc::try_unwrap(concurrent) {
                Ok(exlusive) => break exlusive.cycle_combinations.into_sequential().into(),
                Err(still_concurrent) => {
                    concurrent = still_concurrent;
                    std::thread::yield_now();
                }
            }
        }
    }
}

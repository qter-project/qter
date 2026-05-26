use std::{
    num::{NonZeroU16, NonZeroU32, NonZeroUsize},
    sync::{
        Arc,
        atomic::{self, AtomicU32, AtomicUsize},
        mpmc,
    },
    time::Instant,
};

use core_affinity::CoreId;
use humanize_duration::{Truncate, prelude::DurationExt};
use log::debug;

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

pub struct CycleCombinationsTreeMutable<const N: usize> {
    prefix_and_last_registers: Vec<(PossibleOrder<N>, usize)>,
    registers: NonemptyVec<PossibleOrder<N>>,
    candidate_count: u64,
    sender: mpmc::Sender<PackedCycleCombinationCandidateQueue<N>>,
}

pub struct CycleCombinationsTreeConcurrent<const N: usize> {
    cycle_combinations: ConcurrentCCParetoFront<N>,
    max_last_register_order_reverse_index: AtomicUsize,
    post_candidate_count: AtomicU32,
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

fn worker_thread<const N: usize>(
    core_id: CoreId,
    receiver: &mpmc::Receiver<PackedCycleCombinationCandidateQueue<N>>,
    concurrent: &Arc<CycleCombinationsTreeConcurrent<N>>,
    exact_register_count: NonZeroU16,
) {
    core_affinity::set_for_current(core_id);
    while let Ok(packed_queue) = receiver.recv() {
        let (prefix_registers, last_registers) = packed_queue
            .prefix_and_last_registers
            .split_at(usize::from(exact_register_count.get() - 1));
        for &(ref last_register, last_register_order_reverse_index) in last_registers {
            let disjoint_registers = DisjointRegisters {
                prefix_registers,
                last_register,
            };
            if concurrent.cycle_combinations.push_and_dominating_check(
                disjoint_registers,
                |dominating_registers| {
                    concurrent
                        .post_candidate_count
                        .fetch_add(1, atomic::Ordering::Relaxed);
                    CycleCombinationDetails::new(dominating_registers).map(|details| {
                        CycleCombination {
                            registers: dominating_registers.iter().cloned().collect::<Box<_>>(),
                            details,
                        }
                    })
                },
            ) {
                // Note that we are allowed to set
                // `max_last_register_order_reverse_index` to potentially dominated
                // solutions. If something is the maximum in our atomic variable,
                // then it must either be in the front or the atomic variable is an
                // underestimate, which is permitted since our bound is admissible
                concurrent
                    .max_last_register_order_reverse_index
                    .fetch_max(last_register_order_reverse_index, atomic::Ordering::Relaxed);
                break;
            }
        }
    }
}

unsafe fn search_dfs_helper<const N: usize>(
    mutable: &mut CycleCombinationsTreeMutable<N>,
    concurrent: &Arc<CycleCombinationsTreeConcurrent<N>>,
    possible_orders: &[PossibleOrder<N>],
    remaining_register_count: NonZeroUsize,
    remaining_piece_count: NonZeroU32,
) {
    let register_index = mutable.registers.len() - remaining_register_count.get();
    let mut curr_possible_orders = possible_orders;
    let maybe_next_remaining_register_count = NonZeroUsize::new(remaining_register_count.get() - 1);
    let mut send_queue = false;
    while let Some((possible_order, next_possible_orders)) = curr_possible_orders.split_first() {
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
            if let Some(next_remaining_piece_count) = NonZeroU32::new(next_remaining_piece_count) {
                let old = std::mem::replace(
                    unsafe { mutable.registers.get_unchecked_mut(register_index) },
                    possible_order.clone(),
                );
                unsafe {
                    search_dfs_helper(
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

            if !send_queue {
                // Initialize in here because a puzzle with no orientations at all can have a
                // possible order that is not one, which may cause no solutions to be found at
                // this leaf node
                mutable.prefix_and_last_registers.clear();
                mutable.prefix_and_last_registers.extend(
                    mutable
                        .registers
                        .split_last()
                        .1
                        .iter()
                        .map(|register| (register.clone(), 0)),
                );
                send_queue = true;
            }
            mutable
                .prefix_and_last_registers
                .push((possible_order.clone(), next_possible_orders.len()));
        }
        curr_possible_orders = next_possible_orders;
    }

    if send_queue {
        mutable
            .sender
            .send(PackedCycleCombinationCandidateQueue {
                prefix_and_last_registers: Box::clone_from_ref(&mutable.prefix_and_last_registers),
            })
            .unwrap();
    }
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

    #[must_use]
    pub fn search_dfs(self) -> Vec<CycleCombination<N>> {
        #[allow(clippy::missing_panics_doc)]
        let core_ids = core_affinity::get_core_ids().unwrap();

        // We do not use `0` as to allow a buffer for every core to prevent starvation
        let (sender, receiver) =
            mpmc::sync_channel::<PackedCycleCombinationCandidateQueue<N>>(core_ids.len());

        let concurrent = Arc::new(CycleCombinationsTreeConcurrent {
            post_candidate_count: AtomicU32::new(0),
            cycle_combinations: ConcurrentCCParetoFront::default(),
            max_last_register_order_reverse_index: AtomicUsize::new(0),
        });

        let worker_thread_handles = core_ids
            .into_iter()
            .map(|core_id| {
                let receiver = receiver.clone();
                let concurrent = Arc::clone(&concurrent);
                std::thread::spawn(move || {
                    worker_thread(core_id, &receiver, &concurrent, self.exact_register_count);
                })
            })
            .collect::<Vec<_>>();

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
            sender,
        };

        let total_time = Instant::now();

        unsafe {
            search_dfs_helper(
                &mut mutable,
                &concurrent,
                &self.possible_orders_except_one,
                NonZeroUsize::from(self.exact_register_count),
                self.exact_piece_count,
            );
        }

        let dfs_time = total_time.elapsed();

        drop(mutable.sender);
        for handle in worker_thread_handles {
            #[allow(clippy::missing_panics_doc)]
            handle.join().unwrap();
        }

        #[allow(clippy::missing_panics_doc)]
        let exclusive = Arc::into_inner(concurrent).unwrap();
        debug!(
            "Search tree complete: candidates={}; post_candidates={}; total={}; dfs={}",
            mutable.candidate_count,
            exclusive.post_candidate_count.into_inner(),
            total_time.elapsed().human(Truncate::Micro),
            dfs_time.human(Truncate::Micro),
        );
        exclusive.cycle_combinations.into_sequential().into()
    }
}

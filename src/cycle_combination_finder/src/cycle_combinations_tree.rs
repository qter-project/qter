use std::{
    num::{NonZeroU16, NonZeroU32, NonZeroUsize},
    sync::{
        Arc,
        atomic::{self, AtomicUsize},
        mpsc::{self, Sender},
        nonpoison::{Condvar, Mutex},
    },
    thread::JoinHandle,
};

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
    sender: Sender<CycleCombinationCandidate<N>>,
    receiver_thread: JoinHandle<()>,
}

pub struct CycleCombinationsTreeMutable<const N: usize> {
    registers: NonemptyVec<PossibleOrder<N>>,
    candidate_count: u64,
}

pub struct CycleCombinationsTreeConcurrent<const N: usize> {
    cycle_combinations: ConcurrentCCParetoFront<N>,
    max_last_register_reverse_index: AtomicUsize,
    permits: Mutex<usize>,
    search_progression: Condvar,
}

#[derive(Debug, Clone)]
struct CycleCombinationCandidate<const N: usize> {
    head: bool,
    registers: Box<[PossibleOrder<N>]>,
    last_register_reverse_index: usize,
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
        self2: &CycleCombinationsTreeImmutable<N>,
        mutable: &mut CycleCombinationsTreeMutable<N>,
        concurrent: &Arc<CycleCombinationsTreeConcurrent<N>>,
        remaining_possible_orders_except_one: &[PossibleOrder<N>],
        remaining_register_count: NonZeroUsize,
        remaining_piece_count: NonZeroU32,
    ) {
        let register_index = mutable.registers.len() - remaining_register_count.get();
        let mut curr_possible_orders = remaining_possible_orders_except_one;
        let mut head = true;
        while let Some((possible_order, next_possible_orders)) = curr_possible_orders.split_first()
        {
            if register_index <= 1
                && next_possible_orders.len()
                    <= concurrent
                        .max_last_register_reverse_index
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

            if let Some(next_remaining_register_count) =
                NonZeroUsize::new(remaining_register_count.get() - 1)
            {
                if let Some(next_remaining_piece_count) =
                    NonZeroU32::new(next_remaining_piece_count)
                {
                    let old = std::mem::replace(
                        unsafe { mutable.registers.get_unchecked_mut(register_index) },
                        possible_order.clone(),
                    );
                    unsafe {
                        Self::search_dfs_helper(
                            self2,
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
                // SAFETY: `register_index`
                let old = std::mem::replace(
                    unsafe { mutable.registers.get_unchecked_mut(register_index) },
                    possible_order.clone(),
                );
                mutable.candidate_count += 1;
                self2
                    .sender
                    .send(CycleCombinationCandidate {
                        head,
                        registers: Box::clone_from_ref(&mutable.registers),
                        last_register_reverse_index: next_possible_orders.len(),
                    })
                    .unwrap();
                head = false;
                {
                    let mut lock = concurrent.permits.lock();
                    if *lock == 0 {
                        concurrent.search_progression.wait(&mut lock);
                    }
                }
                *unsafe { mutable.registers.get_unchecked_mut(register_index) } = old;
            }
            curr_possible_orders = next_possible_orders;
        }
    }

    #[must_use]
    pub fn search_dfs(self) -> Vec<CycleCombination<N>> {
        // We can unwrap because `exact_register_count` is NonZero.
        #[allow(clippy::missing_panics_doc)]
        let mut mutable = CycleCombinationsTreeMutable {
            registers: NonemptyVec::try_from(vec![
                PossibleOrder::initialized();
                usize::from(self.exact_register_count.get())
            ])
            .unwrap(),
            candidate_count: 0,
        };

        let mut concurrent = Arc::new(CycleCombinationsTreeConcurrent {
            cycle_combinations: ConcurrentCCParetoFront::default(),
            max_last_register_reverse_index: AtomicUsize::new(0),
            permits: Mutex::new(rayon::current_num_threads()),
            search_progression: Condvar::new(),
        });

        let (sender, receiver) = mpsc::channel::<CycleCombinationCandidate<N>>();
        let receiver_thread = {
            let concurrent = Arc::clone(&concurrent);
            std::thread::spawn(move || {
                let mut search_queue: Vec<CycleCombinationCandidate<N>> = vec![];
                for candidate in receiver {
                    if candidate.head && !search_queue.is_empty() {
                        {
                            // TODO: ensure it actually waits since there are 14 threads not 12
                            let search_queue = search_queue.clone();
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
                                // println!(
                                //     "---\n{:?}\n{}\n---",
                                //     std::thread::current().id(),
                                //     q.iter()
                                //         .map(|i| dbg_registers(i))
                                //         .collect::<Vec<_>>()
                                //         .join("\n")
                                // );
                                for search_queue_candidate in search_queue {
                                    if concurrent.cycle_combinations.push_and_dominating_check(
                                        search_queue_candidate.registers,
                                        |dominating_registers| {
                                            match CycleCombinationDetails::try_from(
                                                &*dominating_registers,
                                            ) {
                                                Ok(details) => Ok(CycleCombination {
                                                    registers: dominating_registers,
                                                    details,
                                                }),
                                                Err(()) => Err(dominating_registers),
                                            }
                                        },
                                    ) {
                                        concurrent.max_last_register_reverse_index.update(
                                            atomic::Ordering::Relaxed,
                                            atomic::Ordering::Relaxed,
                                            |curr_max| {
                                                curr_max.max(
                                                    search_queue_candidate
                                                        .last_register_reverse_index,
                                                )
                                            },
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
                        search_queue.clear();
                    }
                    search_queue.push(candidate);
                }
            })
        };

        let self2 = CycleCombinationsTreeImmutable {
            sender,
            receiver_thread,
        };

        unsafe {
            Self::search_dfs_helper(
                &self2,
                &mut mutable,
                &concurrent,
                &self.possible_orders_except_one,
                NonZeroUsize::from(self.exact_register_count),
                self.exact_piece_count,
            );
        }
        debug!("Cycle combinations in {} iterations", mutable.candidate_count);

        #[allow(clippy::missing_panics_doc)]
        self2
            .sender
            .send(CycleCombinationCandidate {
                head: true,
                registers: vec![].into_boxed_slice(),
                last_register_reverse_index: 0,
            })
            .unwrap();
        drop(self2.sender);

        #[allow(clippy::missing_panics_doc)]
        self2.receiver_thread.join().unwrap();

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

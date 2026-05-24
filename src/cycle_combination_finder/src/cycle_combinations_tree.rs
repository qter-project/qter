use std::num::{NonZeroU16, NonZeroU32, NonZeroUsize};

use log::debug;

use crate::{
    cycle_combination_details::CycleCombinationDetails,
    finder::{CycleCombination, PossibleOrder},
    nonemptyvec::{NonemptySlice, NonemptyVec},
    pareto_front::pareto_front::CCParetoFront,
    puzzle::OrbitDef,
};

pub struct CycleCombinationsTree<const N: usize> {
    possible_orders_except_one: Vec<PossibleOrder<N>>,
    exact_register_count: NonZeroU16,
    exact_piece_count: NonZeroU32,
}

pub struct CycleCombinationsTreeMutable<const N: usize> {
    registers: NonemptyVec<PossibleOrder<N>>,
    max_last_register_reverse_index: usize,
    iter_count: u64,
    cycle_combinations: CCParetoFront<N>,
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
        mutable: &mut CycleCombinationsTreeMutable<N>,
        remaining_possible_orders_except_one: &[PossibleOrder<N>],
        remaining_register_count: NonZeroUsize,
        remaining_piece_count: NonZeroU32,
    ) {
        let register_index = mutable.registers.len() - remaining_register_count.get();
        let mut curr_possible_orders = remaining_possible_orders_except_one;
        while let Some((possible_order, next_possible_orders)) = curr_possible_orders.split_first()
        {
            if register_index <= 1
                && next_possible_orders.len() <= mutable.max_last_register_reverse_index
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
                            mutable,
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
                mutable.iter_count += 1;
                if mutable.cycle_combinations.push_and_dominating_check(
                    Box::clone_from_ref(&mutable.registers),
                    |dominating_registers| match CycleCombinationDetails::try_from(
                        &*dominating_registers,
                    ) {
                        Ok(details) => Ok(CycleCombination {
                            registers: dominating_registers,
                            details,
                        }),
                        Err(()) => Err(dominating_registers),
                    },
                ) {
                    mutable.max_last_register_reverse_index = mutable
                        .max_last_register_reverse_index
                        .max(next_possible_orders.len());
                    break;
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
            max_last_register_reverse_index: 0,
            iter_count: 0,
            cycle_combinations: CCParetoFront::default(),
        };
        unsafe {
            Self::search_dfs_helper(
                &mut mutable,
                &self.possible_orders_except_one,
                NonZeroUsize::from(self.exact_register_count),
                self.exact_piece_count,
            );
        }
        debug!("Cycle combinations in {} iterations", mutable.iter_count);
        Vec::from(mutable.cycle_combinations)
    }
}

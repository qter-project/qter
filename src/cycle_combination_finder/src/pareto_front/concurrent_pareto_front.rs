use std::cell::UnsafeCell;

use thread_local::ThreadLocal;

use crate::{
    finder::{CycleCombination, PossibleOrder},
    pareto_front::pareto_front::CCParetoFront,
};

#[derive(Default)]
pub struct ConcurrentCCParetoFront<const N: usize> {
    inner: ThreadLocal<UnsafeCell<CCParetoFront<N>>>,
}

impl<const N: usize> ConcurrentCCParetoFront<N> {
    /// Adds `new_element` to the Pareto front.
    /// Returns `true` if the element *might be* in the Pareto front.
    /// Returns `false` if the element was dominated and, thus, not added to the
    /// front.
    ///
    /// This operation has `O(n/t)` complexity
    /// where `n` is the number of elements currently in the Pareto front
    /// and `t` the number of threads used.
    /// It is cache friendly and optimized to favour early stopping.
    ///
    /// Note that this operation does *not* use any interior paralelism.
    /// Rather, it is meant to be called in parallel.
    pub fn push_and_dominating_check(
        &self,
        registers: (&[(PossibleOrder<N>, usize)], &PossibleOrder<N>),
        dominating_check: impl FnMut(
            (&[(PossibleOrder<N>, usize)], &PossibleOrder<N>),
        ) -> Option<CycleCombination<N>>,
    ) -> bool {
        // gets a mutable *pointer* to the Pareto front associated with the current
        // thread
        let front = self.inner.get_or_default().get();
        // converts the pointer into a mutable reference
        // Note: safe because only one thread can access a thread-local front
        //       this has been validated with a RefCell
        let front = unsafe { &mut *front };
        // push the new element in the Pareto front
        front.push_and_dominating_check(registers, dominating_check)
    }

    /// Turns the concurrent Pareto front into a, sequential, `ParetoFront`.
    ///
    /// This operation has complexity `O(n²)`
    /// where `n` is the size of the Pareto front.
    ///
    /// Note that this operation does *not* use any interior paralelism.
    pub fn into_sequential(self) -> CCParetoFront<N> {
        // NOTE: this could be turned into a parallel reduce
        //       but, tests with `rayon` did not bring any significant speed benefits
        //       however, paralelism might become beneficial on a large (16+) number of
        // cores
        self.inner
                .into_iter()
                .map(std::cell::UnsafeCell::into_inner) // remove UnsafeCells
                .reduce(|mut front_acc, front| {
                    // merge all fronts into one
                    front_acc.merge(front);
                    front_acc
                })
                .unwrap_or_default() // returns an empty front if there was no thread-local front
    }
}

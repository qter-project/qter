//! Build a [Pareto front](https://en.wikipedia.org/wiki/Pareto_front) incrementaly. Based on the [pareto_front](https://crates.io/crates/pareto_front) crate.

use std::sync::nonpoison::RwLock;

use crate::finder::PossibleOrder;

/// Used to define a pseudo-ordering for multi-dimensional optimization.
pub trait CycleCombinationDominate<const N: usize> {
    /// Returns `true` if we are better (which might be superior or inferior
    /// depending on the specification) than `x` along all dimenssions.
    /// By convention, it usually returns `false` if `x` is equal to `self`.
    ///
    /// Note that we assume the first element doesn't exist.
    fn dominate(&self, registers_except_first: &[PossibleOrder<N>]) -> bool;
}

#[derive(Debug)]
pub struct ConcurrentCCParetoFront<const N: usize, T> {
    inner: RwLock<Vec<T>>,
}

impl<const N: usize, T: CycleCombinationDominate<N>> ConcurrentCCParetoFront<N, T> {
    pub fn push_and_dominating_check(
        &self,
        registers: Box<[PossibleOrder<N>]>,
        dominating_check: impl FnOnce(Box<[PossibleOrder<N>]>) -> Option<T>,
    ) -> bool {
        {
            let rg = self.inner.read();
            for (i, element) in rg.iter().enumerate() {
                if element.dominate(&registers) {
                    drop(rg);
                    // `new_element` is dominated by `element`, it is thus not part of the Pareto
                    // front swap `element` with the previous element in order to
                    // percolate the best elements to the top NOTE: in my benchmarks
                    // this brings clear performance benefits by putting "killer" elements first
                    if i > 0 {
                        // SAFETY: `i` is in range, and `i - 1` must also be in range because of the
                        // if note that this was not optimizing the bounds
                        // check
                        unsafe {
                            self.inner.write().swap_unchecked(i, i - 1);
                        }
                    }
                    return false;
                }
                // We never run into a case where `new_element` dominates
                // `element` causing us to have to do extra
                // logic because we always add strictly lesser
                // orders
                //
                // TODO: explain why this is still fine
                // even when the new possible order is equal
            }
        }

        if let Some(candidate) = (dominating_check)(registers) {
            // `new_element` has not been dominated; it is thus part of the Pareto front
            self.inner.write().push(candidate);
            true
        } else {
            false
        }
    }
}

impl<const N: usize, T: CycleCombinationDominate<N>> Default for ConcurrentCCParetoFront<N, T> {
    fn default() -> Self {
        Self {
            inner: RwLock::default(),
        }
    }
}

impl<const N: usize, T: CycleCombinationDominate<N>> From<ConcurrentCCParetoFront<N, T>>
    for Vec<T>
{
    /// Converts the Pareto front into a vector.
    fn from(ConcurrentCCParetoFront { inner }: ConcurrentCCParetoFront<N, T>) -> Vec<T> {
        inner.into_inner()
    }
}

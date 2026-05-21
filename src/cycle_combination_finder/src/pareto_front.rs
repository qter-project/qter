//! Build a [Pareto front](https://en.wikipedia.org/wiki/Pareto_front) incrementaly. Based on the [pareto_front](https://crates.io/crates/pareto_front) crate.

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

#[derive(Clone, Debug)]
pub struct CycleCombinationParetoFront<const N: usize, T> {
    inner: Vec<T>,
}

impl<const N: usize, T: CycleCombinationDominate<N>> CycleCombinationParetoFront<N, T> {
    pub fn push_and_dominating_check(
        &mut self,
        registers_except_first: &[PossibleOrder<N>],
        dominating_check: impl FnOnce(&[PossibleOrder<N>]) -> Option<T>,
    ) -> bool {
        for (i, element) in self.inner.iter().enumerate() {
            if element.dominate(registers_except_first) {
                // `new_element` is dominated by `element`, it is thus not part of the Pareto
                // front swap `element` with the previous element in order to
                // percolate the best elements to the top NOTE: in my benchmarks
                // this brings clear performance benefits by putting "killer" elements first
                if i > 0 {
                    // SAFETY: `i` is in range, and `i - 1` must also be in range because of the if
                    // note that this was not optimizing the bounds check
                    unsafe {
                        self.inner.swap_unchecked(i, i - 1);
                    }
                }
                return false;
            }
            // We never run into a case where `new_element` dominates `element`
            // causing us to have to do extra logic because we always add
            // strictly lesser orders
        }

        if let Some(candidate) = (dominating_check)(registers_except_first) {
            // `new_element` has not been dominated; it is thus part of the Pareto front
            self.inner.push(candidate);
            true
        } else {
            false
        }
    }
}

impl<const N: usize, T: CycleCombinationDominate<N>> Default for CycleCombinationParetoFront<N, T> {
    fn default() -> Self {
        Self { inner: vec![] }
    }
}

impl<const N: usize, T: CycleCombinationDominate<N>> From<CycleCombinationParetoFront<N, T>>
    for Vec<T>
{
    /// Converts the Pareto front into a vector.
    fn from(CycleCombinationParetoFront { inner }: CycleCombinationParetoFront<N, T>) -> Vec<T> {
        inner
    }
}

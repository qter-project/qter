//! Build a [Pareto front](https://en.wikipedia.org/wiki/Pareto_front) incrementaly. Based on the [pareto_front](https://crates.io/crates/pareto_front) crate.

/// Used to define a pseudo-ordering for multi-dimensional optimization.
pub trait Dominate {
    /// Returns `true` if we are better (which might be superior or inferior
    /// depending on the specification) than `x` along all dimenssions.
    /// By convention, it usually returns `false` if `x` is equal to `self`.
    fn dominate(&self, x: &Self) -> bool;
}

#[derive(Clone, Debug)]
pub struct CandidateParetoFront<T> {
    inner: Vec<T>,
}

impl<T: Dominate> CandidateParetoFront<T> {
    pub fn push_and_dominating_check(
        &mut self,
        mut new_element: T,
        mut check: impl FnMut(&mut T) -> bool,
    ) -> bool {
        for (i, element) in self.inner.iter().enumerate() {
            if element.dominate(&new_element) {
                // `new_element` is dominated by `element`, it is thus not part of the Pareto
                // front swap `element` with the previous element in order to
                // percolate the best elements to the top NOTE: in my benchmarks
                // this brings clear performance benefits by putting "killer" elements first
                if i > 0 {
                    self.inner.swap(i, i - 1);
                }
                return false;
            }
            // We never run into a case where `new_element` dominates `element`
            // causing us to have to do extra logic because we always add
            // strictly lesser orders
        }

        if (check)(&mut new_element) {
            // `new_element` has not been dominated; it is thus part of the Pareto front
            self.inner.push(new_element);
            true
        } else {
            false
        }
    }
}

impl<T: Dominate> Default for CandidateParetoFront<T> {
    fn default() -> Self {
        Self { inner: vec![] }
    }
}

impl<T: Dominate> From<CandidateParetoFront<T>> for Vec<T> {
    /// Converts the Pareto front into a vector.
    fn from(CandidateParetoFront { inner }: CandidateParetoFront<T>) -> Vec<T> {
        inner
    }
}

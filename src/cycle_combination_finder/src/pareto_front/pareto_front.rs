//! Build a [Pareto front](https://en.wikipedia.org/wiki/Pareto_front) incrementaly. Based on the [pareto_front](https://crates.io/crates/pareto_front) crate.

use crate::{
    cycle_combinations_tree::DisjointRegisters,
    finder::{CycleCombination, PossibleOrder},
};

#[derive(Debug, Default)]
pub struct CCParetoFront<const N: usize> {
    inner: Vec<CycleCombination<N>>,
    index_dominated_elements: Vec<usize>,
}

fn dominate<'a, const N: usize>(
    // dominating: &[PossibleOrder<N>],
    // to_dominate: &[PossibleOrder<N>],
    dominating: impl IntoIterator<Item = &'a PossibleOrder<N>>,
    to_dominate: impl IntoIterator<Item = &'a PossibleOrder<N>>,
    skip_first: bool,
) -> bool {
    // TODO: uncomment
    // sanity check
    // debug_assert_eq!(dominating.len(), to_dominate.len());
    // debug_assert!(!skip_first || dominating[0].order >= to_dominate[0].order);

    dominating
        .into_iter()
        .zip(to_dominate)
        .skip(usize::from(skip_first))
        .all(|(d, t)| d.order >= t.order)
}

impl<const N: usize> CCParetoFront<N> {
    /// Removes all elements in the front that are dominated by `new_element`,
    /// starting at index `index_start`.
    fn remove_dominated_starting_at(&mut self, registers: &[PossibleOrder<N>], start: usize) {
        // lists all elements dominated by `new_element`, starting at index
        // `index_start`
        let available_elements = self.inner.len().saturating_sub(start);
        if self.index_dominated_elements.len() < available_elements {
            self.index_dominated_elements.resize(available_elements, 0);
        }
        let mut len = 0;
        for (i, member) in self.inner.iter().enumerate().skip(start) {
            if dominate(registers, &member.registers, false) {
                self.index_dominated_elements[len] = i;
                len += 1;
            }
        }

        // removes the elements at the listed indexes
        // in reverse order to take into acount that each removed index shift all the
        // following indexes
        for &i in self.index_dominated_elements.iter().take(len).rev() {
            self.inner.swap_remove(i);
        }
    }

    pub fn push_and_dominating_check(
        &mut self,
        registers: DisjointRegisters<N>,
        mut dominating_check: impl FnMut(DisjointRegisters<N>) -> Option<CycleCombination<N>>,
    ) -> bool {
        for (i, member) in self.inner.iter().enumerate() {
            if dominate(&member.registers, registers.iter(), true) {
                // `new_element` is dominated by `element`, it is thus not part of the Pareto
                // front swap `element` with the previous element in order to
                // percolate the best elements to the top NOTE: in my benchmarks
                // this brings clear performance benefits by putting "killer" elements first
                if i > 0 {
                    // SAFETY: `i` is in range, and `i - 1` must also be in range because of the
                    // if note that this was not optimizing the bounds
                    // check
                    unsafe {
                        self.inner.swap_unchecked(i, i - 1);
                    }
                }
                return false;
            } else if dominate(registers.iter(), &member.registers, false)
                && let Some(cycle_combination) = (dominating_check)(registers)
            {
                // `new_element` dominates `element`, it is thus part of the Pareto front
                self.inner.swap_remove(i);
                // looks at the rest of the Pareto front to remove any further element that
                // are dominated
                self.remove_dominated_starting_at(&cycle_combination.registers, i);
                self.inner.push(cycle_combination);
                return true;
            }
        }

        if let Some(candidate) = (dominating_check)(registers) {
            // `new_element` has not been dominated; it is thus part of the Pareto front
            self.inner.push(candidate);
            true
        } else {
            false
        }
    }

    fn remove_dominated(&mut self, registers: &[PossibleOrder<N>]) -> bool {
        for (i, member) in self.inner.iter().enumerate() {
            if dominate(&member.registers, registers, false) {
                if i > 0 {
                    unsafe {
                        self.inner.swap_unchecked(i, i - 1);
                    }
                }
                return false;
            } else if dominate(registers, &member.registers, false) {
                self.inner.swap_remove(i);
                self.remove_dominated_starting_at(registers, i);
                return true;
            }
        }
        true
    }

    /// Adds the content of `pareto_front` to the Pareto front.
    ///
    /// This operation has `O(n*m)` complexity
    /// where `n` is the number of elements in `self`
    /// and `m` is the number of elements in `pareto_front`
    /// but is optimized to favour early stopping.
    pub fn merge(&mut self, other_pareto_front: Self) {
        // set the largest front aside
        let mut largest_front = other_pareto_front.inner;
        if largest_front.len() < self.inner.len() {
            std::mem::swap(&mut self.inner, &mut largest_front);
        }
        // for all the elements in the largest front, remove dominated elements from the
        // smallest front the largest front keeps only the elements that should be in
        // the final Pareto front
        largest_front.retain(|x| self.remove_dominated(&x.registers));
        // extends the largest front with the content of the smallest front
        // and make it our front
        std::mem::swap(&mut self.inner, &mut largest_front);
        self.inner.extend(largest_front);
    }
}

impl<const N: usize> From<CCParetoFront<N>> for Vec<CycleCombination<N>> {
    /// Converts the Pareto front into a vector.
    fn from(
        CCParetoFront {
            inner,
            index_dominated_elements: _,
        }: CCParetoFront<N>,
    ) -> Vec<CycleCombination<N>> {
        inner
    }
}

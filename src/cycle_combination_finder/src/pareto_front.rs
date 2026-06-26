//! Build a [Pareto front](https://en.wikipedia.org/wiki/Pareto_front) incrementaly. Based on the [pareto_front](https://crates.io/crates/pareto_front) crate.

use std::cmp::Ordering;

use crate::{cycle_combinations_tree::DisjointRegisters, finder::CycleCombination};

#[derive(Debug, Default)]
pub(crate) struct CCParetoFront {
    pub(crate) inner: Vec<CycleCombination>,
    index_dominated_elements: Vec<usize>,
}

impl Ord for CCParetoFront {
    fn cmp(&self, other: &Self) -> Ordering {
        self.inner.len().cmp(&other.inner.len())
    }
}

impl PartialOrd for CCParetoFront {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for CCParetoFront {}

impl PartialEq for CCParetoFront {
    fn eq(&self, other: &Self) -> bool {
        self.inner.len() == other.inner.len()
    }
}

fn dominate(
    dominating: impl IntoIterator<Item = u32>,
    to_dominate: impl IntoIterator<Item = u32>,
) -> bool {
    if cfg!(debug_assertions) {
        let mut dominating_iter = dominating.into_iter();
        let mut to_dominate_iter = to_dominate.into_iter();
        loop {
            match (dominating_iter.next(), to_dominate_iter.next()) {
                (Some(d), Some(t)) => {
                    if d < t {
                        return false;
                    }
                }
                (None, None) => break,
                _ => panic!("mismatched lengths"),
            }
        }
        true
    } else {
        dominating.into_iter().zip(to_dominate).all(|(d, t)| d >= t)
    }
}

impl CCParetoFront {
    /// Removes all elements in the front that are dominated by `new_element`,
    /// starting at index `index_start`.
    fn remove_dominated_starting_at(&mut self, registers: &[u32], start: usize) {
        // lists all elements dominated by `new_element`, starting at index
        // `index_start`
        let available_elements = self.inner.len().saturating_sub(start);
        if self.index_dominated_elements.len() < available_elements {
            self.index_dominated_elements.resize(available_elements, 0);
        }
        let mut len = 0;
        for (i, member) in self.inner.iter().enumerate().skip(start) {
            if dominate(
                registers.iter().copied(),
                member.inner.registers.iter().copied(),
            ) {
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

    pub fn push(&mut self, existing: CycleCombination) -> bool {
        for (i, member) in self.inner.iter().enumerate() {
            if dominate(
                member.inner.registers.iter().copied(),
                existing.inner.registers.iter().copied(),
            ) {
                // `new_element` is dominated by `element`, it is thus not part of the Pareto
                // front swap `element` with the previous element in order to
                // percolate the best elements to the top NOTE: in my benchmarks
                // this brings clear performance benefits by putting "killer" elements first
                if i > 0 {
                    // SAFETY: `i` is in range, and `i - 1` must also be in range because of the
                    // if. Note that the safe version was not optimizing the bounds check
                    unsafe {
                        self.inner.swap_unchecked(i, i - 1);
                    }
                }
                return false;
            } else if dominate(
                existing.inner.registers.iter().copied(),
                member.inner.registers.iter().copied(),
            ) {
                // `new_element` dominates `element`, it is thus part of the Pareto front
                self.inner.swap_remove(i);
                // looks at the rest of the Pareto front to remove any further element that
                // are dominated
                self.remove_dominated_starting_at(&existing.inner.registers, i);
                break;
            }
        }

        self.inner.push(existing);
        true
    }

    pub fn push_and_dominating_check(
        &mut self,
        registers: DisjointRegisters,
        mut dominating_check: impl FnMut(DisjointRegisters) -> Option<CycleCombination>,
    ) -> bool {
        for (i, member) in self.inner.iter().enumerate() {
            if dominate(member.inner.registers.iter().copied(), registers.iter()) {
                // `new_element` is dominated by `element`, it is thus not part of the Pareto
                // front swap `element` with the previous element in order to
                // percolate the best elements to the top NOTE: in my benchmarks
                // this brings clear performance benefits by putting "killer" elements first
                if i > 0 {
                    // SAFETY: `i` is in range, and `i - 1` must also be in range because of the
                    // if. Note that the safe version was not optimizing the bounds check
                    unsafe {
                        self.inner.swap_unchecked(i, i - 1);
                    }
                }
                return false;
            } else if dominate(registers.iter(), member.inner.registers.iter().copied())
                && let Some(cycle_combination) = (dominating_check)(registers)
            {
                // `new_element` dominates `element`, it is thus part of the Pareto front
                self.inner.swap_remove(i);
                // looks at the rest of the Pareto front to remove any further element that
                // are dominated
                self.remove_dominated_starting_at(&cycle_combination.inner.registers, i);
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

    fn remove_dominated(&mut self, registers: &[u32]) -> bool {
        for (i, member) in self.inner.iter().enumerate() {
            if dominate(
                member.inner.registers.iter().copied(),
                registers.iter().copied(),
            ) {
                if i > 0 {
                    unsafe {
                        self.inner.swap_unchecked(i, i - 1);
                    }
                }
                return false;
            } else if dominate(
                registers.iter().copied(),
                member.inner.registers.iter().copied(),
            ) {
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
        largest_front.retain(|x| self.remove_dominated(&x.inner.registers));
        // extends the largest front with the content of the smallest front
        // and make it our front
        std::mem::swap(&mut self.inner, &mut largest_front);
        self.inner.extend(largest_front);
    }
}

impl From<CCParetoFront> for Vec<CycleCombination> {
    /// Converts the Pareto front into a vector.
    fn from(
        CCParetoFront {
            inner,
            index_dominated_elements: _,
        }: CCParetoFront,
    ) -> Vec<CycleCombination> {
        inner
    }
}

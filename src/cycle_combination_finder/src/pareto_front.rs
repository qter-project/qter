//! Build a [Pareto front](https://en.wikipedia.org/wiki/Pareto_front) incrementaly. Based on the [pareto_front](https://crates.io/crates/pareto_front) crate.

use std::{cmp::Ordering, sync::Arc};

use crate::cycle_combinations_tree::DisjointRegisters;

#[derive(Debug, Default)]
pub(crate) struct CCParetoFront(pub(crate) Vec<Arc<[u32]>>);

impl Ord for CCParetoFront {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.len().cmp(&other.0.len())
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
        self.0.len() == other.0.len()
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
        let mut write_idx = start;
        for read_idx in start..self.0.len() {
            let member = &self.0[read_idx];
            if !dominate(registers.iter().copied(), member.iter().copied()) {
                self.0[write_idx] = Arc::clone(&self.0[read_idx]);
                write_idx += 1;
            }
        }
        self.0.truncate(write_idx);
    }

    pub fn push(&mut self, existing: Arc<[u32]>) -> bool {
        for (i, member) in self.0.iter().enumerate() {
            if dominate(member.iter().copied(), existing.iter().copied()) {
                // `new_element` is dominated by `element`, it is thus not part of the Pareto
                // front swap `element` with the previous element in order to
                // percolate the best elements to the top NOTE: in my benchmarks
                // this brings clear performance benefits by putting "killer" elements first
                if i > 0 {
                    // SAFETY: `i` is in range, and `i - 1` must also be in range because of the
                    // if. Note that the safe version was not optimizing the bounds check
                    unsafe {
                        self.0.swap_unchecked(i, i - 1);
                    }
                }
                return false;
            } else if dominate(existing.iter().copied(), member.iter().copied()) {
                // `new_element` dominates `element`, it is thus part of the Pareto front
                self.0.remove(i);
                // looks at the rest of the Pareto front to remove any further element that
                // are dominated
                self.remove_dominated_starting_at(&existing, i);
                break;
            }
        }

        self.0.push(existing);
        true
    }

    pub fn push_and_dominating_check(
        &mut self,
        registers: DisjointRegisters,
        mut dominating_check: impl FnMut(DisjointRegisters) -> Option<Arc<[u32]>>,
    ) -> bool {
        let mut domatinating_check_failed = false;
        for (i, member) in self.0.iter().enumerate() {
            if dominate(member.iter().copied(), registers.iter()) {
                // `new_element` is dominated by `element`, it is thus not part of the Pareto
                // front swap `element` with the previous element in order to
                // percolate the best elements to the top NOTE: in my benchmarks
                // this brings clear performance benefits by putting "killer" elements first
                if i > 0 {
                    // SAFETY: `i` is in range, and `i - 1` must also be in range because of the
                    // if. Note that the safe version was not optimizing the bounds check
                    unsafe {
                        self.0.swap_unchecked(i, i - 1);
                    }
                }
                return false;
            } else if !domatinating_check_failed
                && dominate(registers.iter(), member.iter().copied())
            {
                if let Some(cycle_combination) = (dominating_check)(registers) {
                    // `new_element` dominates `element`, it is thus part of the Pareto front
                    self.0.remove(i);
                    // looks at the rest of the Pareto front to remove any further element that
                    // are dominated
                    self.remove_dominated_starting_at(&cycle_combination, i);
                    self.0.push(cycle_combination);
                    return true;
                }
                domatinating_check_failed = true;
            }
        }

        if !domatinating_check_failed && let Some(candidate) = (dominating_check)(registers) {
            // `new_element` has not been dominated; it is thus part of the Pareto front
            self.0.push(candidate);
            true
        } else {
            false
        }
    }

    fn remove_dominated(&mut self, registers: &[u32]) -> bool {
        for (i, member) in self.0.iter().enumerate() {
            if dominate(member.iter().copied(), registers.iter().copied()) {
                if i > 0 {
                    unsafe {
                        self.0.swap_unchecked(i, i - 1);
                    }
                }
                return false;
            } else if dominate(registers.iter().copied(), member.iter().copied()) {
                self.0.remove(i);
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
        let mut largest_front = other_pareto_front.0;
        if largest_front.len() < self.0.len() {
            std::mem::swap(&mut self.0, &mut largest_front);
        }
        // for all the elements in the largest front, remove dominated elements from the
        // smallest front the largest front keeps only the elements that should be in
        // the final Pareto front
        largest_front.retain(|x| self.remove_dominated(x));
        // extends the largest front with the content of the smallest front
        // and make it our front
        std::mem::swap(&mut self.0, &mut largest_front);
        self.0.extend(largest_front);
    }
}

impl From<CCParetoFront> for Vec<Arc<[u32]>> {
    /// Converts the Pareto front into a vector.
    fn from(CCParetoFront(possible_registers): CCParetoFront) -> Vec<Arc<[u32]>> {
        possible_registers
    }
}

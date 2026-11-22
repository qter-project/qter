//! Build a [Pareto front](https://en.wikipedia.org/wiki/Pareto_front) incrementaly. Based on the [pareto_front](https://crates.io/crates/pareto_front) crate.

use std::cmp::Ordering;

use crate::cycle_combinations_tree::DisjointRegisters;

#[derive(Debug, Default)]
pub(crate) struct CCParetoFront(pub(crate) Vec<Box<[u32]>>);

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

fn dominate<const ASSUME_UNIQUE: bool>(
    a: impl IntoIterator<Item = u32>,
    b: impl IntoIterator<Item = u32>,
    return_less_early: bool,
) -> Ordering {
    /*
    old >, new >: continue
    old >, new =: continue
    old >, new <: return eq
    old =, new >: continue gt
    old =, new =: continue
    old =, new <: continue lt
    old <, new >: return eq
    old <, new =: continue
    old <, new <: continue
    */
    let mut old = Ordering::Equal;
    for (x, y) in a.into_iter().zip(b) {
        let new = x.cmp(&y);
        match (old, new) {
            (Ordering::Less, Ordering::Greater) | (Ordering::Greater, Ordering::Less) => {
                return Ordering::Equal;
            }

            (Ordering::Less, Ordering::Less | Ordering::Equal)
            | (Ordering::Greater, Ordering::Greater | Ordering::Equal)
            | (Ordering::Equal, Ordering::Equal) => (),

            (Ordering::Equal, Ordering::Greater) => {
                old = new;
            }
            (Ordering::Equal, Ordering::Less) => {
                if return_less_early {
                    return Ordering::Less;
                }
                old = new;
            }
        }
    }
    if ASSUME_UNIQUE {
        old
    } else {
        match old {
            Ordering::Equal | Ordering::Greater => Ordering::Greater,
            Ordering::Less => Ordering::Less,
        }
    }
}

impl CCParetoFront {
    /// Removes all elements in the front that are dominated by `registers`,
    /// starting at `start`, which must already be known to be dominated.
    fn remove_dominated_starting_at<const ASSUME_UNIQUE: bool>(
        &mut self,
        registers: &[u32],
        start: usize,
    ) {
        let mut write_idx = start;
        for read_idx in start + 1..self.0.len() {
            let member = &self.0[read_idx];
            if dominate::<ASSUME_UNIQUE>(registers.iter().copied(), member.iter().copied(), true)
                .is_le()
            {
                // Move survivors into the gap without touching their reference
                // counts.
                self.0.swap(write_idx, read_idx);
                write_idx += 1;
            }
        }
        self.0.truncate(write_idx);
    }

    pub fn push_and_dominating_check(
        &mut self,
        registers: DisjointRegisters,
        mut dominating_check: impl FnMut(DisjointRegisters) -> Option<Box<[u32]>>,
    ) -> bool {
        let mut domatinating_check_failed = false;
        for (i, member) in self.0.iter().enumerate() {
            match dominate::<true>(
                member.iter().copied(),
                registers.iter(),
                domatinating_check_failed,
            ) {
                Ordering::Less => {
                    if !domatinating_check_failed {
                        if let Some(cycle_combination) = (dominating_check)(registers) {
                            // `new_element` dominates `element`, it is thus
                            // part of the Pareto front
                            // looks at the rest of the Pareto front to remove
                            // any further element that are dominated
                            self.remove_dominated_starting_at::<true>(&cycle_combination, i);
                            self.0.push(cycle_combination);
                            return true;
                        }
                        domatinating_check_failed = true;
                    }
                }
                Ordering::Equal => (),
                Ordering::Greater => {
                    // `new_element` is dominated by `element`, it is thus not
                    // part of the Pareto front swap `element` with the previous
                    // element in order to percolate the best elements to the
                    // top NOTE: in my benchmarks this brings clear performance
                    // benefits by putting "killer" elements first
                    if i > 0 {
                        // SAFETY: `i` is in range, and `i - 1` must also be in
                        // range because of the if. Note that the safe version
                        // was not optimizing the bounds check
                        unsafe {
                            self.0.swap_unchecked(i, i - 1);
                        }
                    }
                    return false;
                }
            }
        }

        if !domatinating_check_failed && let Some(candidate) = (dominating_check)(registers) {
            // `new_element` has not been dominated; it is thus part of the
            // Pareto front
            self.0.push(candidate);
            true
        } else {
            false
        }
    }
}

impl From<CCParetoFront> for Vec<Box<[u32]>> {
    /// Converts the Pareto front into a vector.
    fn from(CCParetoFront(possible_registers): CCParetoFront) -> Vec<Box<[u32]>> {
        possible_registers
    }
}

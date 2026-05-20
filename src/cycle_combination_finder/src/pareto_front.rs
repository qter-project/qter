//! Build a [Pareto front](https://en.wikipedia.org/wiki/Pareto_front) incrementaly. Based on the [pareto_front](https://crates.io/crates/pareto_front) crate.

use std::iter::FromIterator;

/// Used to define a pseudo-ordering for multi-dimensional optimization.
pub trait Dominate {
    /// Returns `true` if we are better (which might be superior or inferior
    /// depending on the specification) than `x` along all dimenssions.
    /// By convention, it usually returns `false` if `x` is equal to `self`.
    fn dominate(&self, x: &Self) -> bool;
}

#[derive(Clone, Debug)]
pub struct CandidateParetoFront<T> {
    front: Vec<T>,
}

impl<T: Dominate> CandidateParetoFront<T> {
    /// Removes all elements in the front that are dominated by `new_element`,
    /// starting at index `index_start`.
    fn remove_dominated_starting_at(&mut self, new_element: &T, index_start: usize) {
        // lists all elements dominated by `new_element`, starting at index
        // `index_start`
        let mut index_dominated_elements = Vec::new();
        for (index, element) in self.front.iter().enumerate().skip(index_start) {
            if new_element.dominate(element) {
                index_dominated_elements.push(index);
            }
        }

        // removes the elements at the listed indexes
        // in reverse order to take into acount that each removed index shift all the
        // following indexes
        for index in index_dominated_elements.into_iter().rev() {
            self.front.swap_remove(index);
        }
    }

    /// Removes all the elements in the Pareto front that are dominated by
    /// `new_element`. Returns `true` if `new_element` should be in the
    /// Pareto front. Returns `false` if `new_element` was dominated and,
    /// thus, shouldn't be added to the front.
    ///
    /// This operation has `O(n)` complexity (where `n` is the number of
    /// elements currently in the Pareto front) but is optimized to favour
    /// early stopping and cache friendly.
    ///
    /// This operation might *not* preserve the ordering of the elements in the
    /// front.
    fn remove_dominated(&mut self, new_element: &T) -> bool {
        // for all elements of the pareto front, check whether they are dominated or
        // dominate `new_element`
        for (index, element) in self.front.iter().enumerate() {
            if element.dominate(new_element) {
                // `new_element` is dominated by `element`, it is thus not part of the Pareto
                // front swap `element` with the previous element in order to
                // percolate the best elements to the top NOTE: in my benchmarks
                // this brings clear performance benefits by putting "killer" elements first
                if index > 0 {
                    self.front.swap(index, index - 1);
                }
                return false;
            } else if new_element.dominate(element) {
                // `new_element` dominates `element`, it is thus part of the Pareto front
                self.front.swap_remove(index);
                // looks at the rest of the Pareto front to remove any further element that are
                // dominated
                self.remove_dominated_starting_at(new_element, index);
                return true;
            }
        }

        // `new_element` has not been dominated, it is thus part of the Pareto front
        true
    }

    /// Returns `true` if at least one element on the Pareto front dominates
    /// `new_element`.
    ///
    /// This operation has `O(n)` complexity (where `n` is the number of
    /// elements currently in the Pareto front) but is optimized to favour
    /// early stopping and cache friendly.
    pub fn dominate(&self, new_element: &T) -> bool {
        self.front
            .iter()
            .any(|element| element.dominate(new_element))
    }

    /// Adds `new_element` to the Pareto front.
    /// Returns `true` if the element is now in the Pareto front.
    /// Returns `false` if the element was dominated and, thus, not added to the
    /// front.
    ///
    /// This operation has `O(n)` complexity (where `n` is the number of
    /// elements currently in the Pareto front) but is optimized to favour
    /// early stopping and cache friendly.
    ///
    /// This operation might *not* preserve the ordering of the elements in the
    /// front.
    pub fn push(&mut self, new_element: T) -> bool {
        // removes dominated elements from the front and checks whether `new_element`
        // should be added
        let is_pareto_optimal = self.remove_dominated(&new_element);
        if is_pareto_optimal {
            self.front.push(new_element);
        }
        is_pareto_optimal
    }

    /// Adds the content of `pareto_front` to the Pareto front.
    ///
    /// This operation has `O(n*m)` complexity
    /// where `n` is the number of elements in `self`
    /// and `m` is the number of elements in `pareto_front`
    /// but is optimized to favour early stopping.
    pub fn merge(&mut self, pareto_front: CandidateParetoFront<T>) {
        // set the largest front aside
        let mut largest_front = pareto_front.front;
        if largest_front.len() < self.front.len() {
            std::mem::swap(&mut self.front, &mut largest_front);
        }
        // for all the elements in the largest front, remove dominated elements from the
        // smallest front the largest front keeps only the elements that should
        // be in the final Pareto front
        largest_front.retain(|x| self.remove_dominated(x));
        // extends the largest front with the content of the smallest front
        // and make it our front
        std::mem::swap(&mut self.front, &mut largest_front);
        self.front.extend(largest_front);
    }
}

impl<T: Dominate> Default for CandidateParetoFront<T> {
    fn default() -> Self {
        Self { front: vec![] }
    }
}

impl<T: Dominate> From<CandidateParetoFront<T>> for Vec<T> {
    /// Converts the Pareto front into a vector.
    /// This operation is free as the underlying datastructure is a vector.
    fn from(front: CandidateParetoFront<T>) -> Vec<T> {
        front.front
    }
}

impl<T: Dominate> IntoIterator for CandidateParetoFront<T> {
    type IntoIter = std::vec::IntoIter<T>;
    type Item = T;

    /// Creates an iterator from a `ParetoFront`.
    fn into_iter(self) -> Self::IntoIter {
        self.front.into_iter()
    }
}

impl<T: Dominate> FromIterator<T> for CandidateParetoFront<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut front = CandidateParetoFront::default();

        for x in iter {
            front.push(x);
        }

        front
    }
}

impl<T: Dominate> Extend<T> for CandidateParetoFront<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        // Note: I tried a divide and conquer type of approach
        //       (creating a new pareto front from `iter` and merging it)
        //       but it was slightly slower for all problem sizes
        for x in iter {
            self.push(x);
        }
    }
}

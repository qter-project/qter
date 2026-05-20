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
    front: Vec<T>,
    index_dominated_elements: Vec<usize>,
}

impl<T: Dominate> CandidateParetoFront<T> {
    /// Removes all elements in the front that are dominated by `new_element`,
    /// starting at index `index_start`.
    fn remove_dominated_starting_at(&mut self, new_element: &T, index_start: usize) {
        // lists all elements dominated by `new_element`, starting at index
        // `index_start`
        let available_elements = self.front.len().saturating_sub(index_start);
        if self.index_dominated_elements.len() < available_elements {
            self.index_dominated_elements.resize(available_elements, 0);
        }
        let mut len = 0;
        for ((index, element), x) in self
            .front
            .iter()
            .enumerate()
            .skip(index_start)
            .zip(&mut self.index_dominated_elements)
        {
            if new_element.dominate(element) {
                *x = index;
                len += 1;
            }
        }

        // removes the elements at the listed indexes
        // in reverse order to take into acount that each removed index shift all the
        // following indexes
        for &index in self.index_dominated_elements.iter().take(len).rev() {
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

    pub fn push_and_check(
        &mut self,
        mut new_element: T,
        mut check: impl FnMut(&mut T) -> bool,
    ) -> bool {
        // for all elements of the pareto front, check whether they are dominated or
        // dominate `new_element`
        for (index, element) in self.front.iter().enumerate() {
            if element.dominate(&new_element) {
                // `new_element` is dominated by `element`, it is thus not part of the Pareto
                // front swap `element` with the previous element in order to
                // percolate the best elements to the top NOTE: in my benchmarks
                // this brings clear performance benefits by putting "killer" elements first
                if index > 0 {
                    self.front.swap(index, index - 1);
                }
                return false;
            } else if new_element.dominate(element) && (check)(&mut new_element) {
                // `new_element` dominates `element`, it is thus part of the Pareto front
                self.front.swap_remove(index);
                // looks at the rest of the Pareto front to remove any further element that are
                // dominated
                self.remove_dominated_starting_at(&new_element, index);
                self.front.push(new_element);
                return true;
            }
        }

        if (check)(&mut new_element) {
            // `new_element` has not been dominated, it is thus part of the Pareto front
            self.front.push(new_element);
            true
        } else {
            false
        }
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
        Self {
            front: vec![],
            index_dominated_elements: vec![],
        }
    }
}

impl<T: Dominate> From<CandidateParetoFront<T>> for Vec<T> {
    /// Converts the Pareto front into a vector.
    /// This operation is free as the underlying datastructure is a vector.
    fn from(front: CandidateParetoFront<T>) -> Vec<T> {
        front.front
    }
}

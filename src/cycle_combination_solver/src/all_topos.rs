use std::collections::VecDeque;

/// Iterator that generates all topological sorts of a directed graph.
///
/// The graph is represented as:
/// - `n`: the maximum node value (nodes are 1..=n)
/// - `edges`: a slice of directed edges [from, to]
pub struct AllTopologicalSorts {
    n: usize,
    /// Adjacency list: successors[i] contains nodes that i points to
    successors: Box<[Vec<usize>]>,
    /// Number of incoming edges for each node
    count: Box<[u32]>,
    /// Deque of nodes with zero in-degree
    available: VecDeque<usize>,
    /// Stack tracking first choice at each position
    bases: Vec<usize>,
    /// Current topological sort (fixed size buffer, reused across iterations)
    current: Vec<usize>,
    /// Current position in the topological sort being built
    // pos: usize,
    /// Whether iteration is complete
    done: bool,
}

impl AllTopologicalSorts {
    /// Creates a new iterator for all topological sorts.
    ///
    /// # Arguments
    /// * `n` - Maximum node value (graph contains nodes 1..=n)
    /// * `edges` - Slice of edges, where each edge is [from, to]
    ///
    /// # Panics
    ///
    /// Panics if the graph has no nodes with in-degree 0 (contains a cycle).
    pub fn new(n: usize, edges: &[[usize; 2]]) -> Self {
        let mut successors = vec![Vec::new(); n + 1];
        let mut count = vec![0; n + 1];

        for &[from, to] in edges {
            successors[from].push(to);
            count[to] += 1;
        }

        let available: VecDeque<_> = (1..=n).filter(|&i| count[i] == 0).collect();

        assert!(!available.is_empty() || n == 0, "Cycle detected");

        Self {
            n,
            successors: successors.into_boxed_slice(),
            count: count.into_boxed_slice(),
            available,
            bases: Vec::with_capacity(n),
            current: Vec::with_capacity(n),
            done: false,
        }
    }

    /// Returns a reference to the current topological sort.
    /// This reference is valid until the next call to `next()`.
    pub fn current(&self) -> &[usize] {
        &self.current
    }
}

impl Iterator for AllTopologicalSorts {
    type Item = ();

    fn next(&mut self) -> Option<Self::Item> {
        // Algorithm based on:
        // Knuth, Donald E., Szwarcfiter, Jayme L. (1974).
        // "A Structured Program to Generate All Topological Sorting Arrangements"
        // Information Processing Letters, Volume 2, Issue 6, 1974, Pages 153-157,
        // ISSN 0020-0190, https://doi.org/10.1016/0020-0190(74)90001-5.
        //
        // The algorithm works by:
        // 1. Maintaining a deque `available` of nodes with in-degree 0 (no incoming edges)
        // 2. At each position in the sort, we can choose any node from `available`
        // 3. When we choose a node, we "remove" its outgoing edges by decrementing
        //    successor counts, potentially adding new nodes to `available`
        // 4. We track the first choice at each position in `bases` for backtracking:
        //    - When we first reach position i, we record which node we chose in bases[i]
        //    - During backtracking, when we return to position i, we rotate `available`
        //      to try different choices at that position
        //    - We know we've tried all choices at position i when the deque has been
        //      rotated back to the original first choice (the "base")
        //    - Continuing the example above: after finding [2,1,3,4], we backtrack:
        //      * Position 3: only one choice (4), so backtrack further
        //      * Position 2: only one choice (3), so backtrack further
        //      * Position 1: only one choice (1), so backtrack further
        //      * Position 0: available=[2,1] (after rotation), try 1 next
        //      * We get sort [1,2,3,4], then backtrack again
        //      * Position 0: available=[1,2], back of deque is 2 (the base!)
        //      * Since we've rotated back to base, position 0 is exhausted, done!
        // 5. When we complete a sort, we backtrack by:
        //    - Restoring edges (incrementing counts)
        //    - Removing nodes from `available` that now have incoming edges
        //    - Moving the just-removed node to the front of `available` (rotation)
        //    - If the back of `available` equals the base, all choices exhausted
        // 6. The algorithm generates all valid topological sorts without duplicationn

        // If we just returned a result, backtrack before computing next
        if self.current.len() == self.n && !self.done {
            while let Some(q) = self.current.pop() {
                // Restore edges from q
                for &successor in &self.successors[q] {
                    self.count[successor] += 1;
                }

                // Remove nodes from available that now have incoming edges
                while self
                    .available
                    .back()
                    .is_some_and(|&node| self.count[node] > 0)
                {
                    self.available.pop_back();
                }

                self.available.push_front(q);

                // Check if we've exhausted all choices at this position
                if self
                    .available
                    .back()
                    .is_some_and(|&node| node == *self.bases.last().unwrap())
                {
                    self.bases.pop();
                } else {
                    // More choices available at this position
                    break;
                }
            }

            // Check if backtracking exhausted all options
            if self.current.is_empty() && self.bases.is_empty() {
                self.done = true;
                return None;
            }
        }

        if self.done {
            return None;
        }

        loop {
            if self.current.len() == self.n {
                // Found a complete topological sort
                // Return without backtracking - that happens on next call
                return Some(());
            }

            // No available nodes means cycle detected
            let q = self.available.pop_back().unwrap();

            // Remove edges from q
            for &successor in &self.successors[q] {
                self.count[successor] -= 1;

                if self.count[successor] == 0 {
                    self.available.push_back(successor);
                }
            }

            self.current.push(q);

            // Track base choice at this position
            if self.bases.len() < self.current.len() {
                self.bases.push(q);
                assert!(self.bases.len() <= self.n);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_graph() {
        let edges = [[1, 2], [2, 3], [2, 4]];

        let mut iter = AllTopologicalSorts::new(4, &edges);
        let mut results = Vec::new();
        while iter.next().is_some() {
            results.push(iter.current().to_vec());
        }

        results.sort_unstable();
        assert_eq!(results, vec![vec![1, 2, 3, 4], vec![1, 2, 4, 3]]);
    }

    #[test]
    #[should_panic = "Cycle detected"]
    fn cycle_detection() {
        let edges = [[1, 2], [2, 3], [3, 1]];
        AllTopologicalSorts::new(3, &edges);
    }

    #[test]
    fn linear_graph() {
        let edges = [[1, 2], [2, 3], [3, 4]];

        let mut iter = AllTopologicalSorts::new(4, &edges);
        let mut results = Vec::new();
        while iter.next().is_some() {
            results.push(iter.current().to_vec());
        }

        assert_eq!(results, vec![vec![1, 2, 3, 4]]);
    }

    #[test]
    fn empty_graph() {
        let edges = [];

        let mut iter = AllTopologicalSorts::new(3, &edges);
        let mut results = Vec::new();
        while iter.next().is_some() {
            results.push(iter.current().to_vec());
        }

        // All permutations of [1, 2, 3] are valid
        assert_eq!(results.len(), 6); // 3! = 6
    }

    #[test]
    fn diamond_graph() {
        let edges = [[1, 2], [1, 3], [2, 4], [3, 4]];

        let mut iter = AllTopologicalSorts::new(4, &edges);
        let mut results = Vec::new();
        while iter.next().is_some() {
            results.push(iter.current().to_vec());
        }
        results.sort_unstable();

        assert_eq!(results, vec![vec![1, 2, 3, 4], vec![1, 3, 2, 4]]);
    }

    #[test]
    fn single_node() {
        let edges = [];

        let mut iter = AllTopologicalSorts::new(1, &edges);
        let mut results = Vec::new();
        while iter.next().is_some() {
            results.push(iter.current().to_vec());
        }

        assert_eq!(results, vec![vec![1]]);
    }

    #[test]
    fn larger_graph() {
        let edges = [[1, 2], [1, 3], [2, 4], [2, 5], [3, 5], [3, 6], [4, 6]];

        let mut iter = AllTopologicalSorts::new(6, &edges);
        let mut results = Vec::new();
        while iter.next().is_some() {
            results.push(iter.current().to_vec());
        }
        assert_eq!(
            results,
            vec![
                vec![1, 3, 2, 5, 4, 6],
                vec![1, 3, 2, 4, 6, 5],
                vec![1, 3, 2, 4, 5, 6],
                vec![1, 2, 4, 3, 6, 5],
                vec![1, 2, 4, 3, 5, 6],
                vec![1, 2, 3, 5, 4, 6],
                vec![1, 2, 3, 4, 6, 5],
                vec![1, 2, 3, 4, 5, 6]
            ]
        );

        for sort in &results {
            for &[from, to] in &edges {
                let from_pos = sort.iter().position(|&x| x == from).unwrap();
                let to_pos = sort.iter().position(|&x| x == to).unwrap();
                assert!(from_pos < to_pos);
            }
        }
    }

    #[test]
    fn complex_dag() {
        let edges = [
            [1, 3],
            [1, 4],
            [2, 3],
            [2, 5],
            [3, 6],
            [3, 7],
            [4, 7],
            [4, 8],
            [5, 6],
            [6, 8],
        ];

        let mut iter = AllTopologicalSorts::new(8, &edges);
        let mut results = Vec::new();
        while iter.next().is_some() {
            results.push(iter.current().to_vec());
        }
        assert_eq!(results.len(), 63);

        for sort in &results {
            for &[from, to] in &edges {
                let from_pos = sort.iter().position(|&x| x == from).unwrap();
                let to_pos = sort.iter().position(|&x| x == to).unwrap();
                assert!(from_pos < to_pos);
            }
        }
    }

    #[test]
    fn large_graph_two_solutions() {
        let mut edges = vec![[1, 3], [2, 3]];
        for i in 3..20 {
            edges.push([i, i + 1]);
        }

        let mut iter = AllTopologicalSorts::new(20, &edges);
        let mut results = Vec::new();
        while iter.next().is_some() {
            results.push(iter.current().to_vec());
        }
        results.sort_unstable();

        assert_eq!(
            results,
            vec![
                vec![
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
                ],
                vec![
                    2, 1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
                ]
            ]
        );
    }
}

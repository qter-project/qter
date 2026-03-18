#![warn(clippy::pedantic)]
#![allow(clippy::missing_panics_doc, clippy::too_many_lines)]

use fxhash::{FxHashMap, FxHashSet};
use num_integer::gcd;
use std::{cmp::Ordering, time::Instant};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OrderFactors<const N: usize> {
    pub exps: [u8; N],
}

impl<const N: usize> OrderFactors<N> {
    #[inline]
    #[must_use]
    pub fn one() -> Self {
        Self { exps: [0; N] }
    }

    #[inline]
    #[must_use]
    pub fn to_u64(&self, primes: &[u64]) -> u64 {
        let mut result = 1u64;
        for (i, &p) in primes.iter().enumerate() {
            for _ in 0..self.exps[i] {
                result *= p;
            }
        }
        result
    }

    #[must_use]
    pub fn exp_sum(&self) -> u16 {
        self.exps.iter().map(|&x| u16::from(x)).sum()
    }
}

/// Assumes `gcd(u64, u64) -> u64` is already provided.
///
/// Returns:
///   result[parity][orientation_sum] = set of reachable orders
///   (represented as prime-exponent vectors over `primes`)
///   using exactly `piece_count` pieces.
///
/// This uses:
/// - shared prime basis
/// - fixed-size exponent vectors
/// - canonical cycle ordering
/// - forward unbounded-knapsack DP
/// - no whole-set cloning
#[must_use]
pub fn possible_orders_for_piece_type_with_primes<const N: usize>(
    piece_count: usize,
    orientation_count: usize,
    primes: &[u64],
) -> Vec<Vec<Vec<FxHashSet<OrderFactors<N>>>>> {
    assert!(
        primes.len() <= N,
        "Need OrderFactors<{}>, but got {} primes",
        N,
        primes.len()
    );

    // Canonical cycle types in sorted order:
    // (cycle_len ascending, cycle_orient_sum ascending)
    let cycle_count = piece_count * orientation_count;

    let mut cycle_lens = Vec::with_capacity(cycle_count);
    let mut cycle_parities = Vec::with_capacity(cycle_count);
    let mut cycle_orient_sums = Vec::with_capacity(cycle_count);
    let mut cycle_factors = Vec::with_capacity(cycle_count);

    for cycle_len in 1..=piece_count {
        for cycle_orient_sum in 0..orientation_count {
            let cycle_order: u64 = if cycle_orient_sum == 0 {
                cycle_len as u64
            } else {
                (cycle_len as u64 * orientation_count as u64)
                    / gcd(orientation_count as u64, cycle_orient_sum as u64)
            };

            let mut exps = [0u8; N];
            let mut n = cycle_order;

            for (i, &p) in primes.iter().enumerate() {
                if p * p > n && n > 1 {
                    break;
                }
                while n.is_multiple_of(p) {
                    exps[i] += 1;
                    n /= p;
                }
            }

            if n > 1 {
                let idx = primes
                    .iter()
                    .position(|&p| p == n)
                    .expect("prime basis missing leftover prime factor");
                exps[idx] += 1;
            }

            cycle_lens.push(cycle_len);
            cycle_parities.push((cycle_len - 1) % 2);
            cycle_orient_sums.push(cycle_orient_sum);
            cycle_factors.push(OrderFactors::<N> { exps });
        }
    }

    // dp[parity][orient_sum][used] = set of reachable factor-vectors
    let mut dp: Vec<Vec<Vec<FxHashSet<OrderFactors<N>>>>> = (0..2)
        .map(|_| {
            (0..orientation_count)
                .map(|_| {
                    (0..=piece_count)
                        .map(|_| FxHashSet::<OrderFactors<N>>::default())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect();

    // Empty decomposition
    dp[0][0][0].insert(OrderFactors::<N>::one());

    // Process cycle types in canonical order.
    // Ascending `used` => same cycle type can be reused (unbounded knapsack),
    // while cycle-type order remains canonical.
    // TODO: this zip is super ugly
    for (&cycle_len, &cycle_parity, &cycle_orient_sum, cycle_factor) in cycle_lens
        .iter()
        .zip(cycle_parities.iter())
        .zip(cycle_orient_sums.iter())
        .zip(cycle_factors.iter())
        .map(|(((len, parity), orient_sum), factor)| (len, parity, orient_sum, factor))
    {
        // TODO: why the range?
        for used in cycle_len..=piece_count {
            let prev_used = used - cycle_len;

            for prev_parity in 0..2 {
                for prev_orient_sum in 0..orientation_count {
                    if dp[prev_parity][prev_orient_sum][prev_used].is_empty() {
                        continue;
                    }

                    let new_parity = prev_parity ^ cycle_parity;
                    let new_orient_sum = (prev_orient_sum + cycle_orient_sum) % orientation_count;

                    // let mut additions: Vec<OrderFactors<N>> =
                    //     Vec::with_capacity(dp[prev_parity][prev_orient_sum][prev_used].len());

                    // for &prev in dp[prev_parity][prev_orient_sum][prev_used].iter() {
                    //     let mut exps = [0u8; N];
                    //     for (i, (&prev_exp, &cycle_exp)) in
                    //         prev.exps.iter().zip(cycle_factor.exps.iter()).enumerate()
                    //     {
                    //         exps[i] = prev_exp.max(cycle_exp);
                    //     }
                    //     additions.push(OrderFactors::<N> { exps });
                    // }

                    match prev_parity.cmp(&new_parity) {
                        Ordering::Equal => match prev_orient_sum.cmp(&new_orient_sum) {
                            Ordering::Equal => {
                                // Same parity + same orient bucket: only `used` differs.
                                let bucket = &mut dp[new_parity][new_orient_sum];
                                let (left, right) = bucket.split_at_mut(used);
                                let src = &left[prev_used]; // prev_used < used always
                                let dst = &mut right[0]; // index `used`

                                extend_mapped(src, dst, cycle_factor);
                            }
                            Ordering::Less => {
                                // Same parity, different orientation buckets.
                                let parity_bucket = &mut dp[new_parity];
                                let (left, right) = parity_bucket.split_at_mut(new_orient_sum);
                                let src = &left[prev_orient_sum][prev_used];
                                let dst = &mut right[0][used];

                                extend_mapped(src, dst, cycle_factor);
                            }
                            Ordering::Greater => {
                                // Same parity, different orientation buckets.
                                let parity_bucket = &mut dp[new_parity];
                                let (left, right) = parity_bucket.split_at_mut(prev_orient_sum);
                                let dst = &mut left[new_orient_sum][used];
                                let src = &right[0][prev_used];

                                extend_mapped(src, dst, cycle_factor);
                            }
                        },
                        Ordering::Less => {
                            // Different parity buckets.
                            let (left, right) = dp.split_at_mut(new_parity);
                            let src = &left[prev_parity][prev_orient_sum][prev_used];
                            let dst = &mut right[0][new_orient_sum][used];

                            extend_mapped(src, dst, cycle_factor);
                        }
                        Ordering::Greater => {
                            // Different parity buckets.
                            let (left, right) = dp.split_at_mut(prev_parity);
                            let dst = &mut left[new_parity][new_orient_sum][used];
                            let src = &right[0][prev_orient_sum][prev_used];

                            extend_mapped(src, dst, cycle_factor);
                        }
                    }
                }
            }
        }
    }

    dp
}

fn extend_mapped<const N: usize>(
    src: &FxHashSet<OrderFactors<N>>,
    dst: &mut FxHashSet<OrderFactors<N>>,
    cycle_factor: &OrderFactors<N>,
) {
    dst.extend(src.iter().map(|prev| {
        let mut exps = [0u8; N];
        for (i, (&prev_exp, &cycle_exp)) in
            prev.exps.iter().zip(cycle_factor.exps.iter()).enumerate()
        {
            exps[i] = prev_exp.max(cycle_exp);
        }
        OrderFactors::<N> { exps }
    }));
}

fn main() {
    const N: usize = 30;

    let primes: [u64; N] = [
        2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89,
        97, 101, 103, 107, 109, 113,
    ];
    let edges_count = 120;
    let corners_count = 80;

    let now = Instant::now();
    let mut edges = possible_orders_for_piece_type_with_primes::<N>(edges_count, 2, &primes);
    let mut corners = possible_orders_for_piece_type_with_primes::<N>(corners_count, 3, &primes);
    println!("{:?}", now.elapsed());

    // let all_edges: Vec<OrderFactors<N>> = (0..2)
    //     .flat_map(|parity| edges[parity][0][edges_count].iter().copied())
    //     .collect();

    // let all_corners: Vec<OrderFactors<N>> = (0..2)
    //     .flat_map(|parity| corners[parity][0][corners_count].iter().copied())
    //     .collect();

    let mut all_combined = FxHashSet::default();
    for parity in 0..2 {
        let a = std::mem::take(&mut edges[parity][0][edges_count]);
        let b = std::mem::take(&mut corners[parity][0][corners_count]);

        let (mut outer, inner) = if a.len() >= b.len() {
            (
                a.into_iter().collect::<Vec<_>>(),
                b.into_iter().collect::<Vec<_>>(),
            )
        } else {
            (
                b.into_iter().collect::<Vec<_>>(),
                a.into_iter().collect::<Vec<_>>(),
            )
        };

        outer.sort_unstable_by_key(|x| std::cmp::Reverse(x.exp_sum()));

        let mut trie = MaxTrieNode::<N>::new(0);
        for y in &inner {
            trie.insert(y);
        }

        for x in &outer {
            let mut cur = [0u8; N];
            collect_distinct_maxima_for_x(&trie, x, &mut cur, &mut all_combined);
        }
    }
    // for parity in 0..2 {
    //     let a = std::mem::take(&mut edges[parity][0][edges_count]);
    //     let a: ParetoFront<OrderFactors<N>> = a.into_iter().collect();
    //     let b = std::mem::take(&mut corners[parity][0][corners_count]);
    //     let b: ParetoFront<OrderFactors<N>> = b.into_iter().collect();

    //     for i in b.iter().take(10) {
    //         println!("{:?} {}", i, i.to_u64(&primes));
    //     }

    //     for x in b {
    //         for y in a.iter() {
    //             let mut result = 1u64;

    //             for (i, &p) in primes.iter().enumerate() {
    //                 for _ in 0..x.exps[i].max(y.exps[i]) {
    //                     result *= p;
    //                 }
    //             }

    //             all_combined.insert(result);
    //         }
    //     }
    // }

    println!("{:?}", now.elapsed());

    println!("Total unique orders: {}", all_combined.len());
    let mut all_combined = all_combined
        .into_iter()
        .map(|factors| factors.to_u64(&primes))
        .collect::<Vec<_>>();
    all_combined.sort_unstable();
    for &order in all_combined.iter().rev().take(100) {
        println!("{order}");
    }
}

fn collect_distinct_maxima_for_x<const N: usize>(
    node: &MaxTrieNode<N>,
    x: &OrderFactors<N>,
    cur: &mut [u8; N],
    out: &mut FxHashSet<OrderFactors<N>>,
) {
    let dim = node.dim;

    if dim == N {
        out.insert(OrderFactors { exps: *cur });
        return;
    }

    // If all remaining subtree exponents are <= x on remaining dims,
    // then every y in this subtree yields exactly x on remaining dims.
    let mut fully_bounded = true;
    for i in dim..N {
        if node.subtree_max[i] > x.exps[i] {
            fully_bounded = false;
            break;
        }
    }

    if fully_bounded {
        let mut saved = [0u8; N];
        saved[dim..N].copy_from_slice(&cur[dim..N]);

        for i in dim..N {
            cur[i] = x.exps[i];
        }

        out.insert(OrderFactors { exps: *cur });

        cur[dim..N].copy_from_slice(&saved[dim..N]);
        return;
    }

    for (&e, child) in &node.children {
        let old = cur[dim];
        cur[dim] = x.exps[dim].max(e);
        collect_distinct_maxima_for_x(child, x, cur, out);
        cur[dim] = old;
    }
}

#[derive(Debug)]
struct MaxTrieNode<const N: usize> {
    dim: usize,
    // Children keyed by exponent value at this dimension.
    children: FxHashMap<u8, Box<MaxTrieNode<N>>>,
    // If dim == N, this is a terminal count (we don't really need multiplicity).
    terminal: bool,
    // For pruning: coordinatewise maxima over all vectors in this subtree.
    subtree_max: [u8; N],
}

impl<const N: usize> MaxTrieNode<N> {
    fn new(dim: usize) -> Self {
        Self {
            dim,
            children: FxHashMap::default(),
            terminal: false,
            subtree_max: [0; N],
        }
    }

    fn insert(&mut self, v: &OrderFactors<N>) {
        // Update subtree maxima.
        for i in 0..N {
            self.subtree_max[i] = self.subtree_max[i].max(v.exps[i]);
        }

        if self.dim == N {
            self.terminal = true;
            return;
        }

        let e = v.exps[self.dim];
        self.children
            .entry(e)
            .or_insert_with(|| Box::new(Self::new(self.dim + 1)))
            .insert(v);
    }
}

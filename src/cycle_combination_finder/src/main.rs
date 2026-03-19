#![feature(portable_simd)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_panics_doc, clippy::too_many_lines)]

use dashmap::DashSet;
use fxhash::{FxBuildHasher, FxHashMap, FxHashSet};
use log::debug;
use num_integer::gcd;
use rayon::prelude::*;
use std::{
    cmp::Ordering,
    fmt::{Debug, Display, Formatter},
    simd::{
        LaneCount, Simd, SupportedLaneCount,
        cmp::{SimdOrd, SimdPartialOrd},
        num::SimdUint,
    },
    time::Instant,
};

const N: usize = 32;

const PRIMES: [u64; N] = [
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131,
];

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OrderFactors<const N: usize>
where
    LaneCount<N>: SupportedLaneCount,
{
    exps: Simd<u8, N>,
}

impl<const N: usize> OrderFactors<N>
where
    LaneCount<N>: SupportedLaneCount,
{
    fn one() -> Self {
        Self {
            exps: Simd::splat(0),
        }
    }

    fn to_u64(&self) -> u64 {
        let mut result = 1u64;
        for (i, p) in PRIMES.into_iter().enumerate() {
            for _ in 0..self.exps[i] {
                result *= p;
            }
        }
        result
    }

    fn exp_sum(&self) -> u16 {
        self.exps.cast::<u16>().reduce_sum()
    }

    fn lcm(&self, other: &Self) -> Self {
        Self {
            exps: self.exps.simd_max(other.exps),
        }
    }
}

fn combine_orders<const N: usize>(
    dst: &mut FxHashSet<OrderFactors<N>>,
    src: &FxHashSet<OrderFactors<N>>,
    combine: &OrderFactors<N>,
) where
    LaneCount<N>: SupportedLaneCount,
{
    dst.extend(src.iter().map(|order| order.lcm(combine)));
}

impl<const N: usize> Display for OrderFactors<N>
where
    LaneCount<N>: SupportedLaneCount,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "OF({})", self.to_u64())
    }
}

#[must_use]
pub fn possible_orders_for_piece_type_with_primes<const N: usize>(
    piece_count: usize,
    orientation_count: usize,
) -> Vec<Vec<Vec<FxHashSet<OrderFactors<N>>>>>
where
    LaneCount<N>: SupportedLaneCount,
{
    #[allow(clippy::struct_field_names)]
    #[derive(Debug)]
    struct Subproblem<const N: usize>
    where
        LaneCount<N>: SupportedLaneCount,
    {
        subproblem_piece_count: usize,
        subproblem_parity: usize,
        subproblem_orient_sum: usize,
        subproblem_order: OrderFactors<N>,
    }

    assert!(
        PRIMES.len() <= N,
        "Need OrderFactors<{}>, but got {} primes",
        N,
        PRIMES.len()
    );

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

    // Identity
    dp[0][0][0].insert(OrderFactors::<N>::one());

    // Canonical cycle types in sorted order:
    // (cycle_len ascending, cycle_orient_sum ascending)
    let subproblems = (1..=piece_count).flat_map(|subproblem_piece_count| {
        (0..orientation_count).map(move |subproblem_orient_sum| {
            let cycle_order: u64 = if subproblem_orient_sum == 0 {
                subproblem_piece_count as u64
            } else {
                (subproblem_piece_count as u64 * orientation_count as u64)
                    / gcd(orientation_count as u64, subproblem_orient_sum as u64)
            };

            let mut exps = [0u8; N];
            let mut n = cycle_order;

            for (i, p) in PRIMES.into_iter().enumerate() {
                if p * p > n && n > 1 {
                    break;
                }
                while n.is_multiple_of(p) {
                    exps[i] += 1;
                    n /= p;
                }
            }

            if n > 1 {
                let idx = PRIMES.into_iter().position(|p| p == n).unwrap();
                exps[idx] += 1;
            }

            Subproblem {
                subproblem_piece_count,
                subproblem_parity: (subproblem_piece_count - 1) % 2,
                subproblem_orient_sum,
                subproblem_order: OrderFactors::<N> {
                    exps: Simd::from_array(exps),
                },
            }
        })
    });

    for Subproblem {
        subproblem_piece_count,
        subproblem_parity,
        subproblem_orient_sum,
        subproblem_order,
    } in subproblems
    {
        // debug!("{:?}", "new");
        for first_piece_count in subproblem_piece_count..=piece_count {
            let second_piece_count = first_piece_count - subproblem_piece_count;

            // debug!("trying {first_piece_count:?} {second_piece_count:?}");

            for second_parity in 0..2 {
                for second_orient_sum in 0..orientation_count {
                    if dp[second_parity][second_orient_sum][second_piece_count].is_empty() {
                        continue;
                    }

                    let first_parity = second_parity ^ subproblem_parity;
                    let first_orient_sum =
                        (second_orient_sum + subproblem_orient_sum) % orientation_count;

                    match second_parity.cmp(&first_parity) {
                        Ordering::Equal => match second_orient_sum.cmp(&first_orient_sum) {
                            Ordering::Equal => {
                                // Same parity + same orient bucket: only `first_used` differs.
                                let bucket = &mut dp[first_parity][first_orient_sum];
                                let (left, right) = bucket.split_at_mut(first_piece_count);
                                let src = &left[second_piece_count]; // prev_used < used always
                                let dst = &mut right[0]; // index `used`

                                combine_orders(dst, src, &subproblem_order);
                            }
                            Ordering::Less => {
                                let parity_bucket = &mut dp[first_parity];
                                let (left, right) = parity_bucket.split_at_mut(first_orient_sum);
                                let src = &left[second_orient_sum][second_piece_count];
                                let dst = &mut right[0][first_piece_count];

                                combine_orders(dst, src, &subproblem_order);
                            }
                            Ordering::Greater => {
                                let parity_bucket = &mut dp[first_parity];
                                let (left, right) = parity_bucket.split_at_mut(second_orient_sum);
                                let dst = &mut left[first_orient_sum][first_piece_count];
                                let src = &right[0][second_piece_count];

                                combine_orders(dst, src, &subproblem_order);
                            }
                        },
                        Ordering::Less => {
                            let (left, right) = dp.split_at_mut(first_parity);
                            let src = &left[second_parity][second_orient_sum][second_piece_count];
                            let dst = &mut right[0][first_orient_sum][first_piece_count];

                            combine_orders(dst, src, &subproblem_order);
                        }
                        Ordering::Greater => {
                            let (left, right) = dp.split_at_mut(second_parity);
                            let dst = &mut left[first_parity][first_orient_sum][first_piece_count];
                            let src = &right[0][second_orient_sum][second_piece_count];

                            combine_orders(dst, src, &subproblem_order);
                        }
                    }
                }
            }
        }
    }

    dp
}

fn main() {
    pretty_env_logger::init();

    let edge = (120, 2);
    let corner = (80, 3);

    let orgnow = Instant::now();
    let mut edges = possible_orders_for_piece_type_with_primes::<N>(edge.0, edge.1);
    debug!("Edges in {:?}", orgnow.elapsed());
    let now = Instant::now();
    let mut corners = possible_orders_for_piece_type_with_primes::<N>(corner.0, corner.1);
    debug!("Corners in {:?}", now.elapsed());

    let all_combined = DashSet::<OrderFactors<N>, FxBuildHasher>::default();
    for parity in 0..2 {
        let a = std::mem::take(&mut edges[parity][0][edge.0]);
        let b = std::mem::take(&mut corners[parity][0][corner.0]);

        let (mut outer, inner) = if a.len() >= b.len() {
            (a.into_iter().collect::<Vec<_>>(), b)
        } else {
            (b.into_iter().collect::<Vec<_>>(), a)
        };

        outer.sort_unstable_by_key(|x| std::cmp::Reverse(x.exp_sum()));

        let mut trie = MaxTrieNode::<N>::new(0);
        for y in inner {
            trie.insert(y);
        }

        let now = Instant::now();
        outer
            .into_par_iter()
            .map(|x| {
                let mut cur = [0u8; N];
                let mut local = FxHashSet::default();
                collect_distinct_maxima_for_x(&trie, &x, &mut cur, &mut local);
                local
            })
            .for_each(|local| {
                for order in local {
                    all_combined.insert(order);
                }
            });
        debug!("Main in {:?}", now.elapsed());
    }

    println!("{:?}", orgnow.elapsed());

    println!("Total unique orders: {}", all_combined.len());
    let mut all_combined = all_combined
        .into_iter()
        .map(|f| f.to_u64())
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
) where
    LaneCount<N>: SupportedLaneCount,
{
    if node.level == N {
        out.insert(OrderFactors {
            exps: Simd::from_array(*cur),
        });
    } else if node.subtree_max.exps.simd_gt(x.exps).to_bitmask() >> node.level == 0 {
        // If all remaining subtree exponents are <= x on remaining levels,
        // then every y in this subtree yields exactly x on remaining levels.
        let mut exps = x.exps;
        exps[..node.level].copy_from_slice(&cur[..node.level]);
        out.insert(OrderFactors { exps });
    } else {
        for (&exp, child) in &node.children {
            let old = std::mem::replace(&mut cur[node.level], x.exps[node.level].max(exp));
            collect_distinct_maxima_for_x(child, x, cur, out);
            cur[node.level] = old;
        }
    }
}

#[derive(Debug)]
struct MaxTrieNode<const N: usize>
where
    LaneCount<N>: SupportedLaneCount,
{
    level: usize,
    // Children keyed by exponent value at this level.
    children: FxHashMap<u8, MaxTrieNode<N>>,
    // For pruning: coordinatewise maxima over all vectors in this subtree.
    subtree_max: OrderFactors<N>,
}

impl<const N: usize> MaxTrieNode<N>
where
    LaneCount<N>: SupportedLaneCount,
{
    fn new(level: usize) -> Self {
        Self {
            level,
            children: FxHashMap::default(),
            subtree_max: OrderFactors::one(),
        }
    }

    fn insert(&mut self, v: OrderFactors<N>) {
        self.subtree_max = self.subtree_max.lcm(&v);

        if self.level == N {
            return;
        }

        let e = v.exps[self.level];
        self.children
            .entry(e)
            .or_insert_with(|| Self::new(self.level + 1))
            .insert(v);
    }
}

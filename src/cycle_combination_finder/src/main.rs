#![feature(portable_simd)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_panics_doc, clippy::too_many_lines)]

use dashmap::DashSet;
use fxhash::{FxBuildHasher, FxHashMap, FxHashSet};
use log::debug;
use num_integer::gcd;
use rayon::prelude::*;
use std::{
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
    #[derive(Clone, Debug)]
    struct Cycle<const N: usize>
    where
        LaneCount<N>: SupportedLaneCount,
    {
        cycle_piece_count: usize,
        cycle_parity: usize,
        cycle_orient_sum: usize,
        cycle_order: OrderFactors<N>,
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

    let cycles = (1..=piece_count)
        .flat_map(|cycle_piece_count| {
            (0..orientation_count).map(move |cycle_orient_sum| {
                let cycle_order = if cycle_orient_sum == 0 {
                    cycle_piece_count as u64
                } else {
                    (cycle_piece_count as u64 * orientation_count as u64)
                        / gcd(orientation_count as u64, cycle_orient_sum as u64)
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

                Cycle {
                    cycle_piece_count,
                    cycle_parity: (cycle_piece_count - 1) % 2,
                    cycle_orient_sum,
                    cycle_order: OrderFactors::<N> {
                        exps: Simd::from_array(exps),
                    },
                }
            })
        })
        .collect::<Vec<_>>();

    // Build by total piece count.
    //
    // For a fixed dst_piece_count, every destination bucket depends only on
    // smaller piece counts, so all buckets at this layer can be computed independently.
    for dst_piece_count in 1..=piece_count {
        let bucket_count = 2 * orientation_count;

        let layer_results: Vec<(usize, usize, FxHashSet<OrderFactors<N>>)> = (0..bucket_count)
            .into_par_iter()
            .map(|bucket_idx| {
                let dst_parity = bucket_idx / orientation_count;
                let dst_orient_sum = bucket_idx % orientation_count;

                let mut dst = FxHashSet::<OrderFactors<N>>::default();

                for cycle in cycles
                    .iter()
                    .filter(|c| c.cycle_piece_count <= dst_piece_count)
                {
                    let src_piece_count = dst_piece_count - cycle.cycle_piece_count;
                    let src_parity = dst_parity ^ cycle.cycle_parity;

                    let src_orient_sum = (dst_orient_sum + orientation_count
                        - (cycle.cycle_orient_sum % orientation_count))
                        % orientation_count;

                    let src = &dp[src_parity][src_orient_sum][src_piece_count];
                    if src.is_empty() {
                        continue;
                    }

                    dst.extend(src.iter().map(|order| order.lcm(&cycle.cycle_order)));
                }

                (dst_parity, dst_orient_sum, dst)
            })
            .collect();

        for (dst_parity, dst_orient_sum, set) in layer_results {
            dp[dst_parity][dst_orient_sum][dst_piece_count] = set;
        }
        // let (prev, rest) = dp.split_at_mut(dst_piece_count);
        // let dst_layer = &mut rest[0];

        // dst_layer
        //     .par_iter_mut()
        //     .enumerate()
        //     .for_each(|(dst_parity, parity_vec)| {
        //         parity_vec
        //             .iter_mut()
        //             .enumerate()
        //             .for_each(|(dst_orient_sum, dst)| {
        //                 for cycle in cycles
        //                     .iter()
        //                     .filter(|c| c.cycle_piece_count <= dst_piece_count)
        //                 {
        //                     let src_piece_count = dst_piece_count - cycle.cycle_piece_count;
        //                     let src_parity = dst_parity ^ cycle.cycle_parity;
        //                     let src_orient_sum = (dst_orient_sum + orientation_count
        //                         - (cycle.cycle_orient_sum % orientation_count))
        //                         % orientation_count;

        //                     let src = &prev[src_piece_count][src_parity][src_orient_sum];
        //                     if src.is_empty() {
        //                         continue;
        //                     }

        //                     dst.extend(src.iter().map(|order| order.lcm(&cycle.cycle_order)));
        //                 }
        //             });
        //     });
    }

    dp
}

fn main() {
    pretty_env_logger::init();

    let edge = (120, 20);
    let corner = (80, 30);

    let orgnow = Instant::now();
    
    let mut binding = [edge, corner]
        .into_par_iter()
        .map(|(piece_count, orient_count)| {
            possible_orders_for_piece_type_with_primes::<N>(piece_count, orient_count)
        })
        .collect::<Vec<_>>();
    let mut edges = std::mem::take(&mut binding[0]);
    let mut corners = std::mem::take(&mut binding[1]);

    let all_distinct_orders = DashSet::<OrderFactors<N>, FxBuildHasher>::default();
    for parity in 0..2 {
        let a = std::mem::take(&mut edges[parity][0][edge.0]);
        let b = std::mem::take(&mut corners[parity][0][corner.0]);

        let (mut outer, inner) = if a.len() >= b.len() {
            (a.into_iter().collect::<Vec<_>>(), b)
        } else {
            (b.into_iter().collect::<Vec<_>>(), a)
        };

        outer.sort_unstable_by_key(|x| std::cmp::Reverse(x.exp_sum()));

        let mut root = MaxTrieNode::<N>::new(0);
        for y in inner {
            root.insert(y);
        }

        let now = Instant::now();
        outer
            .into_par_iter()
            .fold(FxHashSet::default, |mut local_acc, order| {
                let mut cur = [0u8; N];
                root.collect_distinct_lcms(&order, &mut cur, &mut local_acc);
                local_acc
            })
            .for_each(|local_acc| {
                for order in local_acc {
                    all_distinct_orders.insert(order);
                }
            });
        debug!("Main in {:?}", now.elapsed());
    }

    println!("{:?}", orgnow.elapsed());

    println!("Total distinct orders: {}", all_distinct_orders.len());
    let mut all_combined = all_distinct_orders
        .into_iter()
        .map(|f| f.to_u64())
        .collect::<Vec<_>>();
    all_combined.sort_unstable();
    for &order in all_combined.iter().rev().take(100) {
        println!("{order}");
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

        if self.level != N {
            self.children
                .entry(v.exps[self.level])
                .or_insert_with(|| Self::new(self.level + 1))
                .insert(v);
        }
    }

    fn collect_distinct_lcms(
        &self,
        order: &OrderFactors<N>,
        cur: &mut [u8; N],
        out: &mut FxHashSet<OrderFactors<N>>,
    ) {
        if self.level == N {
            out.insert(OrderFactors {
                exps: Simd::from_array(*cur),
            });
        } else if self.subtree_max.exps.simd_gt(order.exps).to_bitmask() >> self.level == 0 {
            // If all remaining subtree exponents are <= x on remaining levels,
            // then every y in this subtree yields exactly x on remaining levels.
            let mut exps = order.exps;
            exps[..self.level].copy_from_slice(&cur[..self.level]);
            out.insert(OrderFactors { exps });
        } else {
            for (&exp, child) in &self.children {
                let old = std::mem::replace(&mut cur[self.level], order.exps[self.level].max(exp));
                child.collect_distinct_lcms(order, cur, out);
                cur[self.level] = old;
            }
        }
    }
}

#![feature(portable_simd)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_panics_doc, clippy::too_many_lines)]

use crate::trie::MaxOrderTrie;
use dashmap::DashSet;
use fxhash::{FxBuildHasher, FxHashSet};
use humanize_duration::{Truncate, prelude::DurationExt};
use log::debug;
use num_integer::gcd;
use puzzle_theory::numbers::{Int, U};
use rayon::prelude::*;
use std::{
    fmt::{Debug, Formatter},
    simd::{LaneCount, Simd, SupportedLaneCount, cmp::SimdOrd},
    time::Instant,
};

mod trie;

const N: usize = 32;

const PRIMES: [u8; N] = [
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131,
];

#[derive(Clone, PartialEq, Eq, Hash)]
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

    fn as_bigint(&self) -> Int<U> {
        let mut result = Int::one();
        for (i, p) in PRIMES.into_iter().enumerate() {
            for _ in 0..self.exps[i] {
                result *= Int::<U>::from(p);
            }
        }
        result
    }

    fn lcm(&self, other: &Self) -> Self {
        Self {
            exps: self.exps.simd_max(other.exps),
        }
    }
}

impl<const N: usize> Debug for OrderFactors<N>
where
    LaneCount<N>: SupportedLaneCount,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "OF({})", self.as_bigint())
    }
}

#[must_use]
pub fn possible_orders_for_piece_type_with_primes<const N: usize>(
    piece_count: usize,
    orientation_count: usize,
) -> Vec<Vec<FxHashSet<OrderFactors<N>>>>
where
    LaneCount<N>: SupportedLaneCount,
{
    #[allow(clippy::struct_field_names)]
    #[derive(Clone, Debug)]
    struct Cycle<const N: usize>
    where
        LaneCount<N>: SupportedLaneCount,
    {
        piece_count: usize,
        parity: usize,
        orient_sum: usize,
        order: OrderFactors<N>,
    }

    assert!(
        PRIMES.len() <= N,
        "Need OrderFactors<{}>, but got {} primes",
        N,
        PRIMES.len()
    );

    let mut dp: Vec<Vec<Vec<FxHashSet<OrderFactors<N>>>>> = (0..=piece_count)
        .map(|_| {
            (0..orientation_count)
                .map(|_| (0..2).map(|_| FxHashSet::default()).collect())
                .collect()
        })
        .collect();

    // Identity
    dp[0][0][0].insert(OrderFactors::one());

    let cycles: Vec<Cycle<N>> = (1..=piece_count)
        .flat_map(|piece_count| {
            (0..orientation_count).map(move |orient_sum| {
                let cycle_order = if orient_sum == 0 {
                    piece_count as u64
                } else {
                    (piece_count as u64 * orientation_count as u64)
                        / gcd(orientation_count as u64, orient_sum as u64)
                };

                let mut exps = [0u8; N];
                let mut n = cycle_order;

                for (i, p) in PRIMES.into_iter().enumerate() {
                    let p = u64::from(p);
                    if p * p > n && n > 1 {
                        break;
                    }
                    while n.is_multiple_of(p) {
                        exps[i] += 1;
                        n /= p;
                    }
                }

                if n > 1 {
                    let idx = PRIMES.into_iter().position(|p| u64::from(p) == n).unwrap();
                    exps[idx] += 1;
                }

                Cycle {
                    piece_count,
                    parity: (piece_count - 1) % 2,
                    orient_sum,
                    order: OrderFactors::<N> {
                        exps: Simd::from_array(exps),
                    },
                }
            })
        })
        .collect();

    // For a dst_piece_count, every destination bucket depends only on
    // smaller piece counts, so all buckets at this layer can be computed
    // independently
    for dst_piece_count in 1..=piece_count {
        let (subproblems, [problem, ..]) = dp.split_at_mut(dst_piece_count) else {
            panic!();
        };
        // TODO: we shouldn't have to loop over every single orient_sum because
        // of GCD magic
        problem
            .par_iter_mut()
            .enumerate()
            .flat_map_iter(|(dst_orient_sum, tmp)| {
                tmp.iter_mut()
                    .enumerate()
                    .map(move |(dst_parity, dst)| (dst_orient_sum, dst_parity, dst))
            })
            .for_each(|(dst_orient_sum, dst_parity, dst)| {
                for cycle in cycles
                    .iter()
                    .take_while(|c| c.piece_count <= dst_piece_count)
                {
                    let src_piece_count = dst_piece_count - cycle.piece_count;
                    let src_parity = dst_parity ^ cycle.parity;
                    let src_orient_sum =
                        (dst_orient_sum + orientation_count - cycle.orient_sum) % orientation_count;

                    let src = &subproblems[src_piece_count][src_orient_sum][src_parity];
                    if !src.is_empty() {
                        dst.extend(src.iter().map(|order| order.lcm(&cycle.order)));
                    }
                }
            });
    }

    std::mem::take(&mut dp[piece_count])
}

fn main() {
    pretty_env_logger::init();

    let edge = (120, 2);
    let corner = (80, 3);

    let start = Instant::now();

    let [mut edges, mut corners] = [edge, corner]
        .into_par_iter()
        .map(|(piece_count, orient_count)| {
            possible_orders_for_piece_type_with_primes::<N>(piece_count, orient_count)
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    debug!("DP in {}", start.elapsed().human(Truncate::Millis));

    let all_distinct_orders = DashSet::<OrderFactors<N>, FxBuildHasher>::default();
    for parity in 0..2 {
        let begin_setup = start.elapsed();
        let a = std::mem::take(&mut edges[0][parity]);
        let b = std::mem::take(&mut corners[0][parity]);

        let (outer, inner) = if a.len() >= b.len() {
            (a.into_iter().collect::<Vec<_>>(), b)
        } else {
            (b.into_iter().collect::<Vec<_>>(), a)
        };

        let mut root = MaxOrderTrie::new(0);
        for y in inner {
            root.insert(y);
        }

        let finished_setup = begin_setup.saturating_sub(start.elapsed());
        debug!("Setup in {}", finished_setup.human(Truncate::Millis));
        outer
            .into_par_iter()
            .fold(FxHashSet::default, |mut local_acc, order| {
                let mut cur = [0u8; N];
                root.collect_distinct_orders(&order, &mut cur, &mut local_acc);
                local_acc
            })
            .for_each(|local_acc| {
                for order in local_acc {
                    all_distinct_orders.insert(order);
                }
            });
        debug!(
            "Main in {}",
            start
                .elapsed()
                .saturating_sub(finished_setup)
                .human(Truncate::Millis)
        );
    }

    println!("{:?}", start.elapsed());

    println!("Total distinct orders: {}", all_distinct_orders.len());
    let mut all_combined = all_distinct_orders
        .into_iter()
        .map(|f| f.as_bigint())
        .collect::<Vec<_>>();
    all_combined.sort_unstable();
    for &order in all_combined.iter().rev().take(10) {
        println!("{order}");
    }
}

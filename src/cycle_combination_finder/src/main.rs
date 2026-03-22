#![feature(portable_simd)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_panics_doc, clippy::too_many_lines)]

use std::{
    fmt::Debug,
    simd::{LaneCount, Simd, SupportedLaneCount},
    time::Instant,
};

use dashmap::DashSet;
use fxhash::{FxBuildHasher, FxHashSet};
use humanize_duration::{Truncate, prelude::DurationExt};
use log::debug;
use ndarray::{Array2, Array3, Axis, Zip};
use num_integer::gcd;
use rayon::prelude::*;

use crate::{
    orderexps::{OrderExps, PRIMES},
    trie::MaxOrderTrie,
};

mod orderexps;
mod trie;

pub const N: usize = 32;

#[must_use]
pub fn possible_orders_for_piece_type_with_primes<const N: usize>(
    piece_count: usize,
    orientation_count: usize,
) -> Array2<FxHashSet<OrderExps<N>>>
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
        order: OrderExps<N>,
    }

    let mut dp = Array3::from_elem(
        (piece_count + 1, orientation_count, 2),
        FxHashSet::<OrderExps<N>>::default(),
    );

    // Identity
    dp[(0, 0, 0)].insert(OrderExps::one());

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
                    order: OrderExps(Simd::from_array(exps)),
                }
            })
        })
        .collect();

    // For a dst_piece_count, every destination bucket depends only on
    // smaller piece counts, so all buckets at this layer can be computed
    // independently
    for dst_piece_count in 1..=piece_count {
        let (subproblems, mut problems) = dp.view_mut().split_at(Axis(0), dst_piece_count);
        let problem = problems.index_axis_mut(Axis(0), 0);

        // TODO: we shouldn't have to loop over every single orient_sum because
        // of GCD magic
        Zip::indexed(problem)
            .into_par_iter()
            .for_each(|((dst_orient_sum, dst_parity), dst)| {
                for cycle in cycles
                    .iter()
                    .take_while(|c| c.piece_count <= dst_piece_count)
                {
                    let src_piece_count = dst_piece_count - cycle.piece_count;
                    let src_parity = dst_parity ^ cycle.parity;
                    let src_orient_sum =
                        (dst_orient_sum + orientation_count - cycle.orient_sum) % orientation_count;

                    let src = &subproblems[(src_piece_count, src_orient_sum, src_parity)];
                    if !src.is_empty() {
                        dst.extend(src.iter().map(|order| order.lcm(&cycle.order)));
                    }
                }
            });
    }

    dp.index_axis_move(Axis(0), piece_count)
}

fn main() {
    pretty_env_logger::init();

    let edge = (12, 2);
    let corner = (8, 3);

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

    let all_distinct_orders = DashSet::<OrderExps<N>, FxBuildHasher>::default();
    for parity in 0..2 {
        let begin_setup = start.elapsed();
        let a = std::mem::take(&mut edges[(0, parity)]);
        let b = std::mem::take(&mut corners[(0, parity)]);

        let (outer, inner) = if a.len() >= b.len() {
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

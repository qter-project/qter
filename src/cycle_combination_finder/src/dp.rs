#![warn(clippy::pedantic)]
#![allow(clippy::missing_panics_doc, clippy::too_many_lines)]

use std::{
    fmt::Debug,
    simd::{LaneCount, Simd, SupportedLaneCount},
};

use fxhash::FxHashSet;
use ndarray::{Array2, Array3, Axis, Zip};
use num_integer::gcd;
use rayon::prelude::*;

use crate::{
    orderexps::{OrderExps, PRIMES},
    puzzle::OrbitDef,
};

impl OrbitDef {
    #[must_use]
    pub fn possible_orders<const N: usize>(self) -> Array2<FxHashSet<OrderExps<N>>>
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

        let piece_count = self.piece_count.get() as usize;
        let orientation_count = self.orientation_count.get() as usize;
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

        let mut dp = Array3::from_elem(
            (piece_count + 1, orientation_count, 2),
            FxHashSet::<OrderExps<N>>::default(),
        );

        // Identity
        dp[(0, 0, 0)].insert(OrderExps::one());

        // For a dst_piece_count, every destination bucket depends only on
        // smaller piece counts, so all buckets at this layer can be computed
        // independently
        for dst_piece_count in 1..=piece_count {
            let (subproblems, mut problems) = dp.view_mut().split_at(Axis(0), dst_piece_count);
            let problem = problems.index_axis_mut(Axis(0), 0);

            // TODO: we shouldn't have to loop over every single orient_sum because
            // of GCD magic
            Zip::indexed(problem).into_par_iter().for_each(
                |((dst_orient_sum, dst_parity), dst)| {
                    for cycle in cycles
                        .iter()
                        .take_while(|c| c.piece_count <= dst_piece_count)
                    {
                        let src_piece_count = dst_piece_count - cycle.piece_count;
                        let src_parity = dst_parity ^ cycle.parity;
                        let src_orient_sum = (dst_orient_sum + orientation_count
                            - cycle.orient_sum)
                            % orientation_count;

                        let src = &subproblems[(src_piece_count, src_orient_sum, src_parity)];
                        if !src.is_empty() {
                            dst.extend(src.iter().map(|order| order.lcm(&cycle.order)));
                        }
                    }
                },
            );
        }

        dp.index_axis_move(Axis(0), piece_count)
    }
}

#[cfg(test)]
mod tests {
    use std::{cmp::Ordering, collections::BinaryHeap, time::Instant};

    use dashmap::DashSet;
    use fxhash::{FxBuildHasher, FxHashSet};
    use humanize_duration::{Truncate, prelude::DurationExt};
    use ndarray::Array2;
    use rayon::prelude::*;

    use crate::{
        N,
        orderexps::OrderExps,
        puzzle::{OrbitDef, OrientationSumConstraint, ParityConstraint, PuzzleDef},
        trie::MaxOrderTrie,
    };

    // struct E<'a>(&'a FxHashSet<OrderExps<N>>);
    struct E(DashSet<OrderExps<N>, FxBuildHasher>);

    impl PartialOrd for E {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for E {
        fn cmp(&self, other: &Self) -> Ordering {
            other.0.len().cmp(&self.0.len())
        }
    }

    impl PartialEq for E {
        fn eq(&self, other: &Self) -> bool {
            self.0.len() == other.0.len()
        }
    }

    impl Eq for E {}

    #[test_log::test]
    fn main() {
        let puzzle_def = PuzzleDef::from_orbit_defs_naive(
            vec![
                OrbitDef {
                    piece_count: 8.try_into().unwrap(),
                    orientation_count: 3.try_into().unwrap(),
                    orientation_sum_constraint: OrientationSumConstraint::Zero,
                    parity_constraint: ParityConstraint::None,
                },
                OrbitDef {
                    piece_count: 12.try_into().unwrap(),
                    orientation_count: 2.try_into().unwrap(),
                    orientation_sum_constraint: OrientationSumConstraint::Zero,
                    parity_constraint: ParityConstraint::None,
                },
            ],
            OrientationSumConstraint::Zero,
            ParityConstraint::Even,
        )
        .unwrap();
        let start = Instant::now();

        let all_orbit_possible_orders = puzzle_def
            .orbit_defs()
            .par_iter()
            .copied()
            .map(OrbitDef::possible_orders::<N>)
            .collect::<Vec<_>>();

        let mut orbit_possible_orders_combinations = vec![];
        let mut curr = vec![(0, 0); all_orbit_possible_orders.len()];
        loop {
            let mut end = true;
            let (puzzle_orient_sum, puzzle_parity) = curr
                .iter()
                .fold((0usize, 0usize), |acc, i| (acc.0 + i.0, acc.1 + i.1));
            let valid_parity = match puzzle_def.parity_constraint() {
                ParityConstraint::Even if puzzle_parity.is_multiple_of(2) => true,
                ParityConstraint::None => true,
                ParityConstraint::Even => false,
            };
            let valid_orient_sum = match puzzle_def.orientation_sum_constraint() {
                OrientationSumConstraint::Zero if puzzle_orient_sum == 0 => true,
                OrientationSumConstraint::None => true,
                OrientationSumConstraint::Zero => false,
            };
            if valid_orient_sum && valid_parity {
                orbit_possible_orders_combinations.push(curr.clone());
            }
            for ((orient_sum, parity), (max_orient_sum, max_parity)) in curr
                .iter_mut()
                .zip(all_orbit_possible_orders.iter().map(Array2::dim))
            {
                *orient_sum += 1;
                if *orient_sum < max_orient_sum {
                    end = false;
                    break;
                }
                *orient_sum = 0;
                *parity += 1;
                if *parity < max_parity {
                    end = false;
                    break;
                }
                *parity = 0;
            }
            if end {
                break;
            }
        }

        let all_distinct_orders = DashSet::<OrderExps<N>, FxBuildHasher>::default();
        // TODO: move out of orbit_possible_orders in the par iter loop
        // let all_orbit_possible_orders_iter = all_orbit_possible_orders
        //     .into_iter()
        //     .map(|a| a.into_par_iter())
        //     .collect::<Vec<_>>();
        // orbit_possible_orders_combinations
        //     .into_par_iter()
        //     .zip(all_orbit_possible_orders_iter)
        //     .for_each(|(orient_sum, par)| {
        //         let things = vec![];
        //     });

        orbit_possible_orders_combinations.into_par_iter().for_each(
            |orbit_possible_orders_combination| {
                let mut orbit_possible_orders = all_orbit_possible_orders
                    .iter()
                    .zip(orbit_possible_orders_combination)
                    .map(|(orbit_possible_orders, (orient_sum, parity))| {
                        E(orbit_possible_orders[(orient_sum, parity)]
                            .iter()
                            .cloned()
                            .collect::<DashSet<_, FxBuildHasher>>())
                    })
                    .collect::<BinaryHeap<_>>();
                while orbit_possible_orders.len() > 1 {
                    let acc = DashSet::<OrderExps<N>, FxBuildHasher>::default();
                    let inner = orbit_possible_orders.pop().unwrap();
                    let outer = orbit_possible_orders.pop().unwrap();
                    let mut root = MaxOrderTrie::new(0);
                    for y in inner.0 {
                        root.insert(y.clone());
                    }
                    outer
                        .0
                        .into_par_iter()
                        .fold(FxHashSet::default, |mut local_acc, order| {
                            let mut acc = [0u8; N];
                            root.collect_distinct_orders(&order, &mut acc, &mut local_acc);
                            local_acc
                        })
                        .for_each(|local_acc| {
                            for order in local_acc {
                                acc.insert(order);
                            }
                        });
                    orbit_possible_orders.push(E(acc));
                }
                let last = orbit_possible_orders.pop().unwrap();
                for order in last.0 {
                    all_distinct_orders.insert(order);
                }
            },
        );

        println!("{}", start.elapsed().human(Truncate::Micro));

        println!("Total distinct orders: {}", all_distinct_orders.len());
        let mut all_combined = all_distinct_orders
            .into_iter()
            .map(|f| f.as_bigint())
            .collect::<Vec<_>>();
        all_combined.sort_unstable();
        for &order in all_combined.iter().rev() {
            println!("{order}");
        }
        panic!();
    }
}

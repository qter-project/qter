use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    fmt::Debug,
    simd::{LaneCount, Simd, SupportedLaneCount},
};

use dashmap::DashSet;
use fxhash::{FxBuildHasher, FxHashSet};
use ndarray::{Array2, Array3, ArrayViewMut3, Axis, Zip};
use num_integer::gcd;
use rayon::prelude::*;

use crate::{
    N,
    orderexps::{OrderExps, PRIMES},
    puzzle::{OrbitDef, OrientationStatus, OrientationSumConstraint, ParityConstraint, PuzzleDef},
    trie::MaxOrderTrie,
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
        let orientation_count = self.orientation_count() as usize;
        // TODO: can we switch back to the original DP algorithm and then use 0/1
        // knapsack adding prime powers only? Similar to how Asher's modified Landau
        // works
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

        let mut dp: Array3<FxHashSet<OrderExps<N>>> =
            Array3::default((piece_count, orientation_count, 2));

        // Identity
        dp[(0, 0, 0)].insert(OrderExps::one());

        let solve_problem = |subproblems: &ArrayViewMut3<FxHashSet<OrderExps<N>>>,
                             dst_piece_count,
                             dst_orient_sum,
                             dst_parity,
                             dst: &mut FxHashSet<OrderExps<N>>| {
            for cycle in cycles
                .iter()
                .take_while(|&c| c.piece_count <= dst_piece_count)
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
        };

        for dst_piece_count in 1..piece_count {
            let (subproblems, mut problems) = dp.view_mut().split_at(Axis(0), dst_piece_count);
            let problem = problems.index_axis_mut(Axis(0), 0);

            // TODO: we shouldn't have to loop over every single orient_sum because
            // of GCD magic
            Zip::indexed(problem).into_par_iter().for_each(
                |((dst_orient_sum, dst_parity), dst)| {
                    solve_problem(
                        &subproblems,
                        dst_piece_count,
                        dst_orient_sum,
                        dst_parity,
                        dst,
                    );
                },
            );
        }

        let mut possible_orders: Array2<FxHashSet<OrderExps<N>>> = Array2::default((
            match self.orientation {
                OrientationStatus::CanOrient {
                    sum_constraint: OrientationSumConstraint::Zero,
                    ..
                }
                | OrientationStatus::CannotOrient => 1,
                OrientationStatus::CanOrient { count, .. } => count as usize,
            },
            match self.parity_constraint {
                ParityConstraint::Even => 1,
                ParityConstraint::None => 2,
            },
        ));
        let dp = dp.view_mut();
        Zip::indexed(possible_orders.view_mut())
            .into_par_iter()
            .for_each(|((dst_orient_sum, dst_parity), dst)| {
                solve_problem(&dp, piece_count, dst_orient_sum, dst_parity, dst);
            });
        possible_orders
    }
}

// struct E<'a>(&'a FxHashSet<OrderExps<N>>);
// TODO: dynamic dispatch
enum OrdWrapper {
    A(DashSet<OrderExps<N>, FxBuildHasher>),
    B(FxHashSet<OrderExps<N>>),
}

// impl IntoIterator for OrdWrapper {
//     type IntoIter;
//     type Item;

//     fn into_iter(self) -> Self::IntoIter {
//         todo!()
//     }
// }

impl PartialOrd for OrdWrapper {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrdWrapper {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (OrdWrapper::A(a), OrdWrapper::B(b)) => b.len().cmp(&a.len()),
            (OrdWrapper::A(a), OrdWrapper::A(b)) => b.len().cmp(&a.len()),
            (OrdWrapper::B(a), OrdWrapper::A(b)) => b.len().cmp(&a.len()),
            (OrdWrapper::B(a), OrdWrapper::B(b)) => b.len().cmp(&a.len()),
        }
    }
}

impl PartialEq for OrdWrapper {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (OrdWrapper::A(a), OrdWrapper::B(b)) => a.len() == b.len(),
            (OrdWrapper::A(a), OrdWrapper::A(b)) => a.len() == b.len(),
            (OrdWrapper::B(a), OrdWrapper::A(b)) => a.len() == b.len(),
            (OrdWrapper::B(a), OrdWrapper::B(b)) => a.len() == b.len(),
        }
    }
}

impl Eq for OrdWrapper {}

impl PuzzleDef {
    pub fn possible_orders(&self) -> DashSet<OrderExps<N>, FxBuildHasher> {
        let all_orbit_possible_orders = self
            .orbit_defs()
            .par_iter()
            .copied()
            .map(OrbitDef::possible_orders::<N>)
            .collect::<Vec<_>>();

        let mut orbit_possible_orders_combinations = vec![];
        let mut curr = vec![(0, 0); all_orbit_possible_orders.len()];
        loop {
            let mut end = true;
            let valid_parity =
                self.even_parity_constraints()
                    .0
                    .iter()
                    .all(|even_parity_constraint| {
                        even_parity_constraint
                            .iter()
                            .map(|&i| curr[i].1)
                            .sum::<usize>()
                            .is_multiple_of(2)
                    });
            let valid_orient_sum = match self.orientation_sum_constraint() {
                OrientationSumConstraint::Zero if curr.iter().map(|&i| i.0).sum::<usize>() == 0 => {
                    true
                }
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

        let possible_orders = DashSet::<OrderExps<N>, FxBuildHasher>::default();

        // TODO: improve naming
        orbit_possible_orders_combinations.into_par_iter().for_each(
            |orbit_possible_orders_combination| {
                let mut orbit_possible_orders = all_orbit_possible_orders
                    .iter()
                    .zip(orbit_possible_orders_combination)
                    .map(|(orbit_possible_orders, (orient_sum, parity))| {
                        OrdWrapper::B(orbit_possible_orders[(orient_sum, parity)].clone())
                    })
                    .collect::<BinaryHeap<_>>();
                while let Some(smallest) = orbit_possible_orders.pop() {
                    if let Some(smaller) = orbit_possible_orders.pop() {
                        let acc = DashSet::<OrderExps<N>, FxBuildHasher>::default();
                        let mut root = MaxOrderTrie::new(0);
                        match smallest {
                            OrdWrapper::A(a) => {
                                for y in a {
                                    root.insert(y.clone());
                                }
                            }
                            OrdWrapper::B(b) => {
                                for y in b {
                                    root.insert(y.clone());
                                }
                            }
                        }
                        match smaller {
                            OrdWrapper::A(a) => a
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
                                }),
                            OrdWrapper::B(b) => b
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
                                }),
                        }

                        orbit_possible_orders.push(OrdWrapper::A(acc));
                    } else {
                        match smallest {
                            OrdWrapper::A(a) => {
                                for order in a {
                                    possible_orders.insert(order);
                                }
                            }
                            OrdWrapper::B(b) => {
                                for order in b {
                                    possible_orders.insert(order);
                                }
                            }
                        }
                        break;
                    }
                }
            },
        );
        possible_orders
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use humanize_duration::{Truncate, prelude::DurationExt};
    use log::info;
    use puzzle_theory::numbers::{Int, U};

    use crate::puzzle::{
        cubeN::{CUBE3, CUBE4, CUBE5},
        minxN::MEGAMINX,
    };

    fn bigint(n: &'static [u64]) -> Vec<Int<U>> {
        n.iter().map(|&i| Int::<U>::from(i)).collect()
    }

    #[test_log::test]
    fn cube3_possible_orders() {
        let cube3 = &*CUBE3;
        let start = Instant::now();
        let possible_orders = cube3.possible_orders();
        info!(
            "Possible orders in {}",
            start.elapsed().human(Truncate::Micro)
        );

        assert_eq!(possible_orders.len(), 73);

        let mut possible_orders = possible_orders
            .into_iter()
            .map(|f| f.as_bigint())
            .collect::<Vec<_>>();
        possible_orders.sort_unstable();
        assert_eq!(
            possible_orders.rchunks(10).next().unwrap(),
            bigint(&[360, 420, 462, 495, 504, 630, 720, 840, 990, 1260])
        );
    }

    #[test_log::test]
    fn cube4_possible_orders() {
        let cube4 = &*CUBE4;
        let start = Instant::now();
        let possible_orders = cube4.possible_orders();
        info!(
            "Possible orders in {}",
            start.elapsed().human(Truncate::Micro)
        );

        assert_eq!(possible_orders.len(), 877);

        let mut possible_orders = possible_orders
            .into_iter()
            .map(|f| f.as_bigint())
            .collect::<Vec<_>>();
        possible_orders.sort_unstable();
        assert_eq!(
            possible_orders.rchunks(10).next().unwrap(),
            bigint(&[
                360360, 376740, 406980, 437580, 471240, 489060, 510510, 556920, 720720, 765765
            ])
        );
    }

    #[test_log::test]
    fn cube5_possible_orders() {
        let cube5 = &*CUBE5;
        let start = Instant::now();
        let possible_orders = cube5.possible_orders();
        info!(
            "Possible orders in {}",
            start.elapsed().human(Truncate::Micro)
        );

        assert_eq!(possible_orders.len(), 1770);

        let mut possible_orders = possible_orders
            .into_iter()
            .map(|f| f.as_bigint())
            .collect::<Vec<_>>();
        possible_orders.sort_unstable();
        assert_eq!(
            possible_orders.rchunks(10).next().unwrap(),
            bigint(&[
                58198140, 70450380, 77597520, 78738660, 93933840, 104984880, 116396280, 140900760,
                232792560, 281801520
            ])
        );
    }

    #[test_log::test]
    fn megaminx_possible_orders() {
        let megaminx = &*MEGAMINX;
        let start = Instant::now();
        let possible_orders = megaminx.possible_orders();
        info!(
            "Possible orders in {}",
            start.elapsed().human(Truncate::Micro)
        );

        assert_eq!(possible_orders.len(), 1278);

        let mut possible_orders = possible_orders
            .into_iter()
            .map(|f| f.as_bigint())
            .collect::<Vec<_>>();
        possible_orders.sort_unstable();
        assert_eq!(
            possible_orders.rchunks(10).next().unwrap(),
            bigint(&[
                278460, 282744, 308880, 332640, 353430, 360360, 432432, 471240, 540540, 720720,
            ])
        );
        panic!();
    }
}

use std::{cmp::Ordering, collections::BinaryHeap, fmt::Debug, num::NonZeroU16, simd::Simd};

use dashmap::DashSet;
use fxhash::{FxBuildHasher, FxHashSet};
use ndarray::{Array2, Array3, ArrayRef3, Axis, Zip};
use num_integer::gcd;
use rayon::prelude::*;

use crate::{
    N, PRIME_AFTER_LAST, PRIMES,
    number_theory::divisors,
    orderexps::OrderExps,
    puzzle::{OrbitDef, OrientationStatus, OrientationSumConstraint, ParityConstraint, PuzzleDef},
    trie::MaxOrderTrie,
};

pub type OrdersSet<const N: usize> = FxHashSet<OrderExps<N>>;

pub enum OrbitPossibleOrders<const N: usize> {
    CombinedOrders(OrdersSet<N>),
    ParityOrders {
        even_parity_orders: OrdersSet<N>,
        maybe_odd_parity_orders: Option<OrdersSet<N>>,
    },
}

impl OrbitDef {
    #[must_use]
    pub fn possible_orders2<const N: usize>(self) -> Array2<OrdersSet<N>> {
        #[allow(clippy::struct_field_names)]
        #[derive(Clone, Debug)]
        struct Cycle<const N: usize> {
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

        let mut dp: Array3<OrdersSet<N>> = Array3::default((piece_count, orientation_count, 2));

        // Identity
        dp[(0, 0, 0)].insert(OrderExps::one());

        let solve_problem = |subproblems: &ArrayRef3<OrdersSet<N>>,
                             dst_piece_count,
                             dst_orient_sum,
                             dst_parity,
                             dst: &mut OrdersSet<N>| {
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
            Zip::indexed(problem).par_for_each(|(dst_orient_sum, dst_parity), dst| {
                solve_problem(
                    &subproblems,
                    dst_piece_count,
                    dst_orient_sum,
                    dst_parity,
                    dst,
                );
            });
        }

        let mut possible_orders: Array2<OrdersSet<N>> = Array2::default((
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
        Zip::indexed(possible_orders.view_mut()).par_for_each(
            |(dst_orient_sum, dst_parity), dst| {
                solve_problem(&dp, piece_count, dst_orient_sum, dst_parity, dst);
            },
        );
        possible_orders
    }

    #[must_use]
    pub fn possible_orders<const N: usize>(
        self,
        combine_parity_orders: bool,
    ) -> OrbitPossibleOrders<N> {
        assert!(
            self.piece_count.get() < u16::from(PRIME_AFTER_LAST),
            "Piece count too large"
        );
        let piece_count = self.piece_count.get();
        let orientation_count = self.orientation_count();

        let invalid_prime_index = PRIMES.partition_point(|&prime| u16::from(prime) <= piece_count);
        if piece_count == 1 {
            let mut combined_orders = FxHashSet::default();
            combined_orders.insert(OrderExps::one());
            return OrbitPossibleOrders::CombinedOrders(combined_orders);
        }
        let mut orbit_possible_orders = if combine_parity_orders {
            OrbitPossibleOrders::CombinedOrders(FxHashSet::default())
        } else {
            OrbitPossibleOrders::ParityOrders {
                even_parity_orders: FxHashSet::default(),
                maybe_odd_parity_orders: match self.parity_constraint {
                    ParityConstraint::Even => None,
                    ParityConstraint::None => Some(FxHashSet::default()),
                },
            }
        };

        let extend_orientation_order_factors = {
            let orientation_order_factors = divisors(orientation_count);
            move |order: &OrderExps<N>, orders_set: &mut OrdersSet<N>| {
                orders_set.extend(orientation_order_factors.iter().map(
                    |orientation_order_factor| order.clone() * orientation_order_factor.clone(),
                ));
            }
        };
        let mut piece_count_prime_power_base = None;

        let mut stack = vec![(0, piece_count, OrderExps::one())];
        while let Some((prime_index, remaining_pieces_count, mut acc_order)) = stack.pop() {
            if prime_index == invalid_prime_index {
                match &mut orbit_possible_orders {
                    OrbitPossibleOrders::CombinedOrders(combined_orders) => {
                        extend_orientation_order_factors(&acc_order, combined_orders);
                    }
                    OrbitPossibleOrders::ParityOrders {
                        even_parity_orders,
                        maybe_odd_parity_orders,
                    } => {
                        // Do we have the two prime power (is it even)?
                        let odd_parity = acc_order.0[0] != 0;

                        if odd_parity {
                            if let Some(odd_parity_orders) = maybe_odd_parity_orders {
                                extend_orientation_order_factors(&acc_order, odd_parity_orders);
                            }
                            if remaining_pieces_count >= 2 {
                                extend_orientation_order_factors(&acc_order, even_parity_orders);
                            }
                        } else {
                            extend_orientation_order_factors(&acc_order, even_parity_orders);
                            if remaining_pieces_count == 2
                                && let Some(odd_parity_orders) = maybe_odd_parity_orders
                            {
                                acc_order.0[0] = acc_order.0[0].checked_add(1).unwrap();
                                extend_orientation_order_factors(&acc_order, odd_parity_orders);
                            }
                        }
                    }
                }
                continue;
            }

            // skip the prime
            stack.push((prime_index + 1, remaining_pieces_count, acc_order.clone()));

            // or add all powers of prime
            let prime = PRIMES[prime_index];
            let mut prime_power_exps = OrderExps::one();
            prime_power_exps.0[prime_index] = 1;
            let mut prime_power = u16::from(prime);
            while prime_power <= remaining_pieces_count {
                if prime_power == piece_count {
                    piece_count_prime_power_base = Some(prime);
                }
                stack.push((
                    prime_index + 1,
                    remaining_pieces_count - prime_power,
                    acc_order.clone() * prime_power_exps.clone(),
                ));
                prime_power_exps.0[prime_index] += 1;
                prime_power *= u16::from(prime);
            }
        }

        if let Some(base_prime) = piece_count_prime_power_base
            && let OrientationStatus::CanOrient {
                count: _,
                sum_constraint: OrientationSumConstraint::Zero,
            } = self.orientation
        {
            let mut gcd = piece_count;
            while !u16::from(orientation_count).is_multiple_of(gcd) {
                gcd = gcd.div_exact(u16::from(base_prime)).unwrap();
            }
            for multiple in (gcd..=piece_count).step_by(usize::from(gcd)) {
                let multiple = NonZeroU16::new(multiple).unwrap();
                if multiple.get() == 1 {
                    continue;
                }

                let impossible = OrderExps::<N>::try_from(self.piece_count).unwrap()
                    * OrderExps::<N>::try_from(multiple).unwrap();
                match &mut orbit_possible_orders {
                    OrbitPossibleOrders::CombinedOrders(combined_orders) => {
                        combined_orders.remove(&impossible);
                    }
                    OrbitPossibleOrders::ParityOrders {
                        even_parity_orders,
                        maybe_odd_parity_orders,
                    } => {
                        even_parity_orders.remove(&impossible);
                        if let Some(odd_parity_orders) = maybe_odd_parity_orders {
                            odd_parity_orders.remove(&impossible);
                        }
                    }
                }
            }
        }
        orbit_possible_orders
    }

    #[must_use]
    pub fn combined_parity_possible_orders<const N: usize>(self) -> OrdersSet<N> {
        let OrbitPossibleOrders::CombinedOrders(combined_orders) = self.possible_orders(true)
        else {
            panic!();
        };
        combined_orders
    }

    #[must_use]
    pub fn uncombined_parity_possible_orders<const N: usize>(
        self,
    ) -> (OrdersSet<N>, Option<OrdersSet<N>>) {
        let OrbitPossibleOrders::ParityOrders {
            even_parity_orders,
            maybe_odd_parity_orders,
        } = self.possible_orders(false)
        else {
            panic!();
        };
        (even_parity_orders, maybe_odd_parity_orders)
    }
}

enum LcmOrders<'a> {
    CombinedOrders(DashSet<OrderExps<N>, FxBuildHasher>),
    OrbitOrders(&'a OrdersSet<N>),
}

impl PartialOrd for LcmOrders<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LcmOrders<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (LcmOrders::CombinedOrders(a), LcmOrders::OrbitOrders(b)) => a.len().cmp(&b.len()),
            (LcmOrders::CombinedOrders(a), LcmOrders::CombinedOrders(b)) => a.len().cmp(&b.len()),
            (LcmOrders::OrbitOrders(a), LcmOrders::CombinedOrders(b)) => a.len().cmp(&b.len()),
            (LcmOrders::OrbitOrders(a), LcmOrders::OrbitOrders(b)) => a.len().cmp(&b.len()),
        }
        .reverse()
    }
}

impl PartialEq for LcmOrders<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for LcmOrders<'_> {}

impl PuzzleDef {
    pub fn possible_orders(&self) -> DashSet<OrderExps<N>, FxBuildHasher> {
        let all_orbit_orders = self
            .orbit_defs()
            .par_iter()
            .copied()
            .map(OrbitDef::possible_orders2::<N>)
            .collect::<Vec<_>>();

        let mut orbit_orders_cart_product = vec![];
        let mut curr = vec![(0, 0); all_orbit_orders.len()];
        let even_parity_constraints = self.even_parity_constraints();
        loop {
            let mut end = true;
            let invalid_parity = (0..even_parity_constraints.rows()).any(|i| {
                let row = even_parity_constraints.row(i);
                row.iter()
                    .zip(curr.iter())
                    .fold(
                        false,
                        |parity, (use_it, &(_, c))| {
                            if use_it { parity ^ (c != 0) } else { parity }
                        },
                    )
            });
            if !invalid_parity {
                orbit_orders_cart_product.push(curr.clone());
            }
            for ((orient_sum, parity), (max_orient_sum, max_parity)) in curr
                .iter_mut()
                .zip(all_orbit_orders.iter().map(Array2::dim))
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

        orbit_orders_cart_product
            .into_par_iter()
            .for_each(|orbit_orders_combination| {
                let mut smallest_len_orders = all_orbit_orders
                    .iter()
                    .zip(orbit_orders_combination)
                    .map(|(orbit_orders, (orient_sum, parity))| {
                        LcmOrders::OrbitOrders(&orbit_orders[(orient_sum, parity)])
                    })
                    .collect::<BinaryHeap<_>>();
                while let Some(smallest_len) = smallest_len_orders.pop() {
                    if let Some(smaller_len) = smallest_len_orders.pop() {
                        let combined = DashSet::<OrderExps<N>, FxBuildHasher>::default();
                        let mut root = MaxOrderTrie::new(0);
                        match smallest_len {
                            LcmOrders::CombinedOrders(smallest_len) => {
                                for y in smallest_len {
                                    root.insert(y);
                                }
                            }
                            LcmOrders::OrbitOrders(smallest_len) => {
                                for y in smallest_len {
                                    root.insert(y.clone());
                                }
                            }
                        }
                        match smaller_len {
                            LcmOrders::CombinedOrders(smaller_len) => smaller_len
                                .into_par_iter()
                                .fold(FxHashSet::default, |mut local_acc, order| {
                                    let mut acc = [0u8; N];
                                    root.collect_distinct_orders(&order, &mut acc, &mut local_acc);
                                    local_acc
                                })
                                .for_each(|local_acc| {
                                    for order in local_acc {
                                        combined.insert(order);
                                    }
                                }),
                            LcmOrders::OrbitOrders(smaller_len) => smaller_len
                                .into_par_iter()
                                .fold(FxHashSet::default, |mut local_acc, order| {
                                    let mut acc = [0u8; N];
                                    root.collect_distinct_orders(order, &mut acc, &mut local_acc);
                                    local_acc
                                })
                                .for_each(|local_acc| {
                                    for order in local_acc {
                                        combined.insert(order);
                                    }
                                }),
                        }

                        smallest_len_orders.push(LcmOrders::CombinedOrders(combined));
                    } else {
                        let all_combined = smallest_len;
                        match all_combined {
                            LcmOrders::CombinedOrders(all_combined) => {
                                for order in all_combined {
                                    possible_orders.insert(order);
                                }
                            }
                            LcmOrders::OrbitOrders(all_combined) => {
                                for order in all_combined {
                                    possible_orders.insert(order.clone());
                                }
                            }
                        }
                        break;
                    }
                }
            });
        possible_orders
    }
}

#[cfg(test)]
mod orbit {
    use std::{num::NonZeroU16, time::Instant};

    use humanize_duration::{Truncate, prelude::DurationExt};
    use log::info;

    use crate::{
        N,
        puzzle::{OrbitDef, OrientationStatus, OrientationSumConstraint, ParityConstraint},
    };

    const COMPOSITE_PIECE_COUNT: NonZeroU16 = NonZeroU16::new(120).unwrap();
    const PRIME_PIECE_COUNT: NonZeroU16 = NonZeroU16::new(113).unwrap();
    const PRIME_POWER_PIECE_COUNT: NonZeroU16 = NonZeroU16::new(64).unwrap();
    const COMPOSITE_ORIENTATION: u8 = 20;
    const PRIME_ORIENTATION: u8 = 13;
    const PRIME_POWER_ORIENTATION: u8 = 16;

    fn test_possible_orders_zero_orientation_sum_any_parity(
        orbit_def: OrbitDef,
        expected_len: usize,
        expected_highest: u64,
    ) {
        let start = Instant::now();
        let possible_orders = orbit_def.combined_parity_possible_orders::<N>();
        info!(
            "Possible orbit orders for {orbit_def:?} in {}",
            start.elapsed().human(Truncate::Micro)
        );

        assert_eq!(possible_orders.len(), expected_len);
        let actual_highest = possible_orders
            .iter()
            .map(|possible_order| u64::try_from(possible_order.as_bigint()).unwrap())
            .max()
            .unwrap();
        assert_eq!(expected_highest, actual_highest);
    }

    // #[test_log::test]
    // fn foo() {
    //     let start = Instant::now();
    //     let orbit_def = OrbitDef {
    //         piece_count: PRIME_POWER_PIECE_COUNT,
    //         orientation: OrientationStatus::CanOrient {
    //             count: PRIME_POWER_PIECE_COUNT.get().try_into().unwrap(),
    //             sum_constraint: OrientationSumConstraint::Zero,
    //         },
    //         parity_constraint: ParityConstraint::None,
    //     };
    //     let possible_orders = orbit_def.possible_orders2::<N>();
    //     info!(
    //         "Possible orbit orders for {orbit_def:?} in {}",
    //         start.elapsed().human(Truncate::Micro)
    //     );
    //     panic!();
    // }

    #[test_log::test]
    fn edge_cases() {
        // two orientation count is an edge case since it is the only number
        // where the number of cycles cannot simply be greater than 1
        let orbit_def = OrbitDef {
            piece_count: 12.try_into().unwrap(),
            orientation: OrientationStatus::CanOrient {
                count: 2,
                sum_constraint: OrientationSumConstraint::Zero,
            },
            parity_constraint: ParityConstraint::None,
        };

        let start = Instant::now();
        let possible_orders = orbit_def.possible_orders2::<N>();
        info!(
            "Possible orbit orders for {orbit_def:?} in {}",
            start.elapsed().human(Truncate::Micro)
        );

        println!("{:?}", {
            let mut a = possible_orders[(0, 0)]
                .iter()
                .map(|a| u16::try_from(a.as_bigint()).unwrap())
                .collect::<Vec<_>>();
            a.sort_unstable();
            a
        });
        println!("{:?}", {
            let mut a = possible_orders[(0, 1)]
                .iter()
                .map(|a| u16::try_from(a.as_bigint()).unwrap())
                .collect::<Vec<_>>();
            a.sort_unstable();
            a
        });
        // println!("{:?}", possible_orders[(0, 0)].len());
        // println!("{:?}", possible_orders[(0, 1)].len());
        let mut possible_orders = possible_orders
            .into_iter()
            .flat_map(|f| f.into_iter().map(|a| a.as_bigint()))
            .collect::<Vec<_>>();
        possible_orders.sort_unstable();
        possible_orders.dedup();

        assert_eq!(possible_orders.len(), 0);
    }

    #[test_log::test]
    fn all_composite_piece_counts() {
        for (orientation_count, expected_len, expected_highest) in [
            (COMPOSITE_ORIENTATION, 99622, 107084577600),
            (PRIME_ORIENTATION, 75770, 69604975440),
            (PRIME_POWER_ORIENTATION, 89594, 85667662080),
        ] {
            test_possible_orders_zero_orientation_sum_any_parity(
                OrbitDef {
                    piece_count: COMPOSITE_PIECE_COUNT,
                    orientation: OrientationStatus::CanOrient {
                        count: orientation_count,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                    parity_constraint: ParityConstraint::None,
                },
                expected_len,
                expected_highest,
            );
        }
    }

    #[test_log::test]
    fn all_prime_piece_counts() {
        for (orientation_count, expected_len, expected_highest) in [
            (COMPOSITE_ORIENTATION, 73860, 53542288800),
            (PRIME_ORIENTATION, 55880, 34802487720),
            (PRIME_POWER_ORIENTATION, 66402, 42833831040),
        ] {
            test_possible_orders_zero_orientation_sum_any_parity(
                OrbitDef {
                    piece_count: PRIME_PIECE_COUNT,
                    orientation: OrientationStatus::CanOrient {
                        count: orientation_count,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                    parity_constraint: ParityConstraint::None,
                },
                expected_len,
                expected_highest,
            );
        }
    }

    #[test_log::test]
    fn all_prime_power_piece_counts() {
        for (orientation_count, expected_len, expected_highest) in [
            (COMPOSITE_ORIENTATION, 6222, 40840800),
            (PRIME_ORIENTATION, 4526, 26546520),
            (PRIME_POWER_ORIENTATION, 5534, 32672640),
        ] {
            test_possible_orders_zero_orientation_sum_any_parity(
                OrbitDef {
                    piece_count: PRIME_POWER_PIECE_COUNT,
                    orientation: OrientationStatus::CanOrient {
                        count: orientation_count,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                    parity_constraint: ParityConstraint::None,
                },
                expected_len,
                expected_highest,
            );
        }
    }

    #[test_log::test]
    fn same_piece_count_and_orientation_count() {
        for (same_count, expected_len, expected_highest) in [
            (COMPOSITE_PIECE_COUNT, 155425, 642507465600),
            (PRIME_PIECE_COUNT, 68050, 302513931720),
            (PRIME_POWER_PIECE_COUNT, 6966, 130690560),
        ] {
            test_possible_orders_zero_orientation_sum_any_parity(
                OrbitDef {
                    piece_count: same_count,
                    orientation: OrientationStatus::CanOrient {
                        count: same_count.get().try_into().unwrap(),
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                    parity_constraint: ParityConstraint::None,
                },
                expected_len,
                expected_highest,
            );
        }
    }

    #[test_log::test]
    fn all_parities() {
        panic!()
    }

    #[test_log::test]
    fn all_orientation_sums() {
        panic!();
    }
}

#[cfg(test)]
mod puzzle {
    use std::time::Instant;

    use humanize_duration::{Truncate, prelude::DurationExt};
    use log::info;
    use puzzle_theory::numbers::{Int, U};

    use crate::puzzle::{
        EvenParityConstraints, OrbitDef, OrientationStatus, OrientationSumConstraint,
        ParityConstraint, PuzzleDef,
        cubeN::{CUBE2, CUBE3, CUBE4, CUBE5},
        minxN::MEGAMINX,
        misc::SLOW,
    };

    fn bigints(n: &[u64]) -> Vec<Int<U>> {
        n.iter().map(|&i| Int::<U>::from(i)).collect()
    }

    fn test_possible_orders(
        puzzle_def: &PuzzleDef,
        expected_len: usize,
        expected_highest_ten: [u64; 10],
    ) {
        let start = Instant::now();
        let possible_orders = puzzle_def.possible_orders();
        info!(
            "Possible puzzle orders for {puzzle_def:?} in {}",
            start.elapsed().human(Truncate::Micro)
        );

        assert_eq!(possible_orders.len(), expected_len);

        let mut possible_orders = possible_orders
            .into_iter()
            .map(|f| f.as_bigint())
            .collect::<Vec<_>>();
        possible_orders.sort_unstable();
        assert_eq!(
            possible_orders.rchunks(10).next().unwrap(),
            bigints(expected_highest_ten.as_slice())
        );
    }

    #[test_log::test]
    fn cube2() {
        test_possible_orders(&CUBE2, 17, [8, 9, 10, 12, 15, 18, 21, 30, 36, 45]);
    }

    #[test_log::test]
    fn cube3() {
        test_possible_orders(
            &CUBE3,
            73,
            [360, 420, 462, 495, 504, 630, 720, 840, 990, 1260],
        );
    }

    #[test_log::test]
    fn cube4() {
        test_possible_orders(
            &CUBE4,
            877,
            [
                360360, 376740, 406980, 437580, 471240, 489060, 510510, 556920, 720720, 765765,
            ],
        );
    }

    #[test_log::test]
    fn cube5() {
        test_possible_orders(
            &CUBE5,
            1770,
            [
                58198140, 70450380, 77597520, 78738660, 93933840, 104984880, 116396280, 140900760,
                232792560, 281801520,
            ],
        );
    }

    #[test_log::test]
    fn megaminx() {
        test_possible_orders(
            &MEGAMINX,
            1278,
            [
                278460, 282744, 308880, 332640, 353430, 360360, 432432, 471240, 540540, 720720,
            ],
        );
    }

    #[test_log::test]
    fn slow() {
        test_possible_orders(
            &SLOW,
            24820,
            [
                569729160, 595675080, 617795640, 629909280, 669278610, 698377680, 730122120,
                845404560, 944863920, 1396755360,
            ],
        );
    }

    #[test_log::test]
    fn misc() {
        let tests = vec![(
            PuzzleDef::new(
                vec![
                    OrbitDef {
                        piece_count: 120.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 2,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                        parity_constraint: ParityConstraint::None,
                    },
                    OrbitDef {
                        piece_count: 80.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 3,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                        parity_constraint: ParityConstraint::None,
                    },
                ],
                EvenParityConstraints(vec![vec![0, 1]]),
            )
            .unwrap(),
            1234189,
            [
                48572104155120,
                48734191265760,
                51483005814240,
                51705788294160,
                55271704728240,
                56241383758560,
                57761421157440,
                72201776446800,
                86176313823600,
                86642131736160,
            ],
        )];

        for (puzzle_def, expected_len, expected_highest_len) in tests {
            test_possible_orders(&puzzle_def, expected_len, expected_highest_len);
        }
    }
}

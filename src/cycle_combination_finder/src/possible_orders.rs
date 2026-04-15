use std::{
    borrow::Cow,
    cmp::{Ordering, max},
    collections::BinaryHeap,
    num::NonZeroU16,
};

use bitgauss::BitMatrix;
use dashmap::DashSet;
use fxhash::{FxBuildHasher, FxHashMap, FxHashSet};
use rayon::prelude::*;

use crate::{
    PRIME_AFTER_LAST, PRIMES,
    ac3::backtrack_ac3,
    gauss_jordan_without_zero_rows,
    number_theory::divisors,
    orderexps::OrderExps,
    puzzle::{OrbitDef, OrientationStatus, OrientationSumConstraint, ParityConstraint, PuzzleDef},
    trie::MaxOrderTrie,
};

pub type OrdersSet<const N: usize> = FxHashSet<OrderExps<N>>;
pub type OrdersDashSet<const N: usize> = DashSet<OrderExps<N>, FxBuildHasher>;

pub enum OrbitPossibleOrders<const N: usize> {
    CombinedOrders(OrdersSet<N>),
    ParityOrders {
        even_parity_orders: OrdersSet<N>,
        maybe_odd_parity_orders: Option<OrdersSet<N>>,
    },
}

impl OrbitDef {
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
        let mut orbit_possible_orders = if combine_parity_orders {
            let mut combined_orders = OrdersSet::default();
            if piece_count == 1 {
                combined_orders.insert(OrderExps::one());
            }
            OrbitPossibleOrders::CombinedOrders(combined_orders)
        } else {
            let mut even_parity_orders = OrdersSet::default();
            if piece_count == 1 {
                even_parity_orders.insert(OrderExps::one());
            }
            OrbitPossibleOrders::ParityOrders {
                even_parity_orders,
                maybe_odd_parity_orders: match self.parity_constraint {
                    ParityConstraint::Even => None,
                    ParityConstraint::None => Some(OrdersSet::default()),
                },
            }
        };
        if piece_count == 1 {
            return orbit_possible_orders;
        }

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
            for multiple in (gcd..=piece_count)
                .step_by(usize::from(gcd))
                .skip(if gcd == 1 { 1 } else { 0 })
            {
                let multiple = NonZeroU16::new(multiple).unwrap();

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

#[derive(Debug, Clone)]
enum LcmOrders<const N: usize> {
    CombinedOrders(OrdersDashSet<N>),
    OrbitOrders(OrdersSet<N>),
}

impl<const N: usize> LcmOrders<N> {
    fn len(&self) -> usize {
        match self {
            LcmOrders::CombinedOrders(self_) => self_.len(),
            LcmOrders::OrbitOrders(self_) => self_.len(),
        }
    }
}

impl<const N: usize> PartialOrd for LcmOrders<N> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<const N: usize> Ord for LcmOrders<N> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.len().cmp(&other.len()).reverse()
    }
}

impl<const N: usize> PartialEq for LcmOrders<N> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<const N: usize> Eq for LcmOrders<N> {}

fn combine_heap<const N: usize>(
    mut smallest_len_orders: BinaryHeap<Cow<LcmOrders<N>>>,
) -> Cow<LcmOrders<N>> {
    while let Some(smallest_len) = smallest_len_orders.pop() {
        if let Some(smaller_len) = smallest_len_orders.pop() {
            let combined = OrdersDashSet::default();
            let mut root = MaxOrderTrie::new(0);
            match smallest_len {
                Cow::Borrowed(LcmOrders::CombinedOrders(smallest_len)) => {
                    for y in smallest_len.iter() {
                        root.insert(y.clone());
                    }
                }
                Cow::Owned(LcmOrders::CombinedOrders(smallest_len)) => {
                    for y in smallest_len {
                        root.insert(y);
                    }
                }
                Cow::Borrowed(LcmOrders::OrbitOrders(smallest_len)) => {
                    for y in smallest_len {
                        root.insert(y.clone());
                    }
                }
                Cow::Owned(LcmOrders::OrbitOrders(smallest_len)) => {
                    for y in smallest_len {
                        root.insert(y);
                    }
                }
            }

            match &*smaller_len {
                LcmOrders::CombinedOrders(smaller_len) => {
                    root.par_collect_distinct_orders(smaller_len, &combined);
                }
                LcmOrders::OrbitOrders(smaller_len) => {
                    root.par_collect_distinct_orders(smaller_len, &combined);
                }
            }

            smallest_len_orders.push(Cow::Owned(LcmOrders::CombinedOrders(combined)));
        } else {
            return smallest_len;
        }
    }
    Cow::Owned(LcmOrders::OrbitOrders(OrdersSet::default()))
}

impl PuzzleDef {
    fn connected_component_possible_orders<const N: usize>(
        &self,
        connected_component: &[usize],
    ) -> LcmOrders<N> {
        let even_parity_constraints = self.even_parity_constraints();

        if let [singular_component] = *connected_component {
            let orbit_def = self.orbit_defs()[singular_component];
            return LcmOrders::OrbitOrders(match orbit_def.parity_constraint {
                ParityConstraint::Even => {
                    let (component_possible_orders, None) =
                        orbit_def.uncombined_parity_possible_orders()
                    else {
                        panic!();
                    };
                    component_possible_orders
                }
                ParityConstraint::None => orbit_def.combined_parity_possible_orders(),
            });
        }

        let mut connected_component_parity_constraints = BitMatrix::build(
            even_parity_constraints.rows(),
            connected_component.len(),
            |i, j| even_parity_constraints[(i, j + connected_component[0])],
        );

        gauss_jordan_without_zero_rows(
            &mut connected_component_parity_constraints,
            even_parity_constraints.rows(),
        );
        let possible_assignments = backtrack_ac3(&connected_component_parity_constraints);

        let mut possible_assignments_symbols = vec![];
        for possible_assignment in &possible_assignments {
            let mut possible_assignment_symbols = FxHashSet::default();
            for (i, &parity) in possible_assignment.iter().enumerate() {
                let symbol = i * 2 + usize::from(parity);
                assert!(possible_assignment_symbols.insert(symbol));
            }
            possible_assignments_symbols.push(possible_assignment_symbols);
        }

        let mut work = connected_component
            .iter()
            .enumerate()
            .flat_map(|(symbol, &orbit_index)| {
                let (even_parity_orders, Some(odd_parity_orders)) =
                    self.orbit_defs()[orbit_index].uncombined_parity_possible_orders()
                else {
                    panic!();
                };
                [
                    (symbol * 2, LcmOrders::OrbitOrders(even_parity_orders)),
                    (symbol * 2 + 1, LcmOrders::OrbitOrders(odd_parity_orders)),
                ]
                .into_iter()
            })
            .collect::<FxHashMap<usize, LcmOrders<N>>>();

        let mut next_symbol = connected_component.len() * 2;
        loop {
            let (symbol_pair, max_count) = work
                .keys()
                .enumerate()
                .flat_map(|(i, &a)| {
                    let possible_assignments_symbols = &possible_assignments_symbols;
                    work.keys().skip(i + 1).map(move |&b| {
                        let count = possible_assignments_symbols
                            .iter()
                            .filter(|s| s.contains(&a) && s.contains(&b))
                            .count();
                        ([a, b], count)
                    })
                })
                .max_by_key(|&(_, count)| count)
                .unwrap();
            match max_count {
                0 => panic!(),
                1 => break,
                2.. => (),
            }

            let mut keep = [false, false];
            for possible_assignment_symbols in &mut possible_assignments_symbols {
                match (
                    possible_assignment_symbols.contains(&symbol_pair[0]),
                    possible_assignment_symbols.contains(&symbol_pair[1]),
                ) {
                    (true, true) => {
                        possible_assignment_symbols.remove(&symbol_pair[0]);
                        possible_assignment_symbols.remove(&symbol_pair[1]);
                        possible_assignment_symbols.insert(next_symbol);
                    }
                    (true, false) => {
                        keep[0] = true;
                    }
                    (false, true) => {
                        keep[1] = true;
                    }
                    (false, false) => (),
                }
            }

            let tmp = if keep[1] {
                None
            } else {
                Some(work.remove(&symbol_pair[1]).unwrap())
            };

            let mut smallest = if keep[0] {
                Cow::Borrowed(work.get(&symbol_pair[0]).unwrap())
            } else {
                Cow::Owned(work.remove(&symbol_pair[0]).unwrap())
            };

            let mut smaller = match tmp {
                Some(v) => Cow::Owned(v),
                None => Cow::Borrowed(work.get(&symbol_pair[1]).unwrap()),
            };

            if smaller.len() < smallest.len() {
                std::mem::swap(&mut smallest, &mut smaller);
            }

            let combined_orders = OrdersDashSet::default();
            let mut root = MaxOrderTrie::new(0);

            match smallest {
                Cow::Borrowed(LcmOrders::CombinedOrders(smallest)) => {
                    for order in smallest.iter() {
                        root.insert(order.clone());
                    }
                }
                Cow::Owned(LcmOrders::CombinedOrders(smallest)) => {
                    for order in smallest {
                        root.insert(order);
                    }
                }
                Cow::Borrowed(LcmOrders::OrbitOrders(smallest)) => {
                    for order in smallest {
                        root.insert(order.clone());
                    }
                }
                Cow::Owned(LcmOrders::OrbitOrders(smallest)) => {
                    for order in smallest {
                        root.insert(order);
                    }
                }
            }
            match &*smaller {
                LcmOrders::CombinedOrders(smaller) => {
                    root.par_collect_distinct_orders(smaller, &combined_orders);
                }
                LcmOrders::OrbitOrders(smaller) => {
                    root.par_collect_distinct_orders(smaller, &combined_orders);
                }
            }

            work.insert(next_symbol, LcmOrders::CombinedOrders(combined_orders));
            next_symbol += 1;
        }

        // This would be have caught at the guard clause
        assert_ne!(work.len(), 1);

        let possible_orders = OrdersDashSet::default();
        possible_assignments_symbols
            .into_par_iter()
            .for_each(|possible_assignment_symbols| {
                let smallest_len_orders = possible_assignment_symbols
                    .into_iter()
                    .map(|possible_assignment_symbol| {
                        Cow::Borrowed(work.get(&possible_assignment_symbol).unwrap())
                    })
                    .collect::<BinaryHeap<_>>();

                let all_combined = combine_heap(smallest_len_orders);
                match all_combined {
                    Cow::Borrowed(LcmOrders::CombinedOrders(all_combined)) => {
                        for order in all_combined.iter() {
                            possible_orders.insert(order.clone());
                        }
                    }
                    Cow::Owned(LcmOrders::CombinedOrders(all_combined)) => {
                        for order in all_combined {
                            possible_orders.insert(order);
                        }
                    }
                    Cow::Borrowed(LcmOrders::OrbitOrders(all_combined)) => {
                        for order in all_combined {
                            possible_orders.insert(order.clone());
                        }
                    }
                    Cow::Owned(LcmOrders::OrbitOrders(all_combined)) => {
                        for order in all_combined {
                            possible_orders.insert(order);
                        }
                    }
                }
            });

        LcmOrders::CombinedOrders(possible_orders)
    }

    pub fn possible_orders<const N: usize>(&self) -> OrdersDashSet<N> {
        let smallest_len_orders = self
            .connected_components()
            .par_iter()
            .map(|connected_component| {
                self.connected_component_possible_orders::<N>(connected_component)
            })
            .fold(OrdersDashSet::default, |acc, component_possible_orders| {
                let mut root = MaxOrderTrie::new(0);
                // TODO: is the other order faster?
                for order in acc.iter() {
                    root.insert(order.clone());
                }
                match component_possible_orders {
                    LcmOrders::CombinedOrders(component_possible_orders) => {
                        root.par_collect_distinct_orders(&component_possible_orders, &acc);
                    }
                    LcmOrders::OrbitOrders(component_possible_orders) => {
                        root.par_collect_distinct_orders(&component_possible_orders, &acc);
                    }
                }
                acc
            })
            .map(|component_possible_orders| {
                Cow::Owned(LcmOrders::CombinedOrders(component_possible_orders))
            })
            .collect::<BinaryHeap<_>>();

        let all_combined = combine_heap(smallest_len_orders);
        match all_combined.into_owned() {
            LcmOrders::CombinedOrders(all_combined) => all_combined,
            LcmOrders::OrbitOrders(all_combined) => all_combined.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod orbit {
    use std::{num::NonZeroU16, time::Instant};

    use humanize_duration::{Truncate, prelude::DurationExt};
    use log::info;

    use crate::{
        N,
        puzzle::{
            OrbitDef, OrientationStatus, OrientationSumConstraint, ParityConstraint, cubeN::CUBE5,
            minxN::MINX5,
        },
    };

    const COMPOSITE_PIECE_COUNT: NonZeroU16 = NonZeroU16::new(120).unwrap();
    const PRIME_PIECE_COUNT: NonZeroU16 = NonZeroU16::new(113).unwrap();
    const PRIME_POWER_PIECE_COUNT: NonZeroU16 = NonZeroU16::new(64).unwrap();
    const COMPOSITE_ORIENTATION: u8 = 20;
    const PRIME_ORIENTATION: u8 = 13;
    const PRIME_POWER_ORIENTATION: u8 = 16;

    #[derive(Clone, Copy)]
    struct Expected {
        highest: u64,
        combined_len: usize,
        uncombined_lens: (usize, Option<usize>),
    }

    const DEBUG: bool = false;

    fn test_possible_orders(orbit_def: OrbitDef, expected: Expected) {
        let mut start = Instant::now();
        let possible_orders = orbit_def.combined_parity_possible_orders::<N>();
        info!(
            "Combined orbit orders for {orbit_def:#?} in {}",
            start.elapsed().human(Truncate::Micro)
        );
        if DEBUG {
            if possible_orders.len() != expected.combined_len {
                println!(
                    "Expected: {} (actual: {})",
                    expected.combined_len,
                    possible_orders.len(),
                );
            }
        } else {
            assert_eq!(possible_orders.len(), expected.combined_len);
        }
        let actual_highest = possible_orders
            .iter()
            .map(|possible_order| u64::try_from(possible_order.as_bigint()).unwrap())
            .max()
            .unwrap();
        if DEBUG {
            if expected.highest != actual_highest {
                println!("Expected: {} (actual: {actual_highest})", expected.highest);
            }
        } else {
            assert_eq!(expected.highest, actual_highest);
        }

        start = Instant::now();
        let (even_parity_possible_orders, maybe_odd_parity_possible_orders) =
            orbit_def.uncombined_parity_possible_orders::<N>();
        info!(
            "Uncombined orbit orders for {orbit_def:#?} in {}",
            start.elapsed().human(Truncate::Micro)
        );
        if DEBUG {
            if even_parity_possible_orders.len() != expected.uncombined_lens.0 {
                println!(
                    "Expected: {} (actual: {})",
                    expected.uncombined_lens.0,
                    even_parity_possible_orders.len()
                );
            }
        } else {
            assert_eq!(
                even_parity_possible_orders.len(),
                expected.uncombined_lens.0
            );
        }
        match (maybe_odd_parity_possible_orders, expected.uncombined_lens.1) {
            (None, None) => (),
            (Some(odd_parity_possible_orders), Some(expected_len)) => {
                if DEBUG {
                    if odd_parity_possible_orders.len() != expected_len {
                        println!(
                            "Expected: {expected_len} (actual: {})",
                            odd_parity_possible_orders.len()
                        );
                    }
                } else {
                    assert_eq!(odd_parity_possible_orders.len(), expected_len);
                }
                let mut possible_orders2 = even_parity_possible_orders;
                possible_orders2.extend(odd_parity_possible_orders);
                assert!(
                    possible_orders2 == possible_orders,
                    "possible_order2 != possible_orders"
                );
            }
            _ => panic!(),
        }
    }

    #[test_log::test]
    fn puzzle_orbits() {
        for (&orbit_def, expected) in CUBE5.orbit_defs().iter().zip([
            Expected {
                highest: 45,
                combined_len: 17,
                uncombined_lens: (13, Some(9)),
            },
            Expected {
                highest: 120,
                combined_len: 32,
                uncombined_lens: (27, Some(21)),
            },
            Expected {
                highest: 840,
                combined_len: 111,
                uncombined_lens: (94, Some(75)),
            },
            Expected {
                highest: 840,
                combined_len: 111,
                uncombined_lens: (94, Some(75)),
            },
            Expected {
                highest: 840,
                combined_len: 111,
                uncombined_lens: (94, Some(75)),
            },
        ]) {
            test_possible_orders(orbit_def, expected);
        }

        for (&orbit_def, expected) in MINX5.orbit_defs().iter().zip([
            Expected {
                highest: 1260,
                combined_len: 105,
                uncombined_lens: (89, None),
            },
            Expected {
                highest: 9240,
                combined_len: 267,
                uncombined_lens: (239, None),
            },
            Expected {
                highest: 1021020,
                combined_len: 2083,
                uncombined_lens: (1879, None),
            },
            Expected {
                highest: 1021020,
                combined_len: 2083,
                uncombined_lens: (1879, None),
            },
            Expected {
                highest: 1021020,
                combined_len: 2083,
                uncombined_lens: (1879, None),
            },
        ]) {
            test_possible_orders(orbit_def, expected);
        }
    }

    #[test_log::test]
    fn composite_piece_counts() {
        for (orientation_count, mut expected) in [
            (
                COMPOSITE_ORIENTATION,
                Expected {
                    highest: 107084577600,
                    combined_len: 99622,
                    uncombined_lens: (95101, Some(81435)),
                },
            ),
            (
                PRIME_ORIENTATION,
                Expected {
                    highest: 69604975440,
                    combined_len: 75770,
                    uncombined_lens: (70713, Some(58098)),
                },
            ),
            (
                PRIME_POWER_ORIENTATION,
                Expected {
                    highest: 85667662080,
                    combined_len: 89594,
                    uncombined_lens: (86485, Some(75583)),
                },
            ),
        ] {
            test_possible_orders(
                OrbitDef {
                    piece_count: COMPOSITE_PIECE_COUNT,
                    orientation: OrientationStatus::CanOrient {
                        count: orientation_count,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                    parity_constraint: ParityConstraint::None,
                },
                expected,
            );
            test_possible_orders(
                OrbitDef {
                    piece_count: COMPOSITE_PIECE_COUNT,
                    orientation: OrientationStatus::CanOrient {
                        count: orientation_count,
                        sum_constraint: OrientationSumConstraint::None,
                    },
                    parity_constraint: ParityConstraint::None,
                },
                expected,
            );
            expected.uncombined_lens.1 = None;
            test_possible_orders(
                OrbitDef {
                    piece_count: COMPOSITE_PIECE_COUNT,
                    orientation: OrientationStatus::CanOrient {
                        count: orientation_count,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                    parity_constraint: ParityConstraint::Even,
                },
                expected,
            );
            test_possible_orders(
                OrbitDef {
                    piece_count: COMPOSITE_PIECE_COUNT,
                    orientation: OrientationStatus::CanOrient {
                        count: orientation_count,
                        sum_constraint: OrientationSumConstraint::None,
                    },
                    parity_constraint: ParityConstraint::Even,
                },
                expected,
            );
        }
    }

    #[test_log::test]
    fn prime_piece_counts_zero_orientation_sum_constraint() {
        for (orientation_count, mut expected) in [
            (
                COMPOSITE_ORIENTATION,
                Expected {
                    highest: 53542288800,
                    combined_len: 73860,
                    uncombined_lens: (70528, Some(60037)),
                },
            ),
            (
                PRIME_ORIENTATION,
                Expected {
                    highest: 34802487720,
                    combined_len: 55880,
                    uncombined_lens: (52127, Some(42638)),
                },
            ),
            (
                PRIME_POWER_ORIENTATION,
                Expected {
                    highest: 42833831040,
                    combined_len: 66402,
                    uncombined_lens: (64114, Some(55663)),
                },
            ),
        ] {
            test_possible_orders(
                OrbitDef {
                    piece_count: PRIME_PIECE_COUNT,
                    orientation: OrientationStatus::CanOrient {
                        count: orientation_count,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                    parity_constraint: ParityConstraint::None,
                },
                expected,
            );
            expected.uncombined_lens.1 = None;
            test_possible_orders(
                OrbitDef {
                    piece_count: PRIME_PIECE_COUNT,
                    orientation: OrientationStatus::CanOrient {
                        count: orientation_count,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                    parity_constraint: ParityConstraint::Even,
                },
                expected,
            );
        }
    }

    #[test_log::test]
    fn prime_piece_counts_no_orientation_sum_constraint() {
        for (orientation_count, mut expected) in [
            (
                COMPOSITE_ORIENTATION,
                Expected {
                    highest: 53542288800,
                    combined_len: 73865,
                    uncombined_lens: (70533, Some(60037)),
                },
            ),
            (
                PRIME_ORIENTATION,
                Expected {
                    highest: 34802487720,
                    combined_len: 55881,
                    uncombined_lens: (52128, Some(42638)),
                },
            ),
            (
                PRIME_POWER_ORIENTATION,
                Expected {
                    highest: 42833831040,
                    combined_len: 66406,
                    uncombined_lens: (64118, Some(55663)),
                },
            ),
        ] {
            test_possible_orders(
                OrbitDef {
                    piece_count: PRIME_PIECE_COUNT,
                    orientation: OrientationStatus::CanOrient {
                        count: orientation_count,
                        sum_constraint: OrientationSumConstraint::None,
                    },
                    parity_constraint: ParityConstraint::None,
                },
                expected,
            );
            expected.uncombined_lens.1 = None;
            test_possible_orders(
                OrbitDef {
                    piece_count: PRIME_PIECE_COUNT,
                    orientation: OrientationStatus::CanOrient {
                        count: orientation_count,
                        sum_constraint: OrientationSumConstraint::None,
                    },
                    parity_constraint: ParityConstraint::Even,
                },
                expected,
            );
        }
    }

    #[test_log::test]
    fn prime_power_piece_counts_zero_orientation_sum_constraint() {
        for (orientation_count, mut expected) in [
            (
                COMPOSITE_ORIENTATION,
                Expected {
                    highest: 40840800,
                    combined_len: 6222,
                    uncombined_lens: (5889, Some(4868)),
                },
            ),
            (
                PRIME_ORIENTATION,
                Expected {
                    highest: 26546520,
                    combined_len: 4526,
                    uncombined_lens: (4145, Some(3319)),
                },
            ),
            (
                PRIME_POWER_ORIENTATION,
                Expected {
                    highest: 32672640,
                    combined_len: 5534,
                    uncombined_lens: (5308, Some(4466)),
                },
            ),
        ] {
            test_possible_orders(
                OrbitDef {
                    piece_count: PRIME_POWER_PIECE_COUNT,
                    orientation: OrientationStatus::CanOrient {
                        count: orientation_count,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                    parity_constraint: ParityConstraint::None,
                },
                expected,
            );
            expected.uncombined_lens.1 = None;
            test_possible_orders(
                OrbitDef {
                    piece_count: PRIME_POWER_PIECE_COUNT,
                    orientation: OrientationStatus::CanOrient {
                        count: orientation_count,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                    parity_constraint: ParityConstraint::Even,
                },
                expected,
            );
        }
    }

    #[test_log::test]
    fn prime_power_piece_counts_no_orientation_sum_constraint() {
        for (orientation_count, mut expected) in [
            (
                COMPOSITE_ORIENTATION,
                Expected {
                    highest: 40840800,
                    combined_len: 6224,
                    uncombined_lens: (5889, Some(4870)),
                },
            ),
            (
                PRIME_ORIENTATION,
                Expected {
                    highest: 26546520,
                    combined_len: 4527,
                    uncombined_lens: (4145, Some(3320)),
                },
            ),
            (
                PRIME_POWER_ORIENTATION,
                Expected {
                    highest: 32672640,
                    combined_len: 5535,
                    uncombined_lens: (5308, Some(4467)),
                },
            ),
        ] {
            test_possible_orders(
                OrbitDef {
                    piece_count: PRIME_POWER_PIECE_COUNT,
                    orientation: OrientationStatus::CanOrient {
                        count: orientation_count,
                        sum_constraint: OrientationSumConstraint::None,
                    },
                    parity_constraint: ParityConstraint::None,
                },
                expected,
            );
            expected.uncombined_lens.1 = None;
            test_possible_orders(
                OrbitDef {
                    piece_count: PRIME_POWER_PIECE_COUNT,
                    orientation: OrientationStatus::CanOrient {
                        count: orientation_count,
                        sum_constraint: OrientationSumConstraint::None,
                    },
                    parity_constraint: ParityConstraint::Even,
                },
                expected,
            );
        }
    }

    #[test_log::test]
    fn same_piece_count_and_orientation_count_zero_orientation_sum_constraint() {
        for (same_count, mut expected) in [
            (
                COMPOSITE_PIECE_COUNT,
                Expected {
                    highest: 642507465600,
                    combined_len: 155425,
                    uncombined_lens: (149639, Some(129050)),
                },
            ),
            (
                PRIME_PIECE_COUNT,
                Expected {
                    highest: 302513931720,
                    combined_len: 68050,
                    uncombined_lens: (63474, Some(51862)),
                },
            ),
            (
                PRIME_POWER_PIECE_COUNT,
                Expected {
                    highest: 130690560,
                    combined_len: 6966,
                    uncombined_lens: (6740, Some(5722)),
                },
            ),
        ] {
            test_possible_orders(
                OrbitDef {
                    piece_count: same_count,
                    orientation: OrientationStatus::CanOrient {
                        count: same_count.get().try_into().unwrap(),
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                    parity_constraint: ParityConstraint::None,
                },
                expected,
            );
            expected.uncombined_lens.1 = None;
            test_possible_orders(
                OrbitDef {
                    piece_count: same_count,
                    orientation: OrientationStatus::CanOrient {
                        count: same_count.get().try_into().unwrap(),
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                    parity_constraint: ParityConstraint::Even,
                },
                expected,
            );
        }
    }

    #[test_log::test]
    fn same_piece_count_and_orientation_count_no_orientation_sum_constraint() {
        for (same_count, mut expected) in [
            (
                COMPOSITE_PIECE_COUNT,
                Expected {
                    highest: 642507465600,
                    combined_len: 155425,
                    uncombined_lens: (149639, Some(129050)),
                },
            ),
            (
                PRIME_PIECE_COUNT,
                Expected {
                    highest: 302513931720,
                    combined_len: 68051,
                    uncombined_lens: (63475, Some(51862)),
                },
            ),
            (
                PRIME_POWER_PIECE_COUNT,
                Expected {
                    highest: 130690560,
                    combined_len: 6967,
                    uncombined_lens: (6740, Some(5723)),
                },
            ),
        ] {
            test_possible_orders(
                OrbitDef {
                    piece_count: same_count,
                    orientation: OrientationStatus::CanOrient {
                        count: same_count.get().try_into().unwrap(),
                        sum_constraint: OrientationSumConstraint::None,
                    },
                    parity_constraint: ParityConstraint::None,
                },
                expected,
            );
            expected.uncombined_lens.1 = None;
            test_possible_orders(
                OrbitDef {
                    piece_count: same_count,
                    orientation: OrientationStatus::CanOrient {
                        count: same_count.get().try_into().unwrap(),
                        sum_constraint: OrientationSumConstraint::None,
                    },
                    parity_constraint: ParityConstraint::Even,
                },
                expected,
            );
        }
    }
}

#[cfg(test)]
mod puzzle {
    use std::time::Instant;

    use humanize_duration::{Truncate, prelude::DurationExt};
    use log::info;
    use puzzle_theory::numbers::{Int, U};

    use crate::{
        N,
        puzzle::{
            PuzzleDef,
            cubeN::{CUBE2, CUBE3, CUBE4, CUBE5, CUBE6, CUBE7, CUBE8},
            minxN::{MINX3, MINX4, MINX5},
            misc::{SLOW1, SLOW2, SLOW3, SLOW4},
        },
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
        let possible_orders = puzzle_def.possible_orders::<N>();
        info!(
            "Possible puzzle orders for {puzzle_def:#?} in {}",
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
    fn cubemax() {
        for cubemax in [&CUBE6, &CUBE7, &CUBE8] {
            test_possible_orders(
                cubemax,
                1920,
                [
                    535422888, 594914320, 669278610, 764889840, 892371480, 1070845776, 1338557220,
                    1784742960, 2677114440, 5354228880,
                ],
            );
        }
    }

    #[test_log::test]
    fn minx3() {
        test_possible_orders(
            &MINX3,
            1278,
            [
                278460, 282744, 308880, 332640, 353430, 360360, 432432, 471240, 540540, 720720,
            ],
        );
    }

    #[test_log::test]
    fn minx4() {
        test_possible_orders(
            &MINX4,
            74304,
            [
                38818159380,
                40156716600,
                41495273820,
                46581791256,
                49794328584,
                58227239070,
                62242910730,
                77636318760,
                82990547640,
                116454478140,
            ],
        );
    }

    #[test_log::test]
    fn minx5() {
        test_possible_orders(
            &MINX5,
            531653,
            [
                877874012935920,
                890488576177200,
                952435607563440,
                986757611439600,
                1068586291412640,
                1184109133727520,
                1241870554884960,
                1335732864265800,
                1357393397199840,
                1413291546707040,
            ],
        );
    }

    #[test_log::test]
    fn slow1() {
        test_possible_orders(
            &SLOW1,
            24820,
            [
                569729160, 595675080, 617795640, 629909280, 669278610, 698377680, 730122120,
                845404560, 944863920, 1396755360,
            ],
        );
    }

    #[test_log::test]
    fn slow2() {
        test_possible_orders(
            &SLOW2,
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
        );
    }

    #[test_log::test]
    fn slow3() {
        test_possible_orders(
            &SLOW3,
            2079018,
            [
                485721041551200,
                487341912657600,
                514830058142400,
                517057882941600,
                552717047282400,
                562413837585600,
                577614211574400,
                722017764468000,
                861763138236000,
                866421317361600,
            ],
        );
    }

    #[test_log::test]
    fn slow4() {
        test_possible_orders(
            &SLOW4,
            3631922,
            [
                2036090095799760,
                2069784258141600,
                2119937320060560,
                2137172582825280,
                2368218267455040,
                2671465728531600,
                2960272834318800,
                3104676387212400,
                3205758874237920,
                5342931457063200,
            ],
        );
    }
}

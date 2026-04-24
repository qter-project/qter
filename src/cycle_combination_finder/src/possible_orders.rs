use std::{borrow::Cow, cmp::Ordering};

use bitgauss::BitMatrix;
use dashmap::DashSet;
use fxhash::{FxBuildHasher, FxHashMap, FxHashSet};
use rayon::prelude::*;

use crate::{
    FIRST_129_PRIMES,
    ac3::backtrack_ac3,
    gauss_jordan_without_zero_rows,
    number_theory::divisors,
    orderexps::OrderExps,
    puzzle::{OrbitDef, OrientationStatus, OrientationSumConstraint, ParityConstraint, PuzzleDef},
    trie::MaxOrderTrie,
};

pub type OrdersSet<const N: usize> = FxHashSet<OrderExps<N>>;
pub type OrdersDashSet<const N: usize> = DashSet<OrderExps<N>, FxBuildHasher>;

#[derive(Debug)]
pub enum OrbitPossibleOrders<const N: usize> {
    CombinedOrders(OrdersSet<N>),
    ParityOrders {
        even_parity_orders: OrdersSet<N>,
        maybe_odd_parity_orders: Option<OrdersSet<N>>,
    },
}

impl OrbitDef {
    /// Compute all possible orders of elements for this orbit.
    #[must_use]
    fn possible_orders<const N: usize>(
        self,
        combine_parity_orders: bool,
    ) -> OrbitPossibleOrders<N> {
        let piece_count = self.piece_count.get();
        let orientation_count = self.orientation_count();
        #[allow(clippy::missing_panics_doc)]
        {
            assert!(self.piece_count.get() < FIRST_129_PRIMES[N]);
            assert!(u16::from(orientation_count) < FIRST_129_PRIMES[N]);
        }

        let invalid_prime_index = FIRST_129_PRIMES.partition_point(|&prime| prime <= piece_count);
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

        let mut stack = vec![(0, piece_count, OrderExps::one())];
        while let Some((prime_index, remaining_pieces_count, acc_order)) = stack.pop() {
            if prime_index == invalid_prime_index {
                match &mut orbit_possible_orders {
                    OrbitPossibleOrders::CombinedOrders(combined_orders) => {
                        combined_orders.insert(acc_order);
                    }
                    OrbitPossibleOrders::ParityOrders {
                        even_parity_orders,
                        maybe_odd_parity_orders,
                    } => {
                        // Do we have the two prime power (is it even)?
                        let odd_parity = acc_order.0[0] != 0;

                        if odd_parity {
                            if let Some(odd_parity_orders) = maybe_odd_parity_orders {
                                odd_parity_orders.insert(acc_order.clone());
                            }
                            if remaining_pieces_count >= 2 {
                                even_parity_orders.insert(acc_order);
                            }
                        } else {
                            even_parity_orders.insert(acc_order);
                        }
                    }
                }
                continue;
            }

            // skip the prime
            stack.push((prime_index + 1, remaining_pieces_count, acc_order.clone()));

            // or add all powers of prime
            let prime = FIRST_129_PRIMES[prime_index];
            let mut prime_power_exps = OrderExps::one();
            prime_power_exps.0[prime_index] = 1;
            let mut prime_power = prime;
            while prime_power <= remaining_pieces_count {
                stack.push((
                    prime_index + 1,
                    remaining_pieces_count - prime_power,
                    acc_order.clone() * prime_power_exps.clone(),
                ));
                prime_power_exps.0[prime_index] += 1;
                prime_power *= prime;
            }
        }

        let extend_orientation_order_factors = {
            let orientation_order_factors = divisors(orientation_count);
            let maybe_prime_power_piece_count = if let OrientationStatus::CanOrient {
                count: _,
                sum_constraint: OrientationSumConstraint::Zero,
            } = self.orientation
            {
                let piece_count_orderexps = OrderExps::try_from(self.piece_count).unwrap();
                piece_count_orderexps
                    .is_prime_power()
                    .then_some(piece_count_orderexps)
            } else {
                None
            };

            move |orders_set: &mut OrdersSet<N>| {
                *orders_set = orders_set
                    .drain()
                    .flat_map(|order| {
                        let n = if let Some(prime_power_piece_count) =
                            &maybe_prime_power_piece_count
                            && order == prime_power_piece_count.clone()
                        {
                            1
                        } else {
                            orientation_order_factors.len()
                        };

                        orientation_order_factors
                            .iter()
                            .map(move |orientation_order_factor| {
                                order.clone() * orientation_order_factor.clone()
                            })
                            .take(n)
                    })
                    .collect();
            }
        };

        match &mut orbit_possible_orders {
            OrbitPossibleOrders::CombinedOrders(combined_orders) => {
                extend_orientation_order_factors(combined_orders);
            }
            OrbitPossibleOrders::ParityOrders {
                even_parity_orders,
                maybe_odd_parity_orders,
            } => {
                extend_orientation_order_factors(even_parity_orders);
                if let Some(odd_parity_orders) = maybe_odd_parity_orders {
                    extend_orientation_order_factors(odd_parity_orders);
                }
            }
        }

        orbit_possible_orders
    }

    #[must_use]
    pub fn combined_parity_possible_orders<const N: usize>(self) -> OrdersSet<N> {
        #[allow(clippy::missing_panics_doc)]
        let OrbitPossibleOrders::CombinedOrders(combined_orders) = self.possible_orders(true)
        else {
            // `true` returns the `CombinedOrders` variant
            unreachable!();
        };
        combined_orders
    }

    #[must_use]
    pub fn uncombined_parity_possible_orders<const N: usize>(
        self,
    ) -> (OrdersSet<N>, Option<OrdersSet<N>>) {
        #[allow(clippy::missing_panics_doc)]
        let OrbitPossibleOrders::ParityOrders {
            even_parity_orders,
            maybe_odd_parity_orders,
        } = self.possible_orders(false)
        else {
            // `false` returns the `ParityOrders` variant
            unreachable!();
        };
        (even_parity_orders, maybe_odd_parity_orders)
    }
}

#[derive(Debug, Clone)]
pub enum LcmOrders<const N: usize> {
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

    fn combine_cost(a: &Self, b: &Self) -> usize {
        a.len().max(b.len())
    }
}

impl<const N: usize> From<Cow<'_, LcmOrders<N>>> for MaxOrderTrie<N> {
    fn from(lcm_orders: Cow<LcmOrders<N>>) -> Self {
        let mut root = MaxOrderTrie::new(0);
        match lcm_orders {
            Cow::Borrowed(LcmOrders::CombinedOrders(lcm_orders)) => {
                for y in lcm_orders.iter() {
                    root.insert(y.clone());
                }
            }
            Cow::Owned(LcmOrders::CombinedOrders(lcm_orders)) => {
                for y in lcm_orders {
                    root.insert(y);
                }
            }
            Cow::Borrowed(LcmOrders::OrbitOrders(lcm_orders)) => {
                for y in lcm_orders {
                    root.insert(y.clone());
                }
            }
            Cow::Owned(LcmOrders::OrbitOrders(lcm_orders)) => {
                for y in lcm_orders {
                    root.insert(y);
                }
            }
        }
        root
    }
}

fn combine<'a, const N: usize>(
    mut smallest: Cow<'a, LcmOrders<N>>,
    mut smaller: Cow<'a, LcmOrders<N>>,
) -> Cow<'a, LcmOrders<N>> {
    if smallest.len() == 0 {
        return smaller;
    }
    if smaller.len() < smallest.len() {
        std::mem::swap(&mut smallest, &mut smaller);
    }

    let combined = OrdersDashSet::default();
    MaxOrderTrie::from(smallest).par_collect_distinct_orders(smaller, &combined);

    Cow::Owned(LcmOrders::CombinedOrders(combined))
}

impl<const N: usize> PuzzleDef<N> {
    /// Compute all possible orders for a connected component of orbits.
    fn connected_component_possible_orders(&self, connected_component: &[usize]) -> LcmOrders<N> {
        let even_parity_constraints = self.even_parity_constraints();

        match *connected_component {
            [] => panic!("it is a logic error for a connected component to have no orbits"),
            [singular_component] => {
                let orbit_def = self.orbit_defs()[singular_component];
                return LcmOrders::OrbitOrders(match orbit_def.parity_constraint {
                    ParityConstraint::Even => {
                        let (component_possible_orders, None) =
                            orbit_def.uncombined_parity_possible_orders()
                        else {
                            // When this orbit is set to "must be even," `OrbitDef::possible_orders`
                            // does not record odd parity orders.
                            unreachable!();
                        };
                        component_possible_orders
                    }
                    ParityConstraint::None => orbit_def.combined_parity_possible_orders(),
                });
            }
            _ => (),
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
        for possible_assignment in possible_assignments {
            let mut possible_assignment_symbols = FxHashSet::default();
            for (i, parity) in possible_assignment.enumerate() {
                let symbol = i * 2 + usize::from(parity);
                if !possible_assignment_symbols.insert(symbol) {
                    // The same symbol cannot be in the same assignment. I.e. one orbit cannot have
                    // two possible assignments
                    unreachable!();
                }
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
                    // We would have broken on the guard clause earlier if we only record even
                    // parity orders
                    unreachable!();
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
            let ([smallest_symbol, smaller_symbol], max_count, _) = work
                .keys()
                .enumerate()
                .flat_map(|(i, &smallest_symbol)| {
                    let possible_assignments_symbols = &possible_assignments_symbols;
                    let work = &work;
                    work.keys().skip(i + 1).map(move |&smaller_symbol| {
                        let count = possible_assignments_symbols
                            .iter()
                            .filter(|s| s.contains(&smallest_symbol) && s.contains(&smaller_symbol))
                            .count();

                        let cost = LcmOrders::combine_cost(
                            work.get(&smaller_symbol).unwrap(),
                            work.get(&smallest_symbol).unwrap(),
                        );

                        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
                        let count_cost_product_ln = (count as f64).ln() + (cost as f64).ln();

                        (
                            [smallest_symbol, smaller_symbol],
                            count,
                            count_cost_product_ln,
                        )
                    })
                })
                .max_by(|&(_, _, a), &(_, _, b)| a.partial_cmp(&b).unwrap_or(Ordering::Equal))
                // `work` must not be empty because we asserted `connected_component` must not be empty earlier
                .unwrap();
            match max_count {
                0 => panic!(),
                1 => break,
                2.. => (),
            }

            let mut keep_smallest = false;
            let mut keep_smaller = false;
            for possible_assignment_symbols in &mut possible_assignments_symbols {
                match (
                    possible_assignment_symbols.contains(&smallest_symbol),
                    possible_assignment_symbols.contains(&smaller_symbol),
                ) {
                    (true, true) => {
                        possible_assignment_symbols.remove(&smallest_symbol);
                        possible_assignment_symbols.remove(&smaller_symbol);
                        possible_assignment_symbols.insert(next_symbol);
                    }
                    (true, false) => {
                        keep_smallest = true;
                    }
                    (false, true) => {
                        keep_smaller = true;
                    }
                    (false, false) => (),
                }
            }

            let (smallest, smaller) = match (keep_smallest, keep_smaller) {
                (true, true) => (
                    Cow::Borrowed(work.get(&smallest_symbol).unwrap()),
                    Cow::Borrowed(work.get(&smaller_symbol).unwrap()),
                ),
                (true, false) => {
                    let tmp = work.remove(&smaller_symbol).unwrap();
                    (
                        Cow::Borrowed(work.get(&smallest_symbol).unwrap()),
                        Cow::Owned(tmp),
                    )
                }
                (false, true) => (
                    Cow::Owned(work.remove(&smallest_symbol).unwrap()),
                    Cow::Borrowed(work.get(&smaller_symbol).unwrap()),
                ),
                (false, false) => (
                    Cow::Owned(work.remove(&smallest_symbol).unwrap()),
                    Cow::Owned(work.remove(&smaller_symbol).unwrap()),
                ),
            };

            work.insert(next_symbol, combine(smallest, smaller).into_owned());
            next_symbol += 1;
        }

        let possible_orders = OrdersDashSet::default();
        possible_assignments_symbols
            .into_par_iter()
            .for_each(|possible_assignment_symbols| {
                let all_combined = possible_assignment_symbols
                    .into_par_iter()
                    .map(|possible_assignment_symbol| {
                        Cow::Borrowed(work.get(&possible_assignment_symbol).unwrap())
                    })
                    .reduce(
                        || Cow::Owned(LcmOrders::OrbitOrders(OrdersSet::default())),
                        combine,
                    );
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

    #[must_use]
    pub fn possible_orders(&self) -> OrdersDashSet<N> {
        let all_combined = self
            .connected_components()
            .par_iter()
            .map(|connected_component| {
                Cow::Owned(self.connected_component_possible_orders(connected_component))
            })
            .reduce(
                || Cow::Owned(LcmOrders::OrbitOrders(OrdersSet::default())),
                combine,
            );
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
    use log::trace;

    use crate::{
        P9, P17, P33, P65,
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
        match orbit_def.piece_count.get() {
            1..P9 => test_possible_orders_n::<8>(orbit_def, expected),
            P9..P17 => test_possible_orders_n::<16>(orbit_def, expected),
            P17..P33 => test_possible_orders_n::<32>(orbit_def, expected),
            P33..P65 => test_possible_orders_n::<64>(orbit_def, expected),
            _ => panic!("piece count too big"),
        }
    }

    fn test_possible_orders_n<const N: usize>(orbit_def: OrbitDef, expected: Expected) {
        let mut start = Instant::now();
        let possible_orders = orbit_def.combined_parity_possible_orders::<N>();
        trace!(
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
        trace!(
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
            _ => panic!("expected mismatch"),
        }
    }

    #[test_log::test]
    fn edge_cases() {
        // caused the old CCF to fail
        test_possible_orders(
            OrbitDef {
                piece_count: 8.try_into().unwrap(),
                orientation: OrientationStatus::CanOrient {
                    count: 16,
                    sum_constraint: OrientationSumConstraint::Zero,
                },
                parity_constraint: ParityConstraint::None,
            },
            Expected {
                highest: 240,
                combined_len: 30,
                uncombined_lens: (28, Some(17)),
            },
        );
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
    fn landau() {
        for (piece_count, landau) in [
            1, 1, 2, 3, 4, 6, 6, 12, 15, 20, 30, 30, 60, 60, 84, 105, 140, 210, 210, 420, 420, 420,
            420, 840, 840, 1260, 1260, 1540, 2310, 2520, 4620, 4620, 5460, 5460, 9240, 9240, 13860,
            13860, 16380, 16380, 27720, 30030, 32760, 60060, 60060, 60060, 60060, 120120, 120120,
            180180, 180180, 180180, 180180, 360360, 360360, 360360, 360360, 471240, 510510, 556920,
            1021020,
        ]
        .into_iter()
        .enumerate()
        .skip(1)
        {
            let orbit_def = OrbitDef {
                piece_count: NonZeroU16::new(u16::try_from(piece_count).unwrap()).unwrap(),
                orientation: OrientationStatus::CannotOrient,
                parity_constraint: ParityConstraint::None,
            };
            assert_eq!(
                orbit_def
                    .combined_parity_possible_orders::<64>()
                    .into_iter()
                    .map(|possible_order| u64::try_from(possible_order.as_bigint()).unwrap())
                    .max()
                    .unwrap(),
                landau
            );
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
    use std::{str::FromStr, time::Instant};

    use humanize_duration::{Truncate, prelude::DurationExt};
    use log::info;
    use puzzle_theory::numbers::{Int, U};

    use crate::puzzle::{
        PuzzleDef,
        cubeN::{CUBE2, CUBE3, CUBE4, CUBE5, CUBE6, CUBE7, CUBE8},
        minxN::{MINX3, MINX4, MINX5, MINX6},
        misc::{BIG1, BIG2, BIG3},
    };

    const DEBUG: bool = false;

    fn test_possible_orders_big<const N: usize>(
        puzzle_def: &PuzzleDef<N>,
        expected_len: usize,
        expected_highest_ten: [Int<U>; 10],
    ) {
        let start = Instant::now();
        let possible_orders = puzzle_def.possible_orders();
        info!(
            "Possible puzzle orders for {puzzle_def:?} in {}",
            start.elapsed().human(Truncate::Micro)
        );

        if DEBUG {
            if possible_orders.len() != expected_len {
                println!(
                    "Expected: {} (actual: {})",
                    expected_len,
                    possible_orders.len(),
                );
            }
        } else {
            assert_eq!(possible_orders.len(), expected_len);
        }

        let mut possible_orders = possible_orders
            .into_iter()
            .map(|f| f.as_bigint())
            .collect::<Vec<_>>();
        possible_orders.sort_unstable();

        let actual = possible_orders.rchunks(10).next().unwrap();
        if DEBUG {
            if actual != expected_highest_ten {
                println!("Expected: {expected_highest_ten:?} (actual: {actual:?})");
            }
        } else {
            assert_eq!(actual, expected_highest_ten);
        }
    }

    fn test_possible_orders<const N: usize>(
        puzzle_def: &PuzzleDef<N>,
        expected_len: usize,
        expected_highest_ten: [u64; 10],
    ) {
        let expected_highest_ten = expected_highest_ten.map(Int::<U>::from);
        test_possible_orders_big(puzzle_def, expected_len, expected_highest_ten);
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
    #[ignore = "takes too long"]
    fn minx6() {
        test_possible_orders_big(
            &MINX6,
            1624462,
            [
                Int::<U>::from_str("114459483432082108320").unwrap(),
                Int::<U>::from_str("124809543104132086200").unwrap(),
                Int::<U>::from_str("136419733160330419800").unwrap(),
                Int::<U>::from_str("138938925342335718600").unwrap(),
                Int::<U>::from_str("143074354290102635400").unwrap(),
                Int::<U>::from_str("151863476536971599400").unwrap(),
                Int::<U>::from_str("158541852051194812200").unwrap(),
                Int::<U>::from_str("221360321731856907600").unwrap(),
                Int::<U>::from_str("249619086208264172400").unwrap(),
                Int::<U>::from_str("272839466320660839600").unwrap(),
            ],
        );
    }

    #[test_log::test]
    fn big1() {
        test_possible_orders(
            &BIG1,
            24820,
            [
                569729160, 595675080, 617795640, 629909280, 669278610, 698377680, 730122120,
                845404560, 944863920, 1396755360,
            ],
        );
    }

    #[test_log::test]
    fn big2() {
        test_possible_orders(
            &BIG2,
            43708,
            [
                5697291600,
                5956750800,
                6177956400,
                6299092800,
                6692786100,
                6983776800,
                7301221200,
                8454045600,
                9448639200,
                13967553600,
            ],
        );
    }

    #[test_log::test]
    fn big3() {
        test_possible_orders(
            &BIG3,
            49318,
            [
                8031343320,
                8172244080,
                8735847120,
                9133684560,
                10126476360,
                10708457760,
                10824854040,
                12258366120,
                16062686640,
                20252952720,
            ],
        );
    }
}

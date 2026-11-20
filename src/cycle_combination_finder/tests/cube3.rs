#![warn(clippy::pedantic)]

use cycle_combination_finder::{
    finder::{
        CycleCombinationFinderBuilder, NumCores, Optimality, OrderValueConstraint, PuzzleAnalysis,
        Register,
    },
    orderexps::OrderExps,
    puzzle::cubeN::CUBE3,
};
use puzzle_theory::numbers::{Int, U};

use crate::common::cycles;

mod common;

#[test_log::test]
fn optimal_2() {
    let cube3 = CUBE3.clone();
    let analysis = PuzzleAnalysis::new(&cube3, NumCores::AllCores).unwrap();
    let cycle_combinations = CycleCombinationFinderBuilder::default()
        .with_puzzle_analysis(&analysis)
        .with_registers(Register::default_count(2))
        .build()
        .unwrap()
        .find()
        .unwrap();
    assert_eq!(
        cycles(cycle_combinations),
        vec![
            vec![1260, 2],
            vec![840, 3],
            vec![720, 4],
            vec![630, 9],
            vec![420, 12],
            vec![360, 36],
            vec![180, 72],
            vec![90, 90],
        ],
    );
}

#[test_log::test]
fn optimal_3() {
    let cube3 = CUBE3.clone();
    let analysis = PuzzleAnalysis::new(&cube3, NumCores::AllCores).unwrap();
    let cycle_combinations = CycleCombinationFinderBuilder::default()
        .with_puzzle_analysis(&analysis)
        .with_registers(Register::default_count(3))
        .build()
        .unwrap()
        .find()
        .unwrap();
    assert_eq!(
        cycles(cycle_combinations),
        vec![
            vec![1260, 2, 1],
            vec![720, 2, 2],
            vec![630, 9, 3],
            vec![360, 9, 6],
            vec![210, 9, 9],
            vec![180, 18, 12],
            vec![90, 30, 18],
            vec![72, 36, 24],
            vec![36, 36, 30],
        ]
    );
}

#[test_log::test]
fn optimal_4() {
    let cube3 = CUBE3.clone();
    let analysis = PuzzleAnalysis::new(&cube3, NumCores::AllCores).unwrap();
    let cycle_combinations = CycleCombinationFinderBuilder::default()
        .with_puzzle_analysis(&analysis)
        .with_registers(Register::default_count(4))
        .build()
        .unwrap()
        .find()
        .unwrap();
    assert_eq!(
        cycles(cycle_combinations),
        vec![
            vec![1260, 2, 1, 1],
            vec![630, 3, 3, 3],
            vec![360, 4, 4, 4],
            vec![180, 12, 6, 6],
            vec![90, 12, 12, 12]
        ],
    );
}

#[test_log::test]
fn optimal_5() {
    let cube3 = CUBE3.clone();
    let analysis = PuzzleAnalysis::new(&cube3, NumCores::AllCores).unwrap();
    let cycle_combinations = CycleCombinationFinderBuilder::default()
        .with_puzzle_analysis(&analysis)
        .with_registers(Register::default_count(5))
        .build()
        .unwrap()
        .find()
        .unwrap();
    assert_eq!(
        cycles(cycle_combinations),
        vec![
            vec![1260, 2, 1, 1, 1],
            vec![630, 3, 3, 3, 3],
            vec![180, 4, 4, 4, 4],
            vec![126, 6, 6, 6, 6],
            vec![36, 12, 12, 12, 12]
        ],
    );
}

#[test_log::test]
fn equivalent_2() {
    let cube3 = CUBE3.clone();
    let analysis = PuzzleAnalysis::new(&cube3, NumCores::AllCores).unwrap();
    let cycle_combinations = CycleCombinationFinderBuilder::default()
        .with_puzzle_analysis(&analysis)
        .with_registers(Register::default_count(2))
        .with_optimality(Optimality::EQUIVALENT)
        .build()
        .unwrap()
        .find()
        .unwrap();
    assert_eq!(cycles(cycle_combinations), vec![vec![90, 90]]);
}

#[test_log::test]
fn equivalent_3() {
    let cube3 = CUBE3.clone();
    let analysis = PuzzleAnalysis::new(&cube3, NumCores::AllCores).unwrap();
    let cycle_combinations = CycleCombinationFinderBuilder::default()
        .with_puzzle_analysis(&analysis)
        .with_registers(Register::default_count(3))
        .with_optimality(Optimality::EQUIVALENT)
        .build()
        .unwrap()
        .find()
        .unwrap();
    assert_eq!(cycles(cycle_combinations), vec![vec![30, 30, 30]]);
}

#[test_log::test]
fn exactly_1_register() {
    let cube3 = CUBE3.clone();
    let analysis = PuzzleAnalysis::new(&cube3, NumCores::AllCores).unwrap();
    for order in [90, 360, 1260] {
        let cycle_combinations = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(vec![Register::default().with_order_value_constraint(
                OrderValueConstraint::Exactly(Int::<U>::from(order)),
            )])
            .build()
            .unwrap()
            .find()
            .unwrap();
        assert_eq!(cycles(cycle_combinations), vec![vec![order]]);
    }
}

/// Constraining either register of an unconstrained cycle combination `[x, y]`
/// to exactly `x` or `y` should only find `[x, y]` because it dominates
/// everything else with `x` or `y`
#[test_log::test]
fn exactly_front_entries() {
    let cube3 = CUBE3.clone();
    let analysis = PuzzleAnalysis::new(&cube3, NumCores::AllCores).unwrap();
    let front = cycles(
        CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(2))
            .build()
            .unwrap()
            .find()
            .unwrap(),
    );
    assert!(!front.is_empty());
    for entry in front {
        for &order in &entry {
            let cycle_combinations = CycleCombinationFinderBuilder::default()
                .with_puzzle_analysis(&analysis)
                .with_registers(vec![
                    Register::default().with_order_value_constraint(OrderValueConstraint::Exactly(
                        Int::<U>::from(order),
                    )),
                    Register::default(),
                ])
                .build()
                .unwrap()
                .find()
                .unwrap();
            assert_eq!(
                cycles(cycle_combinations),
                vec![entry.clone()],
                "Exactly({order})"
            );
        }
    }
}

#[test_log::test]
fn at_least_1_register() {
    let cube3 = CUBE3.clone();
    let analysis = PuzzleAnalysis::new(&cube3, NumCores::AllCores).unwrap();
    for (order, expected) in [
        (2_u64, vec![vec![1260]]),
        (1260, vec![vec![1260]]),
        (1261, vec![]),
    ] {
        let cycle_combinations = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(vec![Register::default().with_order_value_constraint(
                OrderValueConstraint::AtLeast(Int::<U>::from(order)),
            )])
            .build()
            .unwrap()
            .find()
            .unwrap();
        assert_eq!(cycles(cycle_combinations), expected, "AtLeast({order})");
    }
}

/// Compare "at least" and custom constraints against the Pareto front of every
/// 2 register cycle combination that satisfies them, found by pinning both
/// registers with `Exactly`
#[test_log::test]
#[allow(clippy::too_many_lines)]
fn predicates_match_brute_force() {
    let cube3 = CUBE3.clone();
    let analysis = PuzzleAnalysis::new(&cube3, NumCores::AllCores).unwrap();
    let find = |registers| {
        let mut cycle_combinations = cycles(
            CycleCombinationFinderBuilder::default()
                .with_puzzle_analysis(&analysis)
                .with_registers(registers)
                .build()
                .unwrap()
                .find()
                .unwrap(),
        );
        for cycle_combination in &mut cycle_combinations {
            cycle_combination.sort_unstable_by(|a, b| b.cmp(a));
        }
        cycle_combinations.sort_unstable();
        cycle_combinations
    };
    // `Exactly` fails to build if the order is not a possible order
    let orders = (2..=1260_u64)
        .filter(|&order| {
            CycleCombinationFinderBuilder::default()
                .with_puzzle_analysis(&analysis)
                .with_registers(vec![Register::default().with_order_value_constraint(
                    OrderValueConstraint::Exactly(Int::<U>::from(order)),
                )])
                .build()
                .is_ok()
        })
        .collect::<Vec<_>>();
    let mut all = vec![];
    for (index, &x) in orders.iter().enumerate() {
        for &y in &orders[..=index] {
            all.extend(find(vec![
                Register::default()
                    .with_order_value_constraint(OrderValueConstraint::Exactly(Int::<U>::from(x))),
                Register::default()
                    .with_order_value_constraint(OrderValueConstraint::Exactly(Int::<U>::from(y))),
            ]));
        }
    }

    // Not monotonic
    let odd: fn(&OrderExps<8>) -> bool =
        |order| order.as_bigint() % Int::<U>::from(2_u64) != Int::<U>::zero();
    let cases: [(Vec<Register<8>>, fn(u64, u64) -> bool); 6] = [
        (
            vec![
                Register::default().with_order_value_constraint(OrderValueConstraint::AtLeast(
                    Int::<U>::from(180_u64),
                )),
                Register::default().with_order_value_constraint(OrderValueConstraint::AtLeast(
                    Int::<U>::from(36_u64),
                )),
            ],
            |x, y| x >= 180 && y >= 36,
        ),
        (
            vec![
                Register::default().with_order_value_constraint(OrderValueConstraint::AtLeast(
                    Int::<U>::from(12_u64),
                )),
                Register::default().with_order_value_constraint(OrderValueConstraint::AtLeast(
                    Int::<U>::from(420_u64),
                )),
            ],
            |x, y| x >= 420 && y >= 12,
        ),
        (
            vec![
                Register::default().with_order_value_constraint(OrderValueConstraint::Exactly(
                    Int::<U>::from(36_u64),
                )),
                Register::default().with_order_value_constraint(OrderValueConstraint::AtLeast(
                    Int::<U>::from(180_u64),
                )),
            ],
            |x, y| (x == 36 && y >= 180) || (y == 36 && x >= 180),
        ),
        (
            vec![
                Register::default().with_order_value_constraint(OrderValueConstraint::Custom(odd)),
                Register::default(),
            ],
            |x, y| x % 2 == 1 || y % 2 == 1,
        ),
        (
            vec![
                Register::default().with_order_value_constraint(OrderValueConstraint::Custom(odd)),
                Register::default().with_order_value_constraint(OrderValueConstraint::Custom(odd)),
            ],
            |x, y| x % 2 == 1 && y % 2 == 1,
        ),
        (
            vec![
                Register::default().with_order_value_constraint(OrderValueConstraint::Custom(odd)),
                Register::default().with_order_value_constraint(OrderValueConstraint::AtLeast(
                    Int::<U>::from(360_u64),
                )),
            ],
            |x, y| (x % 2 == 1 && y >= 360) || (y % 2 == 1 && x >= 360),
        ),
    ];
    for (case, (registers, satisfies)) in cases.into_iter().enumerate() {
        let satisfying = all
            .iter()
            .filter(|cycle_combination| satisfies(cycle_combination[0], cycle_combination[1]))
            .collect::<Vec<_>>();
        let mut expected = satisfying
            .iter()
            .filter(|&&a| {
                !satisfying
                    .iter()
                    .any(|&b| b != a && b[0] >= a[0] && b[1] >= a[1])
            })
            .map(|&cycle_combination| cycle_combination.clone())
            .collect::<Vec<_>>();
        expected.sort_unstable();
        expected.dedup();
        assert!(!expected.is_empty(), "case {case}");
        assert_eq!(find(registers), expected);
    }
}

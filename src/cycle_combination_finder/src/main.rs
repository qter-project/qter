#![allow(unused)]
#![feature(portable_simd)]

use std::{
    fs::File,
    io::{BufWriter, Write},
    num::{NonZeroU16, NonZeroUsize},
    simd::{Simd, cmp::SimdPartialEq},
    time::Duration,
};

use cycle_combination_finder::{
    finder::{
        CycleCombinationFinderBuilder, NumCores, Optimality, OrderValueConstraint, PossibleOrder,
        PuzzleAnalysis, Register, SolutionExpansion,
    },
    puzzle::{
        PuzzleDef,
        cubeN::{self, cube},
        minxN,
    },
};
use puzzle_theory::{
    numbers::{Int, U},
    puzzle_geometry::Point,
};

fn main() {
    let Some(p) = std::env::args().nth(1) else {
        println!("Enter minx3 or minx4 or cube3");
        return;
    };
    env_logger::init();

    let ccf =
        CycleCombinationFinderBuilder::default().with_time_limit(Some(Duration::from_hours(10)));
    if p == "minx3" {
        let minx3 = minxN::MINX3.clone();
        let analysis =
            PuzzleAnalysis::new(&minx3, NumCores::AllCores).unwrap_or_else(|e| panic!("{e}"));
        let ret = ccf
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(4))
            // .with_registers(vec![
            //     Register::default().with_order_value_constraint(OrderValueConstraint::Exactly(
            //         Int::<U>::from(180u64),
            //     )),
            //     Register::default().with_order_value_constraint(OrderValueConstraint::Exactly(
            //         Int::<U>::from(15u64),
            //     )),
            //     Register::default(),
            //     Register::default(),
            // ])
            // .with_optimality(Optimality::MaxOrderRatio(20.0))
            // .with_expected_solutions_count_assertion(Some(347))
            .build()
            .unwrap_or_else(|e| panic!("{e}"))
            .find()
            .unwrap_or_else(|e| panic!("{e}"));
        for x in ret.cycle_combinations {
            println!("{}", x.orders_fmt(&ret.possible_orders_except_one));
        }
    } else if p == "minx4 3" {
        let minx4 = minxN::MINX4.clone();
        let analysis =
            PuzzleAnalysis::new(&minx4, NumCores::AllCores).unwrap_or_else(|e| panic!("{e}"));
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(3))
            .with_mss_batch_size(Some(1000))
            .with_expected_solutions_count_assertion(Some(296))
            .build()
            .unwrap_or_else(|e| panic!("{e}"))
            .find()
            .unwrap_or_else(|e| panic!("{e}"));
        let mut f = BufWriter::new(File::create(p).unwrap());
        for x in ret.cycle_combinations {
            writeln!(
                f,
                "{}",
                x.solutions_fmt(&ret.possible_orders_except_one, &minx4)
            );
            println!("{}", x.orders_fmt(&ret.possible_orders_except_one));
        }
    } else if p == "minx4 4" {
        let minx4 = minxN::MINX4.clone();
        let analysis =
            PuzzleAnalysis::new(&minx4, NumCores::AllCores).unwrap_or_else(|e| panic!("{e}"));
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(4))
            .with_optimality(Optimality::MaxOrderRatio(10.0))
            .with_mss_batch_size(Some(1))
            .build()
            .unwrap_or_else(|e| panic!("{e}"))
            .find()
            .unwrap_or_else(|e| panic!("{e}"));
        for x in ret.cycle_combinations {
            println!("{}", x.orders_fmt(&ret.possible_orders_except_one));
        }
    } else if p == "minx5" {
        let minx5 = minxN::MINX5.clone();
        let analysis =
            PuzzleAnalysis::new(&minx5, NumCores::AllCores).unwrap_or_else(|e| panic!("{e}"));
        let ret = CycleCombinationFinderBuilder::default()
            .with_puzzle_analysis(&analysis)
            .with_registers(Register::default_count(3))
            .with_max_fitting_tries(Some(2500))
            .with_mss_batch_size(Some(10000))
            .with_time_limit(Some(Duration::from_mins(10)))
            .with_solution_expansion(SolutionExpansion::MaxCount(100))
            .with_optimality(Optimality::MaxOrderRatio(1.3))
            .build()
            .unwrap_or_else(|e| panic!("{e}"))
            .find()
            .unwrap_or_else(|e| panic!("{e}"));
        let mut f = BufWriter::new(File::create("results.txt").unwrap());
        for x in ret.cycle_combinations {
            writeln!(
                f,
                "{}",
                x.solutions_fmt(&ret.possible_orders_except_one, &minx5)
            );
            println!("{}", x.orders_fmt(&ret.possible_orders_except_one));
        }
    } else if p == "cube5" {
        let cube3 = cubeN::CUBE5.clone();
        let analysis =
            PuzzleAnalysis::new(&cube3, NumCores::AllCores).unwrap_or_else(|e| panic!("{e}"));
        let ret = ccf
            .with_solution_expansion(SolutionExpansion::All)
            // .with_fast_assumptions(false)
            .with_puzzle_analysis(&analysis)
            .with_registers(vec![
                Register::default().with_order_value_constraint(OrderValueConstraint::Exactly(
                    Int::<U>::from(2u64),
                )),
                // Register::default().with_order_value_constraint(OrderValueConstraint::Custom(
                //     |order| order.0.simd_ne(Simd::splat(0)).to_bitmask() == 1,
                // )),
                Register::default(),
                Register::default(),
            ])
            .build()
            .unwrap_or_else(|e| panic!("{e}"))
            .find()
            .unwrap_or_else(|e| panic!("{e}"));
        // for x in ret.cycle_combinations {
        //     println!("{}", x.orders_fmt(&ret.possible_orders_except_one));
        // }
        let mut f = BufWriter::new(File::create("results.txt").unwrap());
        for x in ret.cycle_combinations {
            writeln!(
                f,
                "{}",
                x.solutions_fmt(&ret.possible_orders_except_one, &cube3)
            );
            println!("{}", x.orders_fmt(&ret.possible_orders_except_one));
        }
    }
}

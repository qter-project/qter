#![allow(unused)]

use std::{
    fs::File,
    io::{BufWriter, Write},
    num::{NonZeroU16, NonZeroUsize},
    time::Duration,
};

use cycle_combination_finder::{
    finder::{CycleCombinationFinder, Optimality, SolutionExpansion},
    puzzle::{
        PuzzleDef,
        cubeN::{self, cube},
        minxN,
    },
};

fn main() {
    let Some(p) = std::env::args().nth(1) else {
        println!("Enter minx3 or minx4 or cube3");
        return;
    };
    env_logger::init();

    let ccf = CycleCombinationFinder::builder().with_sorted(false);
    if p == "minx3" {
        let minx3 = minxN::MINX3.clone();
        ccf.with_puzzle_def(&minx3)
            .with_register_count(3)
            .with_expected_solutions_count_assertion(Some(347))
            .validate()
            .unwrap()
            .find()
            .unwrap();
    } else if p == "minx4 3" {
        let minx4 = minxN::MINX4.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx4)
            .with_register_count(3)
            .with_mss_batch_size(Some(1000))
            .with_expected_solutions_count_assertion(Some(296))
            .validate()
            .unwrap()
            .find()
            .unwrap();
        for x in ret.cycle_combinations {
            println!("{}", x.orders_fmt(&ret.possible_orders_except_one));
        }
    } else if p == "minx4 4" {
        let minx4 = minxN::MINX4.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx4)
            .with_register_count(4)
            .with_optimality(Optimality::MaxOrderRatio(10.0))
            .with_mss_batch_size(Some(1))
            .validate()
            .unwrap()
            .find()
            .unwrap();
        for x in ret.cycle_combinations {
            println!("{}", x.orders_fmt(&ret.possible_orders_except_one));
        }
    } else if p == "minx5" {
        let minx5 = minxN::MINX5.clone();
        let ret = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx5)
            .with_register_count(3)
            .with_max_fitting_tries(Some(2500))
            .with_mss_batch_size(Some(10000))
            .with_time_limit(Some(Duration::from_mins(10)))
            .with_solution_expansion(SolutionExpansion::Limit(100))
            .with_optimality(Optimality::MaxOrderRatio(1.3))
            .validate()
            .unwrap()
            .find()
            .unwrap();
        let mut f = BufWriter::new(File::create("results.txt").unwrap());
        for x in ret.cycle_combinations {
            writeln!(
                f,
                "{}",
                x.solutions_fmt(&ret.possible_orders_except_one, &minx5)
            );
            println!("{}", x.orders_fmt(&ret.possible_orders_except_one));
        }
        f.flush().unwrap();
    } else if p == "cube3" {
        let cube3 = cubeN::CUBE3.clone();
        ccf.with_puzzle_def(&cube3)
            .with_register_count(2)
            .with_expected_solutions_count_assertion(Some(5))
            .validate()
            .unwrap()
            .find()
            .unwrap();
    } else {
        println!("Enter minx3 or minx4 or cube3");
    }
}

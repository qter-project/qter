#![allow(unused)]

use std::num::{NonZeroU16, NonZeroUsize};

use cycle_combination_finder::{
    finder::CycleCombinationFinder,
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
    } else if p == "minx4" {
        let minx4 = minxN::MINX4.clone();
        let base = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx4)
            .with_register_count(3);
        for b in [1, 10, 100, 1000, 10000, 100000, 1000000, 10000000] {
            println!("{:?}", b);
            base.clone()
                .with_mss_batch_size(b)
                .validate()
                .unwrap()
                .find()
                .unwrap();
        }
    } else if p == "minx5" {
        let minx5 = minxN::MINX5.clone();
        let base = CycleCombinationFinder::builder()
            .with_puzzle_def(&minx5)
            .with_register_count(2);
        for b in [1, 10, 100, 1000, 10000, 100000, 1000000, 10000000] {
            println!("{:?}", b);
            base.clone()
                .with_mss_batch_size(b)
                .validate()
                .unwrap()
                .find()
                .unwrap();
        }
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

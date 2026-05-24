use std::{num::NonZeroU16, time::Instant};

use cycle_combination_finder::finder::{
    CycleCombination, CycleCombinationFinder, CycleCombinationFinderConfig, Optimality,
    RegisterCount,
};
use humanize_duration::{Truncate, prelude::DurationExt};
use log::info;

fn cycles<const N: usize>(cycle_combinations: Vec<CycleCombination<N>>) -> Vec<Vec<u64>> {
    cycle_combinations
        .into_iter()
        .map(|cycle_combination| {
            cycle_combination
                .registers()
                .map(|register| register.as_bigint().try_into().unwrap())
                .collect::<Vec<u64>>()
        })
        .collect::<Vec<_>>()
}

fn main() {
    env_logger::init();

    let puzzle = cycle_combination_finder::puzzle::minxN::MINX3.clone();
    // let puzzle = cycle_combination_finder::puzzle::cubeN::CUBE3.clone();
    let now = Instant::now();
    let ccf = CycleCombinationFinder::from(puzzle);
    let cycle_combinations = ccf.find(CycleCombinationFinderConfig {
        optimality: Optimality::Optimal,
        register_count: RegisterCount::Exactly(NonZeroU16::new(4).unwrap()),
    });
    info!("CCF in {}", now.elapsed().human(Truncate::Micro));
    info!("Solutions length: {}", cycle_combinations.len());

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

#![warn(clippy::pedantic)]

use cycle_combination_finder::finder::CycleCombination;

pub fn cycles<const N: usize>(cycle_combinations: Vec<CycleCombination<N>>) -> Vec<Vec<u64>> {
    cycle_combinations
        .into_iter()
        .map(|cycle_combination| {
            cycle_combination
                .orders()
                .map(|order| order.as_bigint().try_into().unwrap())
                .collect::<Vec<u64>>()
        })
        .collect::<Vec<_>>()
}

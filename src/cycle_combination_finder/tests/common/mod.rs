#![warn(clippy::pedantic)]

use cycle_combination_finder::finder::CycleCombination2;

pub fn cycles(cycle_combinations: Vec<CycleCombination2>) -> Vec<Vec<u64>> {
    cycle_combinations
        .into_iter()
        .map(|cycle_combination| {
            cycle_combination
                .cycles()
                .iter()
                .map(|cycle| cycle.order().try_into().unwrap())
                .collect::<Vec<u64>>()
        })
        .collect::<Vec<_>>()
}

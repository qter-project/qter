#![warn(clippy::pedantic)]

use cycle_combination_finder::finder::CycleCombinations;

pub fn cycles<const N: usize>(cycle_combinations: CycleCombinations<N>) -> Vec<Vec<u64>> {
    let cycles = cycle_combinations
        .registers()
        .map(|cycle_combination| {
            cycle_combination
                .map(|register| register.as_bigint().try_into().unwrap())
                .collect::<Vec<u64>>()
        })
        .collect::<Vec<_>>();
    drop(cycle_combinations);
    cycles
}

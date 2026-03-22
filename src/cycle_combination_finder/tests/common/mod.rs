use cycle_combination_finder::finder::CycleCombination;

pub fn cycles(cycle_combinations: Vec<CycleCombination>) -> Vec<Vec<u32>> {
    cycle_combinations
        .into_iter()
        .map(|cycle_combination| {
            cycle_combination
                .cycles()
                .iter()
                .map(|cycle| cycle.order().try_into().unwrap())
                .collect::<Vec<u32>>()
        })
        .collect::<Vec<_>>()
}

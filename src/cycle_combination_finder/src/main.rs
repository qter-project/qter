use std::num::NonZeroU16;

use cycle_combination_finder::finder::{
    CycleCombinationFinder, CycleCombinationFinderConfig, Optimality, RegisterCount,
};

fn main() {
    env_logger::init();

    let puzzle = cycle_combination_finder::puzzle::minxN::MINX3.clone();
    // let puzzle = cycle_combination_finder::puzzle::cubeN::CUBE3.clone();
    let ccf = CycleCombinationFinder::from(puzzle);
    ccf.find(CycleCombinationFinderConfig {
        optimality: Optimality::Optimal,
        register_count: RegisterCount::Exactly(NonZeroU16::new(3).unwrap()),
    });
}

use std::num::NonZeroU16;

use cycle_combination_finder::{
    finder::{CycleCombinationFinder, RegisterCount},
    puzzle::minxN,
};

fn main() {
    let Some(p) = std::env::args().nth(1) else {
        println!("Enter minx3 or minx4");
        return;
    };
    env_logger::init();

    if p == "minx3" {
        let minx3 = minxN::MINX3.clone();
        CycleCombinationFinder::from(minx3)
            .with_register_count(RegisterCount::Exactly(NonZeroU16::new(4).unwrap()))
            .with_expected_length_assertion(347)
            .find();
    } else if p == "minx4" {
        let minx4 = minxN::MINX4.clone();
        CycleCombinationFinder::from(minx4)
            .with_register_count(RegisterCount::Exactly(NonZeroU16::new(3).unwrap()))
            .with_expected_length_assertion(251)
            .find();
    }
}

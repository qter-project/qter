use std::num::NonZeroU16;

use cycle_combination_finder::{
    finder::{CycleCombinationFinder, RegisterCount},
    puzzle::minxN::MINX3,
};

fn main() {
    let minx3 = MINX3.clone();
    CycleCombinationFinder::from(minx3)
        .with_register_count(RegisterCount::Exactly(NonZeroU16::new(4).unwrap()))
        .with_expected_length_assertion(347)
        .find();
}

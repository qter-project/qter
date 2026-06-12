use std::num::NonZeroU16;

use cycle_combination_finder::{
    finder::{CycleCombinationFinder, RegisterCount},
    puzzle::{cubeN, minxN},
};
use log::debug;

fn main() {
    env_logger::init();
    let custom = minxN::CUSTOM.clone();
    let a = CycleCombinationFinder::from(custom)
        .with_register_count(RegisterCount::Exactly(NonZeroU16::new(6).unwrap()))
        .with_expected_length_assertion(249);
    loop {
        a.find().unwrap();
        debug!("Success");
    }

    let Some(p) = std::env::args().nth(1) else {
        println!("Enter minx3 or minx4 or cube3");
        return;
    };

    if p == "minx3" {
        let minx3 = minxN::MINX3.clone();
        CycleCombinationFinder::from(minx3)
            .with_register_count(RegisterCount::Exactly(NonZeroU16::new(4).unwrap()))
            .with_expected_length_assertion(347)
            .find()
            .unwrap();
    } else if p == "minx4" {
        let minx4 = minxN::MINX4.clone();
        CycleCombinationFinder::from(minx4)
            .with_register_count(RegisterCount::Exactly(NonZeroU16::new(3).unwrap()))
            .with_expected_length_assertion(251)
            .find()
            .unwrap();
    } else if p == "cube3" {
        let cube3 = cubeN::CUBE3.clone();
        CycleCombinationFinder::from(cube3)
            .with_register_count(RegisterCount::Exactly(NonZeroU16::new(2).unwrap()))
            .with_expected_length_assertion(5)
            .find()
            .unwrap();
    } else {
        println!("Enter minx3 or minx4 or cube3");
    }
}

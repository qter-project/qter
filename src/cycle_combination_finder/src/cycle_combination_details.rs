#[allow(unused)]
use std::time::{Duration, Instant};

use crate::{cycle_combinations_tree::DisjointRegisters, finder::PossibleOrder};

#[derive(Debug)]
pub struct Cycle {
    // partitions: Vec<Vec<u16>>,
}

#[derive(Debug)]
pub struct CycleCombinationDetails {
    cycles: Vec<Cycle>,
}

impl CycleCombinationDetails {
    #[must_use]
    pub fn new<const N: usize>(
        registers: DisjointRegisters,
        possible_orders_except_one: &[PossibleOrder<N>],
    ) -> Option<Self> {
        let now = Instant::now();
        while now.elapsed() < Duration::from_millis(10) {}
        #[allow(clippy::missing_panics_doc)]
        if registers
            .iter()
            .map(|register| {
                u64::try_from(
                    possible_orders_except_one[register as usize]
                        .order
                        .as_bigint(),
                )
                .unwrap()
            })
            .sum::<u64>()
            .is_multiple_of(28)
        {
            Some(CycleCombinationDetails { cycles: vec![] })
        } else {
            None
        }
    }
}

impl CycleCombinationDetails {
    #[must_use]
    pub fn cycles(&self) -> &[Cycle] {
        &self.cycles
    }
}

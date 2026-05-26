use std::time::{Duration, Instant};

use crate::cycle_combinations_tree::DisjointRegisters;

#[derive(Debug)]
pub struct Cycle<const N: usize> {
    // partitions: Vec<Vec<u16>>,
}

#[derive(Debug)]
pub struct CycleCombinationDetails<const N: usize> {
    cycles: Vec<Cycle<N>>,
}

impl<const N: usize> CycleCombinationDetails<N> {
    #[must_use]
    pub fn new(registers: DisjointRegisters<N>) -> Option<Self> {
        let now = Instant::now();
        while now.elapsed() < Duration::from_millis(10) {}
        #[allow(clippy::missing_panics_doc)]
        if registers
            .iter()
            .map(|register| u64::try_from(register.order.as_bigint()).unwrap())
            .sum::<u64>()
            .is_multiple_of(28)
        {
            Some(CycleCombinationDetails { cycles: vec![] })
        } else {
            None
        }
    }
}

impl<const N: usize> CycleCombinationDetails<N> {
    #[must_use]
    pub fn cycles(&self) -> &[Cycle<N>] {
        &self.cycles
    }
}

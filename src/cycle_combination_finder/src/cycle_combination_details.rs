use crate::finder::PossibleOrder;

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
    pub fn new(registers: (&[(PossibleOrder<N>, usize)], &PossibleOrder<N>)) -> Option<Self> {
        #[allow(clippy::missing_panics_doc)]
        if registers
            .0
            .iter()
            .map(|(prefix_register, _)| prefix_register)
            .chain(std::iter::once(registers.1))
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

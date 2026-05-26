use crate::finder::PossibleOrder;

#[derive(Debug)]
pub struct Cycle<const N: usize> {
    // partitions: Vec<Vec<u16>>,
}

#[derive(Debug)]
pub struct CycleCombinationDetails<const N: usize> {
    cycles: Vec<Cycle<N>>,
}

impl<const N: usize> TryFrom<&[PossibleOrder<N>]> for CycleCombinationDetails<N> {
    type Error = ();

    fn try_from(registers: &[PossibleOrder<N>]) -> Result<Self, ()> {
        // let now = Instant::now();
        // while now.elapsed() < Duration::from_millis(10) {}
        if registers
            .iter()
            .map(|register| u64::try_from(register.order.as_bigint()).unwrap())
            .sum::<u64>()
            .is_multiple_of(28)
        {
            Ok(CycleCombinationDetails { cycles: vec![] })
        } else {
            Err(())
        }
    }
}

impl<const N: usize> CycleCombinationDetails<N> {
    #[must_use]
    pub fn cycles(&self) -> &[Cycle<N>] {
        &self.cycles
    }
}

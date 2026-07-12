use crate::{cycle_combinations_tree::DisjointRegisters, finder::PossibleOrder, puzzle::PuzzleDef};

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
        puzzle_def: &PuzzleDef<N>,
    ) -> Option<Self> {
        // let unorienting_primes_mask = OrderExps::unseen_mask(
        //     registers
        //         .iter_orders(possible_orders_except_one)
        //         .map(|o| &o.order)
        //         .inspect(|a| {
        //             println!("{:?}", a.0);
        //         }),
        // );
        let a = puzzle_def.orientations_exps();
        println!("{a:?}");
        todo!()
    }
}

impl CycleCombinationDetails {
    #[must_use]
    pub fn cycles(&self) -> &[Cycle] {
        &self.cycles
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        cycle_combination_details::CycleCombinationDetails,
        cycle_combinations_tree::DisjointRegisters, finder::PossibleOrder,
        min_piece_count::MinPieceCount, nonemptyvec::NonemptySlice, orderexps::OrderExps,
        puzzle::minxN::MINX3,
    };

    #[test_log::test]
    fn foo() {
        let minx3 = MINX3.clone();
        let possible_orders = minx3.possible_orders(None).unwrap();
        possible_orders.remove(&OrderExps::one());
        let mut min_piece_count_calculator = MinPieceCount::from(&minx3);
        let mut possible_orders_except_one = possible_orders
            .into_iter()
            .map(|possible_order| {
                let min_piece_count = min_piece_count_calculator.calculate(&possible_order).0;
                PossibleOrder {
                    order: possible_order,
                    min_piece_count,
                }
            })
            .collect::<Vec<_>>();
        possible_orders_except_one.sort_unstable_by(|a, b| a.order.cmp(&b.order));
        // 2520 630 420
        let details = CycleCombinationDetails::new(
            DisjointRegisters::from(NonemptySlice::try_from(&[504, 251, 196][..]).unwrap()),
            &possible_orders_except_one,
            &minx3,
        )
        .unwrap();

        // 2520 630 420
        //
        // 2 2 2 3 3 5 7 : 4e 3c
        //     2 3 3 5 7 : 3c
        //   2 2 3   5 7 : 2e
        //
        // 24 edges 5 5 7 7
        // 14 corners 7 5
        //
        // 2520:
        //
        // e: (4+, 5+); total 9/30
        // c: (3+, 7+); total 10/20
        //
        // 630:
        //
        // e: (5+, 7+); total 10/30
        // c: (3+); total 3/20
        //
        // 420:
        //
        // e: (2+, 7+); total 9/30
        // c: (5+); total 5/20
        //
        // parity share 2 corners
        //
        // 28/30
        // 18/20

        println!("{details:?}");
        panic!();
    }
}

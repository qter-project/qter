use std::{
    borrow::Cow,
    simd::{Simd, cmp::SimdPartialOrd},
};

use fxhash::FxHashMap;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};

use crate::{
    orderexps::OrderExps,
    possible_orders::{LcmOrders, OrdersDashSet, OrdersSet},
};

#[derive(Debug)]
pub struct MaxOrderTrie<const N: usize> {
    level: usize,
    children: FxHashMap<u8, MaxOrderTrie<N>>,
    subtree_max_order: OrderExps<N>,
}

impl<const N: usize> MaxOrderTrie<N> {
    #[must_use]
    pub fn new(level: usize) -> Self {
        Self {
            level,
            children: FxHashMap::default(),
            subtree_max_order: OrderExps::one(),
        }
    }

    pub fn insert(&mut self, order: OrderExps<N>) {
        self.subtree_max_order = self.subtree_max_order.lcm(&order);

        if self.level != N {
            self.children
                .entry(order.0[self.level])
                .or_insert_with(|| Self::new(self.level + 1))
                .insert(order);
        }
    }

    fn collect_distinct_orders(
        &self,
        order: &OrderExps<N>,
        acc: &mut [u8; N],
        out: &mut OrdersSet<N>,
    ) {
        if self.level == N {
            out.insert(OrderExps(Simd::from_array(*acc)));
        } else if self.subtree_max_order.0.simd_gt(order.0).to_bitmask() >> self.level == 0 {
            // If all remaining subtree exponents are <= x on remaining levels,
            // then every y in this subtree yields exactly x on remaining levels.
            let mut exps = order.0;
            exps[..self.level].copy_from_slice(&acc[..self.level]);
            out.insert(OrderExps(exps));
        } else {
            for (&exp, child) in &self.children {
                let old = std::mem::replace(&mut acc[self.level], order.0[self.level].max(exp));
                child.collect_distinct_orders(order, acc, out);
                acc[self.level] = old;
            }
        }
    }

    pub fn par_collect_distinct_orders(&self, walker: Cow<LcmOrders<N>>, out: &OrdersDashSet<N>) {
        match walker {
            Cow::Borrowed(LcmOrders::CombinedOrders(walker)) => walker
                .par_iter()
                .fold(OrdersSet::default, |mut local_acc, order| {
                    let mut acc = [0u8; N];
                    self.collect_distinct_orders(&order, &mut acc, &mut local_acc);
                    local_acc
                })
                .for_each(|local_acc| {
                    for order in local_acc {
                        out.insert(order);
                    }
                }),
            Cow::Owned(LcmOrders::CombinedOrders(walker)) => walker
                .into_par_iter()
                .fold(OrdersSet::default, |mut local_acc, order| {
                    let mut acc = [0u8; N];
                    self.collect_distinct_orders(&order, &mut acc, &mut local_acc);
                    local_acc
                })
                .for_each(|local_acc| {
                    for order in local_acc {
                        out.insert(order);
                    }
                }),
            Cow::Borrowed(LcmOrders::OrbitOrders(walker)) => walker
                .par_iter()
                .fold(OrdersSet::default, |mut local_acc, order| {
                    let mut acc = [0u8; N];
                    self.collect_distinct_orders(order, &mut acc, &mut local_acc);
                    local_acc
                })
                .for_each(|local_acc| {
                    for order in local_acc {
                        out.insert(order);
                    }
                }),
            Cow::Owned(LcmOrders::OrbitOrders(walker)) => walker
                .into_par_iter()
                .fold(OrdersSet::default, |mut local_acc, order| {
                    let mut acc = [0u8; N];
                    self.collect_distinct_orders(&order, &mut acc, &mut local_acc);
                    local_acc
                })
                .for_each(|local_acc| {
                    for order in local_acc {
                        out.insert(order);
                    }
                }),
        }
    }
}

#[cfg(test)]
mod benches {
    use std::{borrow::Cow, iter::repeat_with, num::NonZeroU16, time::Instant};

    use log::info;

    use crate::{
        orderexps::OrderExps,
        possible_orders::{LcmOrders, OrdersDashSet, OrdersSet},
        trie::MaxOrderTrie,
    };

    fn mk_data<const N: usize>(count: usize) -> Vec<OrderExps<N>> {
        repeat_with(|| fastrand::u16(..))
            .filter_map(|x| OrderExps::<N>::try_from(NonZeroU16::new(x.max(1)).unwrap()).ok())
            .take(count)
            .collect::<Vec<_>>()
    }

    fn do_bench<const N: usize>(producer: Vec<OrderExps<N>>, walker: Vec<OrderExps<N>>) {
        let now = Instant::now();
        let out = OrdersDashSet::default();
        let mut root = MaxOrderTrie::new(0);
        for order in producer {
            root.insert(order);
        }
        root.par_collect_distinct_orders(
            Cow::Owned(LcmOrders::OrbitOrders(OrdersSet::from_iter(walker))),
            &out,
        );
        info!("Finished in {:?}", now.elapsed());
    }

    const SMALL: usize = 10usize.pow(2);
    const MID: usize = 10usize.pow(4);
    const BIG: usize = 10usize.pow(6);

    // #[test_log::test]
    // fn big_producer_big_walker() {
    //     let producer = mk_data(BIG);
    //     let walker = mk_data(BIG);
    //     do_bench(producer, walker);
    // }

    // #[test_log::test]
    // fn big_producer_mid_walker() {
    //     let producer = mk_data(BIG);
    //     let walker = mk_data(MID);
    //     do_bench(producer, walker);
    // }

    // #[test_log::test]
    // fn big_producer_small_walker() {
    //     let producer = mk_data(BIG);
    //     let walker = mk_data(SMALL);
    //     do_bench(producer, walker);
    // }

    // #[test_log::test]
    // fn mid_producer_big_walker() {
    //     let producer = mk_data(MID);
    //     let walker = mk_data(BIG);
    //     do_bench(producer, walker);
    // }

    // #[test_log::test]
    // fn mid_producer_mid_walker() {
    //     let producer = mk_data(MID);
    //     let walker = mk_data(MID);
    //     do_bench(producer, walker);
    // }

    // #[test_log::test]
    // fn mid_producer_small_walker() {
    //     let producer = mk_data(MID);
    //     let walker = mk_data(SMALL);
    //     do_bench(producer, walker);
    // }

    // #[test_log::test]
    // fn small_producer_big_walker() {
    //     let producer = mk_data(SMALL);
    //     let walker = mk_data(BIG);
    //     do_bench(producer, walker);
    // }

    // #[test_log::test]
    // fn small_producer_mid_walker() {
    //     let producer = mk_data(SMALL);
    //     let walker = mk_data(MID);
    //     do_bench(producer, walker);
    // }

    // #[test_log::test]
    // fn small_producer_small_walker() {
    //     let producer = mk_data(SMALL);
    //     let walker = mk_data(SMALL);
    //     do_bench(producer, walker);
    // }
}

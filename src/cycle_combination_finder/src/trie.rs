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
    terminal: bool,
}

impl<const N: usize> MaxOrderTrie<N> {
    #[must_use]
    pub fn new(level: usize) -> Self {
        Self {
            level,
            children: FxHashMap::default(),
            subtree_max_order: OrderExps::one(),
            terminal: false,
        }
    }

    pub fn insert(&mut self, order: OrderExps<N>) {
        self.subtree_max_order = self.subtree_max_order.lcm(&order);

        if self.level == N || order.0.simd_gt(Simd::splat(0)).to_bitmask() >> self.level == 0 {
            // The remaining zero exponents need no child nodes.
            self.terminal = true;
        } else {
            self.children
                .entry(order.prime_exponent(self.level))
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
            if self.terminal {
                // This prefix is also an order with an all-zero suffix.
                let mut exps = order.0;
                exps[..self.level].copy_from_slice(&acc[..self.level]);
                out.insert(OrderExps(exps));
            }
            for (&exp, child) in &self.children {
                let old = std::mem::replace(
                    &mut acc[self.level],
                    order.prime_exponent(self.level).max(exp),
                );
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
mod tests {
    use super::*;

    fn node_count<const N: usize>(trie: &MaxOrderTrie<N>) -> usize {
        1 + trie.children.values().map(node_count).sum::<usize>()
    }

    #[test]
    fn trailing_zero_exponents_do_not_allocate_nodes() {
        let mut trie = MaxOrderTrie::<32>::new(0);
        trie.insert(OrderExps::one());
        assert!(trie.terminal);
        assert_eq!(node_count(&trie), 1);
        let mut exps = [0; 32];
        exps[0] = 2;
        exps[2] = 1;
        for _ in 0..2 {
            trie.insert(OrderExps(Simd::from_array(exps)));
            assert_eq!(node_count(&trie), 4);
        }
        let leaf = &trie.children[&2].children[&0].children[&1];
        assert!(leaf.terminal);
        assert!(leaf.children.is_empty());
    }

    #[test]
    fn terminal_prefixes_match_pairwise_lcms() {
        // All subsets cover internal zeros, terminal prefixes, and branches
        // that prevent the subtree maximum shortcut, in both insertion orders.
        let orders: Vec<_> = (0..8)
            .map(|bits| {
                let mut exps = [0; 8];
                for (i, exp) in exps.iter_mut().take(3).enumerate() {
                    *exp = (bits >> i) & 1;
                }
                OrderExps(Simd::from_array(exps))
            })
            .collect();
        for subset in 1..256 {
            for reverse in [false, true] {
                let mut trie = MaxOrderTrie::new(0);
                for i in 0..orders.len() {
                    let i = if reverse { orders.len() - 1 - i } else { i };
                    if subset & (1 << i) != 0 {
                        trie.insert(orders[i].clone());
                    }
                }
                for walker in &orders {
                    let expected: OrdersSet<8> = orders
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| subset & (1 << i) != 0)
                        .map(|(_, order)| order.lcm(walker))
                        .collect();
                    let mut actual = OrdersSet::default();
                    trie.collect_distinct_orders(walker, &mut [0; 8], &mut actual);
                    assert_eq!(actual, expected);
                }
            }
        }
    }

    #[test]
    fn final_lane_and_terminal_prefix_preserve_lcms() {
        let mut trie = MaxOrderTrie::<64>::new(0);
        let mut prefix = OrderExps::one();
        prefix.0[0] = 2;
        let mut full = prefix.clone();
        full.0[63] = 1;
        // Mark an existing internal node after inserting the longer order.
        trie.insert(full.clone());
        trie.insert(prefix.clone());
        let mut walker = OrderExps::one();
        walker.0[1] = 1;
        let mut actual = OrdersSet::default();
        trie.collect_distinct_orders(&walker, &mut [0; 64], &mut actual);
        assert_eq!(
            actual,
            OrdersSet::from_iter([prefix.lcm(&walker), full.lcm(&walker)])
        );
    }

    #[test]
    fn parallel_walkers_preserve_lcms() {
        let producer = [
            OrderExps(Simd::from_array([2, 0, 0, 0, 0, 0, 0, 0])),
            OrderExps(Simd::from_array([2, 1, 0, 0, 0, 0, 0, 0])),
            OrderExps::one(),
        ];
        let walkers = [OrderExps::one(), OrderExps(Simd::splat(1))];
        let expected: OrdersSet<8> = producer
            .iter()
            .flat_map(|order| walkers.iter().map(move |walker| order.lcm(walker)))
            .collect();
        let mut trie = MaxOrderTrie::new(0);
        for order in producer {
            trie.insert(order);
        }
        for walker in [
            LcmOrders::OrbitOrders(OrdersSet::from_iter(walkers.clone())),
            LcmOrders::CombinedOrders(OrdersDashSet::from_iter(walkers.clone())),
        ] {
            for owned in [false, true] {
                let out = OrdersDashSet::default();
                trie.par_collect_distinct_orders(
                    if owned {
                        Cow::Owned(walker.clone())
                    } else {
                        Cow::Borrowed(&walker)
                    },
                    &out,
                );
                assert_eq!(out.into_iter().collect::<OrdersSet<8>>(), expected);
            }
        }
    }
}

use std::simd::{LaneCount, Simd, SupportedLaneCount, cmp::SimdPartialOrd};

use fxhash::{FxHashMap, FxHashSet};

use crate::orderexps::OrderExps;

#[derive(Debug)]
pub struct MaxOrderTrie<const N: usize>
where
    LaneCount<N>: SupportedLaneCount,
{
    level: usize,
    children: FxHashMap<u8, MaxOrderTrie<N>>,
    subtree_max_order: OrderExps<N>,
}

impl<const N: usize> MaxOrderTrie<N>
where
    LaneCount<N>: SupportedLaneCount,
{
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

    pub fn collect_distinct_orders(
        &self,
        order: &OrderExps<N>,
        acc: &mut [u8; N],
        out: &mut FxHashSet<OrderExps<N>>,
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
}

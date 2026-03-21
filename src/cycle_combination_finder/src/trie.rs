use crate::OrderFactors;
use fxhash::{FxHashMap, FxHashSet};
use std::simd::{LaneCount, Simd, SupportedLaneCount, cmp::SimdPartialOrd};

#[derive(Debug)]
pub struct MaxOrderTrie<const N: usize>
where
    LaneCount<N>: SupportedLaneCount,
{
    level: usize,
    children: FxHashMap<u8, MaxOrderTrie<N>>,
    subtree_max_order: OrderFactors<N>,
}

impl<const N: usize> MaxOrderTrie<N>
where
    LaneCount<N>: SupportedLaneCount,
{
    pub fn new(level: usize) -> Self {
        Self {
            level,
            children: FxHashMap::default(),
            subtree_max_order: OrderFactors::one(),
        }
    }

    pub fn insert(&mut self, v: OrderFactors<N>) {
        self.subtree_max_order = self.subtree_max_order.lcm(&v);

        if self.level != N {
            self.children
                .entry(v.exps[self.level])
                .or_insert_with(|| Self::new(self.level + 1))
                .insert(v);
        }
    }

    pub fn collect_distinct_orders(
        &self,
        order: &OrderFactors<N>,
        acc: &mut [u8; N],
        out: &mut FxHashSet<OrderFactors<N>>,
    ) {
        if self.level == N {
            out.insert(OrderFactors {
                exps: Simd::from_array(*acc),
            });
        } else if self.subtree_max_order.exps.simd_gt(order.exps).to_bitmask() >> self.level == 0 {
            // If all remaining subtree exponents are <= x on remaining levels,
            // then every y in this subtree yields exactly x on remaining levels.
            let mut exps = order.exps;
            exps[..self.level].copy_from_slice(&acc[..self.level]);
            out.insert(OrderFactors { exps });
        } else {
            for (&exp, child) in &self.children {
                let old = std::mem::replace(&mut acc[self.level], order.exps[self.level].max(exp));
                child.collect_distinct_orders(order, acc, out);
                acc[self.level] = old;
            }
        }
    }
}

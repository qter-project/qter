use crate::OrderFactors;
use fxhash::{FxHashMap, FxHashSet};
use std::simd::{LaneCount, Simd, SupportedLaneCount, cmp::SimdPartialOrd};

#[derive(Debug)]
pub struct MaxTrieNode<const N: usize>
where
    LaneCount<N>: SupportedLaneCount,
{
    level: usize,
    // Children keyed by exponent value at this level.
    children: FxHashMap<u8, MaxTrieNode<N>>,
    // For pruning: coordinatewise maxima over all vectors in this subtree.
    subtree_max: OrderFactors<N>,
}

impl<const N: usize> MaxTrieNode<N>
where
    LaneCount<N>: SupportedLaneCount,
{
    pub fn new(level: usize) -> Self {
        Self {
            level,
            children: FxHashMap::default(),
            subtree_max: OrderFactors::one(),
        }
    }

    pub fn insert(&mut self, v: OrderFactors<N>) {
        self.subtree_max = self.subtree_max.lcm(&v);

        if self.level != N {
            self.children
                .entry(v.exps[self.level])
                .or_insert_with(|| Self::new(self.level + 1))
                .insert(v);
        }
    }

    pub fn collect_distinct_lcms(
        &self,
        order: &OrderFactors<N>,
        lcm: &mut [u8; N],
        out: &mut FxHashSet<OrderFactors<N>>,
    ) {
        if self.level == N {
            out.insert(OrderFactors {
                exps: Simd::from_array(*lcm),
            });
        } else if self.subtree_max.exps.simd_gt(order.exps).to_bitmask() >> self.level == 0 {
            // If all remaining subtree exponents are <= x on remaining levels,
            // then every y in this subtree yields exactly x on remaining levels.
            let mut exps = order.exps;
            exps[..self.level].copy_from_slice(&lcm[..self.level]);
            out.insert(OrderFactors { exps });
        } else {
            for (&exp, child) in &self.children {
                let old = std::mem::replace(&mut lcm[self.level], order.exps[self.level].max(exp));
                child.collect_distinct_lcms(order, lcm, out);
                lcm[self.level] = old;
            }
        }
    }
}

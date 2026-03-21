use crate::N;
use puzzle_theory::numbers::{Int, U};
use std::{
    fmt::{Debug, Formatter},
    simd::{LaneCount, Simd, SupportedLaneCount, cmp::SimdOrd},
};

pub const PRIMES: [u8; N] = [
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131,
];

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct OrderExps<const N: usize>(pub Simd<u8, N>)
where
    LaneCount<N>: SupportedLaneCount;

impl<const N: usize> OrderExps<N>
where
    LaneCount<N>: SupportedLaneCount,
{
    pub fn one() -> Self {
        Self(Simd::splat(0))
    }
    
    pub fn as_bigint(&self) -> Int<U> {
        let mut result = Int::one();
        for (i, p) in PRIMES.into_iter().enumerate() {
            for _ in 0..self.0[i] {
                result *= Int::<U>::from(p);
            }
        }
        result
    }

    pub fn lcm(&self, other: &Self) -> Self {
        Self(self.0.simd_max(other.0))
    }
}

impl<const N: usize> Debug for OrderExps<N>
where
    LaneCount<N>: SupportedLaneCount,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "OF({})", self.as_bigint())
    }
}

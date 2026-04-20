use std::{
    fmt::{Debug, Formatter},
    ops::Mul,
    simd::{
        Simd,
        cmp::{SimdOrd, SimdPartialEq},
    },
};

use puzzle_theory::numbers::{Int, U};

use crate::FIRST_129_PRIMES;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct OrderExps<const N: usize>(pub Simd<u8, N>);

impl<const N: usize> OrderExps<N> {
    #[inline]
    #[must_use]
    pub fn one() -> Self {
        Self(Simd::splat(0))
    }

    #[inline]
    #[must_use]
    pub fn as_bigint(&self) -> Int<U> {
        let mut result = Int::one();
        for (i, p) in FIRST_129_PRIMES.into_iter().enumerate().take(N) {
            for _ in 0..self.0[i] {
                result *= Int::<U>::from(p);
            }
        }
        result
    }

    #[inline]
    #[must_use]
    pub fn lcm(&self, other: &Self) -> Self {
        Self(self.0.simd_max(other.0))
    }

    #[inline]
    #[must_use]
    pub fn is_prime_power(&self) -> bool {
        self.0
            .simd_ne(Simd::splat(0))
            .to_bitmask()
            .is_power_of_two()
    }
}

impl<const N: usize> Mul for OrderExps<N> {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        // We should generally not overflow because 2^256 is way too big.
        #[allow(clippy::suspicious_arithmetic_impl)]
        Self(self.0 + rhs.0)
    }
}

impl<const N: usize> Debug for OrderExps<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "OF({})", self.as_bigint())
    }
}

// const FIRST_32_PRIMES_LN: f32x32 = f32x32::from_array([
//     LN_2, 1.0986123, 1.609438, 1.9459101, 2.3978953, 2.5649493, 2.8332133,
// 2.944439, 3.1354942,     3.3672957, 3.4339871, 3.6109178, 3.713572,
// 3.7612002, 3.8501475, 3.9702919, 4.0775375,     4.1108737, 4.204693, 4.26268,
// 4.2904596, 4.3694477, 4.4188404, 4.4886365, 4.574711, 4.6151204,
//     4.634729, 4.6728287, 4.691348, 4.727388, 4.8441873, 4.8751974,
// ]);
// const MAX_EXPONENT: u8x32 = u8x32::from_array(const {
//     let mut ret = [0; 32];
//     let mut i = 0;
//     while i < 32 {
//         ret[i] = u8::MAX / FIRST_32_PRIMES.to_array()[i];
//         i += 1;
//     }
//     ret
// });

// impl PartialOrd for PrimePowerNum {
//     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
//         let self_ln = self.0.cast::<f32>() * FIRST_32_PRIMES_LN;
//         let other_ln = other.0.cast::<f32>() * FIRST_32_PRIMES_LN;
//         self_ln.reduce_sum().partial_cmp(&other_ln.reduce_sum())
//     }
// }

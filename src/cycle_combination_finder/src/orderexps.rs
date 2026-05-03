use std::{
    cmp::Ordering,
    fmt::{self, Debug},
    num::NonZeroU16,
    ops::Mul,
    simd::{
        Simd,
        cmp::{SimdOrd, SimdPartialEq},
    },
};

use puzzle_theory::numbers::{Int, U};
use thiserror::Error;

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
        for (p, &e) in FIRST_129_PRIMES
            .into_iter()
            .zip(self.0.as_array().iter())
            .take(N)
        {
            for _ in 0..e {
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

#[derive(Error, Debug)]
pub enum OrderExpsConversionError {
    #[error("We cannot represent numbers too large")]
    PrimeTooLarge,
}

impl<const N: usize> TryFrom<NonZeroU16> for OrderExps<N> {
    type Error = OrderExpsConversionError;

    fn try_from(n: NonZeroU16) -> Result<Self, Self::Error> {
        let mut exps = Self::one();
        let mut primes_and_exps = FIRST_129_PRIMES.into_iter().zip(exps.0.as_mut_array());
        let (mut prime, mut exp) = primes_and_exps.next().unwrap();
        let mut remainder = n.get();
        while remainder > 1 {
            if remainder.is_multiple_of(prime) {
                *exp += 1;
                remainder /= prime;
            } else if remainder > 1 {
                (prime, exp) = primes_and_exps
                    .next()
                    .ok_or(OrderExpsConversionError::PrimeTooLarge)?;
            }
        }
        Ok(exps)
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_bigint())
    }
}

impl<const N: usize> PartialOrd for OrderExps<N> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<const N: usize> Ord for OrderExps<N> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        let max = self.0.simd_max(other.0);
        match (max == self.0, max == other.0) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => {
                let a: f64 = FIRST_129_PRIMES
                    .into_iter()
                    .zip(self.0.as_array().iter())
                    .take(N)
                    .map(|(p, &e)| f64::from(e) * f64::from(p).ln())
                    .sum();
                let b: f64 = FIRST_129_PRIMES
                    .into_iter()
                    .zip(other.0.as_array().iter())
                    .take(N)
                    .map(|(p, &e)| f64::from(e) * f64::from(p).ln())
                    .sum();
                a.partial_cmp(&b).unwrap()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU16;

    use crate::{FIRST_129_PRIMES, orderexps::OrderExps};

    #[test_log::test]
    fn try_from_basic() {
        for i in 1..FIRST_129_PRIMES[64] {
            assert_eq!(
                u16::try_from(
                    OrderExps::<64>::try_from(NonZeroU16::new(i).unwrap())
                        .unwrap()
                        .as_bigint()
                )
                .unwrap(),
                i
            );
        }
    }

    #[test_log::test]
    #[should_panic(expected = "PrimeTooLarge")]
    fn try_from_prime_too_large() {
        OrderExps::<64>::try_from(NonZeroU16::new(FIRST_129_PRIMES[65]).unwrap()).unwrap();
    }

    #[test_log::test]
    fn ord() {
        for i in 1..FIRST_129_PRIMES[64] {
            for j in 1..FIRST_129_PRIMES[64] {
                let a = OrderExps::<64>::try_from(NonZeroU16::new(i).unwrap()).unwrap();
                let b = OrderExps::<64>::try_from(NonZeroU16::new(j).unwrap()).unwrap();
                assert_eq!(a.cmp(&b), i.cmp(&j));
            }
        }
    }
}

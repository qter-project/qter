use std::num::{NonZeroU8, NonZeroU16};

use crate::{FIRST_129_PRIMES, orderexps::OrderExps};

/// Compute all divisors of a number, with every divisor represented as a
/// [`OrderExps`].
///
/// # Panics
///
/// This function panics if a divisor cannot be represented by the first `N`
/// primes.
#[must_use]
pub fn divisors<const N: usize>(n: NonZeroU8) -> Vec<OrderExps<N>> {
    #[allow(clippy::missing_panics_doc)]
    {
        assert!(NonZeroU16::from(n).get() < FIRST_129_PRIMES[N]);
    }
    let mut divisor = OrderExps::one();
    let mut divisors = vec![divisor.clone()];

    let mut primes_and_index = FIRST_129_PRIMES
        .into_iter()
        .map_while(|p| u8::try_from(p).ok())
        .enumerate();
    let mut remainder = n.get();
    let mut max_exp: u8 = 0;

    let (mut prime_index, mut prime) = primes_and_index.next().unwrap();
    while remainder > 1 || max_exp > 0 {
        if remainder.is_multiple_of(prime) {
            // We don't care about overflow since it happens at prime^256
            max_exp += 1;
            remainder /= prime;
        } else {
            divisors.reserve(divisors.len() * usize::from(max_exp));

            let org_len = divisors.len();
            divisor.0[prime_index] = 1;
            while divisor.0[prime_index] <= max_exp {
                for i in 0..org_len {
                    divisors.push(divisors[i].clone() * divisor.clone());
                }
                divisor.0[prime_index] += 1;
            }
            divisor.0[prime_index] = 0;
            max_exp = 0;
            if remainder > 1 {
                (prime_index, prime) = primes_and_index
                    .next()
                    .expect("We cannot represent numbers too large");
            }
        }
    }

    divisors
}

#[cfg(test)]
mod divisors {
    use std::num::NonZeroU8;

    use crate::{number_theory, orderexps::OrderExps};

    fn as_u64s<const N: usize>(n: Vec<OrderExps<N>>) -> Vec<u8> {
        n.into_iter()
            .map(|x| x.as_bigint().try_into().unwrap())
            .collect()
    }

    fn divisors<const N: usize>(n: u8) -> Vec<OrderExps<N>> {
        assert_ne!(n, 0);
        number_theory::divisors(NonZeroU8::new(n).unwrap())
    }

    #[test_log::test]
    fn edge_cases() {
        assert_eq!(as_u64s(divisors::<8>(1)), vec![1]);
        assert_eq!(as_u64s(divisors::<8>(2)), vec![1, 2]);
    }

    #[test_log::test]
    fn composites() {
        assert_eq!(
            as_u64s(divisors::<64>(255)),
            vec![1, 3, 5, 15, 17, 51, 85, 255]
        );
    }

    #[test_log::test]
    fn primes() {
        assert_eq!(as_u64s(divisors::<8>(3)), vec![1, 3]);
        assert_eq!(as_u64s(divisors::<8>(17)), vec![1, 17]);
        assert_eq!(as_u64s(divisors::<32>(131)), vec![1, 131]);
    }

    #[test_log::test]
    fn prime_powers() {
        assert_eq!(as_u64s(divisors::<8>(4)), vec![1, 2, 4]);
        assert_eq!(as_u64s(divisors::<8>(9)), vec![1, 3, 9]);
        assert_eq!(as_u64s(divisors::<32>(125)), vec![1, 5, 25, 125]);
        assert_eq!(as_u64s(divisors::<64>(243)), vec![1, 3, 9, 27, 81, 243]);
    }

    #[test_log::test]
    fn between_max_prime() {
        assert_eq!(
            as_u64s(divisors::<32>(132)),
            vec![1, 2, 4, 3, 6, 12, 11, 22, 44, 33, 66, 132]
        );
        assert_eq!(as_u64s(divisors::<32>(133)), vec![1, 7, 19, 133]);
        assert_eq!(as_u64s(divisors::<32>(134)), vec![1, 2, 67, 134]);
        assert_eq!(
            as_u64s(divisors::<32>(135)),
            vec![1, 3, 9, 27, 5, 15, 45, 135]
        );
        assert_eq!(
            as_u64s(divisors::<32>(136)),
            vec![1, 2, 4, 8, 17, 34, 68, 136]
        );
    }

    #[test_log::test]
    #[should_panic(expected = "assertion failed: NonZeroU16::from(n).get() < FIRST_")]
    fn too_big_prime_panics1() {
        let _ = divisors::<32>(251);
    }

    #[test_log::test]
    #[should_panic(expected = "assertion failed: NonZeroU16::from(n).get() < FIRST_")]
    fn too_big_prime_panics2() {
        let _ = divisors::<32>(137);
    }
}

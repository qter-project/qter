use std::num::NonZeroU16;

use thiserror::Error;

use crate::{FIRST_129_PRIMES, orderexps::OrderExps, puzzle::OrbitDef};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MaxPrimePower {
    pub prime: u16,
    pub exponent: u16,
    pub orienting_exponent: u16,
}

/// return a 2D list of prime powers below n. The first index is the prime,
/// the second is the power of that prime Return all
#[must_use]
pub fn max_prime_powers_below(orbit_defs: &[OrbitDef], n: u16) -> Vec<MaxPrimePower> {
    #[derive(Copy, Clone, Debug, PartialEq)]
    enum SieveNumberState {
        Prime,
        Other,
    }

    let n = usize::from(n);

    let mut sieve = vec![SieveNumberState::Prime; n + 1];
    sieve[0] = SieveNumberState::Other;
    if let Some(v) = sieve.get_mut(1) {
        *v = SieveNumberState::Other;
    }

    for i in 2..=n.isqrt() {
        if sieve[i] != SieveNumberState::Prime {
            continue;
        }
        let prime = i;

        for multiple in (prime * prime..=n).step_by(prime) {
            sieve[multiple] = SieveNumberState::Other;
        }
    }

    let mut max_prime_powers = vec![];
    for (i, &state) in sieve.iter().enumerate().take(n + 1).skip(2) {
        if state != SieveNumberState::Prime {
            continue;
        }
        let prime = i;

        let mut exponent = 1;
        let mut min_piece_count = prime;
        let maybe_orienting_orbit = orbit_defs
            .iter()
            .filter(|&&orbit_def| orbit_def.orientation_count() as usize == prime)
            .max_by_key(|&&orbit_def| orbit_def.piece_count)
            .copied();
        let mut orienting_exponent = 2;
        loop {
            let next = min_piece_count * prime;
            let mut changed = false;
            if let Some(orienting_orbit) = maybe_orienting_orbit
                && next <= usize::from(orienting_orbit.piece_count.get())
            {
                orienting_exponent += 1;
                changed = true;
            }
            if next <= n {
                exponent += 1;
                changed = true;
            }
            if changed {
                min_piece_count = next;
            } else {
                break;
            }
        }
        if maybe_orienting_orbit.is_none() {
            orienting_exponent = exponent;
        }

        max_prime_powers.push(MaxPrimePower {
            #[allow(clippy::missing_panics_doc)]
            prime: u16::try_from(prime).unwrap(),
            exponent,
            orienting_exponent,
        });
    }
    max_prime_powers.sort_by_key(|a| a.prime);
    max_prime_powers
}

/// Compute all divisors of a number, with every divisor represented as a
/// [`OrdersExps`].
///
/// # Panics
///
/// This function panics if a divisor cannot be represented by the first `N`
/// primes.
#[must_use]
pub fn divisors<const N: usize>(n: u8) -> Vec<OrderExps<N>> {
    #[allow(clippy::missing_panics_doc)]
    {
        assert!(u16::from(n) < FIRST_129_PRIMES[N]);
    }
    let mut divisors = vec![];
    if n == 0 {
        return divisors;
    }
    let mut divisor = OrderExps::one();
    divisors.push(divisor.clone());

    let mut primes_and_index = FIRST_129_PRIMES
        .into_iter()
        .map_while(|p| u8::try_from(p).ok())
        .enumerate();
    let mut remainder = n;
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

#[cfg(test)]
mod max_prime_powers_below {
    use crate::{
        number_theory::{MaxPrimePower, max_prime_powers_below},
        puzzle::{cubeN::CUBE3, minxN::MINX3, misc::BIG1},
    };

    #[test_log::test]
    fn edge_cases() {
        let cube3 = CUBE3.orbit_defs();
        assert!(max_prime_powers_below(cube3, 0).is_empty());
        assert!(max_prime_powers_below(cube3, 1).is_empty());
        // ensure it generates itself if prime
        assert!(max_prime_powers_below(cube3, 13).contains(&MaxPrimePower {
            prime: 13,
            exponent: 1,
            orienting_exponent: 1,
        }));
        // ensure it generates itself if prime power
        assert!(max_prime_powers_below(cube3, 25).contains(&MaxPrimePower {
            prime: 5,
            exponent: 2,
            orienting_exponent: 2,
        }));
    }

    #[test_log::test]
    fn cube3() {
        let cube3 = CUBE3.orbit_defs();
        let max_prime_powers = max_prime_powers_below(cube3, 12);
        assert_eq!(
            max_prime_powers,
            vec![
                MaxPrimePower {
                    prime: 2,
                    exponent: 3,
                    orienting_exponent: 4,
                },
                MaxPrimePower {
                    prime: 3,
                    exponent: 2,
                    orienting_exponent: 2,
                },
                MaxPrimePower {
                    prime: 5,
                    exponent: 1,
                    orienting_exponent: 1,
                },
                MaxPrimePower {
                    prime: 7,
                    exponent: 1,
                    orienting_exponent: 1,
                },
                MaxPrimePower {
                    prime: 11,
                    exponent: 1,
                    orienting_exponent: 1,
                },
            ]
        );
    }

    #[test_log::test]
    fn minx3() {
        let minx3 = MINX3.orbit_defs();
        let max_prime_powers = max_prime_powers_below(minx3, 30);
        assert_eq!(
            max_prime_powers,
            vec![
                MaxPrimePower {
                    prime: 2,
                    exponent: 4,
                    orienting_exponent: 5
                },
                MaxPrimePower {
                    prime: 3,
                    exponent: 3,
                    orienting_exponent: 3
                },
                MaxPrimePower {
                    prime: 5,
                    exponent: 2,
                    orienting_exponent: 2
                },
                MaxPrimePower {
                    prime: 7,
                    exponent: 1,
                    orienting_exponent: 1
                },
                MaxPrimePower {
                    prime: 11,
                    exponent: 1,
                    orienting_exponent: 1
                },
                MaxPrimePower {
                    prime: 13,
                    exponent: 1,
                    orienting_exponent: 1
                },
                MaxPrimePower {
                    prime: 17,
                    exponent: 1,
                    orienting_exponent: 1
                },
                MaxPrimePower {
                    prime: 19,
                    exponent: 1,
                    orienting_exponent: 1
                },
                MaxPrimePower {
                    prime: 23,
                    exponent: 1,
                    orienting_exponent: 1
                },
                MaxPrimePower {
                    prime: 29,
                    exponent: 1,
                    orienting_exponent: 1
                }
            ]
        );
    }

    #[test_log::test]
    fn big() {
        let big = BIG1.orbit_defs();
        let max_prime_powers = max_prime_powers_below(big, 60);
        assert_eq!(
            max_prime_powers,
            vec![
                MaxPrimePower {
                    prime: 2,
                    exponent: 5,
                    orienting_exponent: 6,
                },
                MaxPrimePower {
                    prime: 3,
                    exponent: 3,
                    orienting_exponent: 4,
                },
                MaxPrimePower {
                    prime: 5,
                    exponent: 2,
                    orienting_exponent: 2,
                },
                MaxPrimePower {
                    prime: 7,
                    exponent: 2,
                    orienting_exponent: 2,
                },
                MaxPrimePower {
                    prime: 11,
                    exponent: 1,
                    orienting_exponent: 1,
                },
                MaxPrimePower {
                    prime: 13,
                    exponent: 1,
                    orienting_exponent: 1,
                },
                MaxPrimePower {
                    prime: 17,
                    exponent: 1,
                    orienting_exponent: 1,
                },
                MaxPrimePower {
                    prime: 19,
                    exponent: 1,
                    orienting_exponent: 1,
                },
                MaxPrimePower {
                    prime: 23,
                    exponent: 1,
                    orienting_exponent: 1,
                },
                MaxPrimePower {
                    prime: 29,
                    exponent: 1,
                    orienting_exponent: 1,
                },
                MaxPrimePower {
                    prime: 31,
                    exponent: 1,
                    orienting_exponent: 1,
                },
                MaxPrimePower {
                    prime: 37,
                    exponent: 1,
                    orienting_exponent: 1,
                },
                MaxPrimePower {
                    prime: 41,
                    exponent: 1,
                    orienting_exponent: 1,
                },
                MaxPrimePower {
                    prime: 43,
                    exponent: 1,
                    orienting_exponent: 1,
                },
                MaxPrimePower {
                    prime: 47,
                    exponent: 1,
                    orienting_exponent: 1,
                },
                MaxPrimePower {
                    prime: 53,
                    exponent: 1,
                    orienting_exponent: 1,
                },
                MaxPrimePower {
                    prime: 59,
                    exponent: 1,
                    orienting_exponent: 1,
                },
            ]
        );
    }
}

#[cfg(test)]
mod divisors {
    use crate::{number_theory::divisors, orderexps::OrderExps};

    fn as_u64s<const N: usize>(n: Vec<OrderExps<N>>) -> Vec<u8> {
        n.into_iter()
            .map(|x| x.as_bigint().try_into().unwrap())
            .collect()
    }

    #[test_log::test]
    fn edge_cases() {
        assert!(divisors::<8>(0).is_empty());
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
    #[should_panic(expected = "assertion failed: u16::from(n) < FIRST_")]
    fn too_big_prime_panics1() {
        let _ = divisors::<32>(251);
    }

    #[test_log::test]
    #[should_panic(expected = "assertion failed: u16::from(n) < FIRST_")]
    fn too_big_prime_panics2() {
        let _ = divisors::<32>(137);
    }
}

#[cfg(test)]
mod orderexps {
    use std::num::NonZeroU16;

    use crate::{FIRST_129_PRIMES, orderexps::OrderExps};

    #[test_log::test]
    fn basic() {
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
    fn prime_too_large() {
        OrderExps::<64>::try_from(NonZeroU16::new(FIRST_129_PRIMES[65]).unwrap()).unwrap();
    }
}

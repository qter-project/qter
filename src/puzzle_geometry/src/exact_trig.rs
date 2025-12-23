use algebraics::prelude::*;
use std::{cmp::Ordering, iter, mem, sync::Mutex, thread};

use algebraics::{
    RealAlgebraicNumber, interval_arithmetic::DyadicFractionInterval, polynomial::Polynomial,
};
use itertools::Itertools;
use num_bigint::BigInt;
use num_rational::Ratio;

use crate::num::{Num, Vector};

/// <https://en.wikipedia.org/wiki/Chebyshev_polynomials>
/// A tuple of (T_n, Branch)
static CHEBYSHEVS: boxcar::Vec<(&'static [BigInt], Mutex<Branch>)> = boxcar::Vec::new();

/// This represents values of (cos, sin)(kπ/n) for k∈0..n. Note that the roots are stored in decreasing order since cos is decreasing along the interval that we have roots for. This is the union of the roots of T_n+1, T_n-1.
/// The extrema of T_n are at cos(kπ/n) for k∈0..n, so we want the roots of cheby-1 and cheby+1
#[derive(Debug)]
enum Branch {
    Dense {
        // Each element is a tuple of (counterclockwise, clockwise) rotations
        cos_sin_vectors: Vec<(&'static Vector<2>, Option<&'static Vector<2>>)>,
    },
    Factors {
        factors: Vec<(Polynomial<BigInt>, Vec<Polynomial<BigInt>>)>,
        numbers: Vec<Num>,
        below: BigInt,
        above: BigInt,
        log2_denom: usize,
    },
    Fork {
        left: Box<Branch>,
        right_start: usize,
        right: Box<Branch>,
    },
}

impl Branch {
    /// Get a vector (cos, sin) of π`nth`/x where this is T_x. Returns `None` if there aren't that many roots.
    fn get(&mut self, nth: usize, counter_clockwise: bool) -> Option<&'static Vector<2>> {
        self.try_simplify();
        // println!("{nth} {self:#?}");

        match self {
            Branch::Dense { cos_sin_vectors } => {
                let value = cos_sin_vectors.get_mut(nth)?;

                Some(if counter_clockwise {
                    value.0
                } else if let Some(v) = value.1 {
                    v
                } else {
                    let [[cos, sin]] = value.0.clone().into_inner();

                    value
                        .1
                        .insert(Box::leak(Box::new(Vector::new([[cos, -sin]]))))
                })
            }
            Branch::Factors {
                factors,
                below,
                above,
                log2_denom,
                numbers,
            } => {
                let mid = &*below + &*above;
                let above: BigInt = above.clone() * 2;
                let below: BigInt = below.clone() * 2;
                let log2_denom = *log2_denom + 1;

                let mut numbers_below = Vec::new();
                let mut numbers_above = Vec::new();
                let mut factors_below = Vec::new();
                let mut factors_above = Vec::new();
                let mut roots_above = 0;

                let denom = BigInt::from(2).pow(log2_denom);
                let threshold = Ratio::new(mid.clone(), denom.clone());
                let threshold_num = Num::from(threshold.clone());

                for number in mem::take(numbers) {
                    if number < threshold_num {
                        numbers_below.push(number);
                    } else {
                        numbers_above.push(number);
                        roots_above += 1;
                    }
                }

                let below_ratio = Ratio::new(below.clone(), denom.clone());
                let above_ratio = Ratio::new(above.clone(), denom);

                // println!("{below_ratio}—{threshold}—{above_ratio}");

                for factor in mem::take(factors) {
                    let sc_below = sign_changes_at(&factor.1, below_ratio.clone());
                    let sc_thresh = sign_changes_at(&factor.1, threshold.clone());
                    let sc_above = sign_changes_at(&factor.1, above_ratio.clone());
                    // println!("SC {}; {sc_below}—{sc_thresh}—{sc_above}", &factor.0);

                    match (sc_below - sc_thresh).cmp(&1) {
                        Ordering::Less => {}
                        Ordering::Equal => {
                            numbers_below.push(Num::from(RealAlgebraicNumber::new_unchecked(
                                factor.0.clone(),
                                DyadicFractionInterval::new(below.clone(), mid.clone(), log2_denom),
                            )));
                        }
                        Ordering::Greater => {
                            factors_below.push(factor.clone());
                        }
                    }

                    let root_count = sc_thresh - sc_above;
                    roots_above += root_count;
                    match root_count.cmp(&1) {
                        Ordering::Less => {}
                        Ordering::Equal => {
                            numbers_above.push(Num::from(RealAlgebraicNumber::new_unchecked(
                                factor.0,
                                DyadicFractionInterval::new(mid.clone(), above.clone(), log2_denom),
                            )));
                        }
                        Ordering::Greater => {
                            factors_above.push(factor);
                        }
                    }
                }

                let below = Branch::Factors {
                    factors: factors_below,
                    numbers: numbers_below,
                    below,
                    above: mid.clone(),
                    log2_denom,
                };
                let above = Branch::Factors {
                    factors: factors_above,
                    numbers: numbers_above,
                    below: mid,
                    above,
                    log2_denom,
                };

                // Remember that cos is decreasing on the interval that we care about, so roots that go above come first
                *self = Branch::Fork {
                    left: Box::new(above),
                    right_start: roots_above,
                    right: Box::new(below),
                };

                self.get(nth, counter_clockwise)
            }
            Branch::Fork {
                left,
                right_start,
                right,
            } => {
                if nth < *right_start {
                    left.get(nth, counter_clockwise)
                } else {
                    right.get(nth - *right_start, counter_clockwise)
                }
            }
        }
    }

    fn try_simplify(&mut self) {
        if let Branch::Fork {
            left,
            right_start: _,
            right,
        } = self
            && let (Branch::Dense { cos_sin_vectors: a }, Branch::Dense { cos_sin_vectors: b }) =
                (&mut **left, &mut **right)
        {
            let mut a = mem::take(a);
            let b = mem::take(b);
            a.extend(b);
            *self = Branch::Dense { cos_sin_vectors: a };
        }

        if let Branch::Factors {
            factors,
            numbers,
            below: _,
            above: _,
            log2_denom: _,
        } = self
            && factors.is_empty()
        {
            *self = Branch::Dense {
                cos_sin_vectors: mem::take(numbers)
                    .into_iter()
                    .sorted_unstable()
                    .rev()
                    .map(mk_vec)
                    .map(|v| (v, None))
                    .collect(),
            }
        }
    }
}

fn mk_vec(num: Num) -> &'static Vector<2> {
    Box::leak(Box::new(Vector::new([[
        num.clone(),
        (Num::from(1) - num.clone() * num).sqrt(),
    ]])))
}

// https://en.wikipedia.org/wiki/Sturm%27s_theorem
fn sign_changes_at(seq: &[Polynomial<BigInt>], ratio: Ratio<BigInt>) -> usize {
    seq.iter()
        .map(|v| v.eval_generic(&ratio, Ratio::zero()).cmp(&Ratio::zero()))
        .filter_map(|v| match v {
            Ordering::Less => Some(false),
            Ordering::Equal => None,
            Ordering::Greater => Some(true),
        })
        .tuple_windows()
        .filter(|(a, b)| a != b)
        .count()
}

fn await_cheby(n: usize) -> (&'static [BigInt], &'static Mutex<Branch>) {
    loop {
        if let Some(item) = CHEBYSHEVS.get(n) {
            return (item.0, &item.1);
        }

        thread::yield_now();
    }
}

fn push_cheby() {
    CHEBYSHEVS.push_with(|idx| {
        if idx == 0 {
            return (
                Box::leak(Box::new([BigInt::from(1)])),
                Mutex::new(Branch::Dense {
                    cos_sin_vectors: Vec::new(),
                }),
            );
        }

        if idx == 1 {
            return (
                Box::leak(Box::new([BigInt::from(0), BigInt::from(1)])),
                Mutex::new(Branch::Dense {
                    cos_sin_vectors: vec![
                        (
                            Box::leak(Box::new(Vector::new([[Num::from(1), Num::from(0)]]))),
                            None,
                        ),
                        (
                            Box::leak(Box::new(Vector::new([[Num::from(-1), Num::from(0)]]))),
                            None,
                        ),
                    ],
                }),
            );
        }

        let (a, _) = await_cheby(idx - 2);
        let (b, _) = await_cheby(idx - 1);

        let cheby = Box::leak(
            iter::once(BigInt::ZERO)
                .chain(b.iter().map(|v| v.clone() * BigInt::from(2)))
                .zip(a.iter().cloned().chain(iter::repeat(BigInt::ZERO)))
                .map(|(a, b)| a - b)
                .collect::<Box<[BigInt]>>(),
        );

        let mut maxima = cheby.to_owned();
        maxima[0] -= 1;
        let maxima_factors = Polynomial::from(maxima).factor().polynomial_factors;
        let mut minima = cheby.to_owned();
        minima[0] += 1;
        let minima_factors = Polynomial::from(minima).factor().polynomial_factors;

        (
            Box::leak(Box::new(cheby)),
            Mutex::new(Branch::Factors {
                factors: maxima_factors
                    .into_iter()
                    .chain(minima_factors)
                    .map(|v| {
                        let poly = v.polynomial;
                        let sturm_seq = poly.to_primitive_sturm_sequence();
                        (poly, sturm_seq)
                    })
                    .collect(),
                below: BigInt::from(-1),
                above: BigInt::from(1),
                log2_denom: 0,
                numbers: Vec::new(),
            }),
        )
    });
}

fn get_cheby(n: usize) -> (&'static [BigInt], &'static Mutex<Branch>) {
    loop {
        if let Some(item) = CHEBYSHEVS.get(n) {
            return (item.0, &item.1);
        }

        push_cheby();
    }
}

fn mk_args(mut n: usize, mut d: usize) -> (usize, usize, bool) {
    n = n.rem_euclid(d);

    // Our system gets `πn/d` instead of `2πn/d` so lets double the numerator
    let mut n = n * 2;
    // Simplify the fraction to make the numbers as small as possible
    let gcd = n.gcd(&d);
    n /= gcd;
    d /= gcd;

    let ccw = if n > d {
        // Then we want a clockwise rotation
        n = 2 * d - n;
        false
    } else {
        true
    };

    (n, d, ccw)
}

/// Get a vector of (cos, sin) of 2πn/d. In effect, this is giving a vector pointing n/d around the circle.
/// This function is memoized.
#[must_use]
pub fn rotation_degree(n: usize, d: usize) -> &'static Vector<2> {
    let (n, d, ccw) = mk_args(n, d);
    // `mk_args` putting the values in range should be sufficient
    get_cheby(d).1.lock().unwrap().get(n, ccw).unwrap()
}

#[cfg(test)]
mod tests {
        use num_bigint::BigInt;

    use crate::{
        exact_trig::{get_cheby, mk_args, rotation_degree},
        num::{Num, Vector},
    };

    #[test]
    fn chebys() {
        assert_eq!(get_cheby(0).0, [1].map(BigInt::from));
        assert_eq!(get_cheby(1).0, [0, 1].map(BigInt::from));
        assert_eq!(get_cheby(2).0, [-1, 0, 2].map(BigInt::from));
        assert_eq!(get_cheby(3).0, [0, -3, 0, 4].map(BigInt::from));
        assert_eq!(get_cheby(4).0, [1, 0, -8, 0, 8].map(BigInt::from));
        assert_eq!(get_cheby(5).0, [0, 5, 0, -20, 0, 16].map(BigInt::from));
        assert_eq!(get_cheby(6).0, [-1, 0, 18, 0, -48, 0, 32].map(BigInt::from));
        assert_eq!(
            get_cheby(7).0,
            [0, -7, 0, 56, 0, -112, 0, 64].map(BigInt::from)
        );
        assert_eq!(
            get_cheby(8).0,
            [1, 0, -32, 0, 160, 0, -256, 0, 128].map(BigInt::from)
        );
        assert_eq!(
            get_cheby(9).0,
            [0, 9, 0, -120, 0, 432, 0, -576, 0, 256].map(BigInt::from)
        );
        assert_eq!(
            get_cheby(10).0,
            [-1, 0, 50, 0, -400, 0, 1120, 0, -1280, 0, 512].map(BigInt::from)
        );
    }

    #[test]
    fn test_mk_args() {
        assert_eq!(mk_args(1, 2), (1, 1, true));
        assert_eq!(mk_args(1, 3), (2, 3, true));
        assert_eq!(mk_args(1, 5), (2, 5, true));
        assert_eq!(mk_args(1, 10), (1, 5, true));
        assert_eq!(mk_args(2, 3), (2, 3, false));
        assert_eq!(mk_args(1, 6), (1, 3, true));
    }

    #[test]
    fn test_rotation_degree() {
        assert_eq!(rotation_degree(1, 2), &Vector::new([[-1, 0]]));
        assert_eq!(rotation_degree(3, 2), &Vector::new([[-1, 0]]));
        assert_eq!(rotation_degree(4, 2), &Vector::new([[1, 0]]));
        assert_eq!(
            rotation_degree(1, 3),
            &Vector::new([[
                Num::from(-1) / Num::from(2),
                Num::from(1) / Num::from(2) * Num::from(3).sqrt(),
            ]])
        );
        assert_eq!(rotation_degree(1, 4), &Vector::new([[0, 1]]));
        let fourth = Num::from(1) / Num::from(4);
        assert_eq!(
            rotation_degree(1, 5),
            &Vector::new([[
                Num::from(5).sqrt() / Num::from(4) - fourth.clone(),
                (Num::from(2) * Num::from(5).sqrt() + Num::from(10)).sqrt() * fourth.clone(),
            ]])
        );
        assert_eq!(
            rotation_degree(1, 10),
            &Vector::new([[
                fourth.clone() * Num::from(5).sqrt() + fourth.clone(),
                fourth.clone() * (Num::from(-2) * Num::from(5).sqrt() + Num::from(10)).sqrt(),
            ]])
        );
        assert_eq!(
            rotation_degree(5, 10),
            &Vector::new([[
                -1, 0
            ]])
        );
        assert_eq!(
            rotation_degree(9, 10),
            &Vector::new([[
                fourth.clone() * Num::from(5).sqrt() + fourth.clone(),
                -(fourth * (Num::from(-2) * Num::from(5).sqrt() + Num::from(10)).sqrt()),
            ]])
        );
    }
}

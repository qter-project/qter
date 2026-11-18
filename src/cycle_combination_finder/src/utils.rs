use std::{sync::nonpoison::Mutex, time::Duration};

use bitgauss::BitMatrix;

use crate::finder::PossibleOrder;

/// Running mean and variance of durations, in seconds, by Welford's algorithm
#[derive(Default)]
pub(crate) struct RunningDurationStats {
    count: u32,
    mean: f64,
    sum_squared_deviations: f64,
}

impl RunningDurationStats {
    /// A duration is an outlier when it is more than this many standard
    /// deviations above the mean
    const OUTLIER_STANDARD_DEVIATIONS: f64 = 3.0;

    /// Whether `duration` is an outlier among the durations pushed so far
    pub(crate) fn is_outlier(&self, duration: Duration) -> bool {
        // The standard deviation needs two durations
        if self.count < 2 {
            return false;
        }
        let standard_deviation = (self.sum_squared_deviations / f64::from(self.count - 1)).sqrt();
        duration.as_secs_f64() > self.mean + Self::OUTLIER_STANDARD_DEVIATIONS * standard_deviation
    }

    pub(crate) fn push(&mut self, duration: Duration) {
        let duration = duration.as_secs_f64();
        self.count += 1;
        let deviation = duration - self.mean;
        self.mean += deviation / f64::from(self.count);
        self.sum_squared_deviations += deviation * (duration - self.mean);
    }
}

/// The percent `done` is of `total`, the first time that percent is reached.
/// `highest_percent` holds the highest percent returned so far.
pub(crate) fn new_percent(
    done: usize,
    total: usize,
    highest_percent: &Mutex<usize>,
) -> Option<usize> {
    let percent = done * 100 / total;
    let mut highest_percent = highest_percent.lock();
    if percent > *highest_percent {
        *highest_percent = percent;
        Some(percent)
    } else {
        None
    }
}

pub(crate) fn gauss_jordan_without_zero_rows(
    m: &mut BitMatrix,
    expected_rows: usize,
) -> Vec<usize> {
    let pivot_cols = m.gauss(true);
    if expected_rows != pivot_cols.len() {
        *m = BitMatrix::build(pivot_cols.len(), m.cols(), |i, j| m[(i, j)]);
    }
    pivot_cols
}

#[must_use]
pub fn orbit_index_cast(orbit_index: usize) -> u16 {
    // We guarantee only `u16` number of orbits
    #[allow(clippy::cast_possible_truncation)]
    let orbit_index = orbit_index as u16;
    orbit_index
}

#[must_use]
pub fn register_index_cast(register_index: usize) -> u16 {
    // We guarantee only `u16` number of registers
    #[allow(clippy::cast_possible_truncation)]
    let register_index = register_index as u16;
    register_index
}

#[must_use]
pub fn possible_order_index_cast(possible_order_index: u32) -> usize {
    // We guarantee only `u32` number of orders for now
    possible_order_index as usize
}

#[must_use]
pub fn prime_index_cast(prime_index: usize) -> u8 {
    // There are max 64 primes
    #[allow(clippy::cast_possible_truncation)]
    let prime_index = prime_index as u8;
    prime_index
}

#[must_use]
pub fn possible_orders_len_cast(len: usize) -> u32 {
    #[allow(clippy::cast_possible_truncation)]
    let len = len as u32;
    len
}

#[must_use]
pub fn dbg_registers<const N: usize>(
    registers: impl IntoIterator<Item = u32>,
    possible_orders: &[PossibleOrder<N>],
) -> String {
    registers
        .into_iter()
        .map(|x| possible_orders[x as usize].order.as_bigint().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[must_use]
pub fn dbg_registers_iter<const N: usize>(
    registers_iter: impl IntoIterator<Item = impl IntoIterator<Item = u32>>,
    possible_orders: &[PossibleOrder<N>],
) -> String {
    registers_iter
        .into_iter()
        .map(|registers| dbg_registers(registers, possible_orders))
        .collect::<Vec<_>>()
        .join("\n")
}

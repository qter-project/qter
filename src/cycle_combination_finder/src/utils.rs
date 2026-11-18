use std::time::Duration;

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

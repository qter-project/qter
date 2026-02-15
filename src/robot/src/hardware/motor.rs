use crate::hardware::config::{Face, Microsteps, RobotConfig};
use log::debug;
use rppal::gpio::{Gpio, Level, OutputPin};
use std::{
    thread,
    time::{Duration, Instant},
};

/// Runs `N` blocks with delays concurrently.
///
/// Each element of `iters` is a generator that runs one block. Yielding a
/// `Duration` will wait for that long before that generator is resumed again.
/// Returns when all blocks are complete.
fn run_many<const N: usize>(mut iters: [impl Iterator<Item = Duration>; N]) {
    let now = Instant::now();
    let mut times: [_; N] = core::array::from_fn(|i| iters[i].next().map(|dur| now + dur));

    loop {
        let mut min = None;
        for (i, time) in times.iter_mut().enumerate() {
            let Some(time) = time else {
                continue;
            };
            match min {
                None => min = Some((i, time)),
                Some((_, min_time)) if *time < *min_time => min = Some((i, time)),
                _ => {}
            }
        }

        if let Some((i, next_update)) = min {
            thread::sleep(next_update.saturating_duration_since(Instant::now()));
            let dur = iters[i].next();
            match dur {
                None => times[i] = None,
                Some(dur) => *next_update += dur,
            }
        } else {
            break;
        }
    }
}

// computes position -> time
fn trapezoid_profile_inv(y: u32, s: u32, v_max: f64, a_max: f64) -> f64 {
    let yf = y as f64;
    let sf = s as f64;
    let thresh = v_max * v_max / a_max;
    if sf > thresh {
        let t1 = v_max / a_max;
        let t2 = sf / v_max;

        if yf <= 0.5 * thresh {
            (yf * 2.0 / a_max).sqrt()
        } else if sf - 0.5 * thresh <= yf {
            (t1 + t2) - ((sf - yf) * 2.0 / a_max).sqrt()
        } else {
            (yf + 0.5 * thresh) / v_max
        }
    } else {
        let t1 = (sf / a_max).sqrt();

        if yf <= sf / 2.0 {
            (yf * 2.0 / a_max).sqrt()
        } else {
            2.0 * t1 - ((sf - yf) * 2.0 / a_max).sqrt()
        }
    }
}

pub struct Motor {
    step: OutputPin,
    dir: OutputPin,
    microsteps: Microsteps,
    v_max: f64,
    a_max: f64,
}

impl Motor {
    pub const FULLSTEPS_PER_REVOLUTION: u32 = 200;

    pub fn new(config: &RobotConfig, face: Face) -> Self {
        fn mk_output_pin(gpio: u8) -> OutputPin {
            debug!(target: "gpio", "attempting to configure GPIO pin {gpio}");
            let mut pin = Gpio::new().unwrap().get(gpio).unwrap().into_output_low();
            pin.set_reset_on_drop(false);
            debug!(target: "gpio", "configured GPIO pin {gpio} as output (initial low)");
            pin
        }

        let microsteps = config.microstep_resolution;
        let mult = (Self::FULLSTEPS_PER_REVOLUTION * microsteps.value()) as f64;
        let motor_config = &config.motors[face];
        Self {
            step: mk_output_pin(motor_config.step_pin),
            dir: mk_output_pin(motor_config.dir_pin),
            microsteps,
            v_max: config.revolutions_per_second * mult,
            a_max: config.max_acceleration * mult,
        }
    }

    pub fn half_the_speed(&mut self) {
        self.v_max /= 2.0;
        self.a_max /= 2.0;
    }

    pub fn double_the_speed(&mut self) {
        self.v_max *= 2.0;
        self.a_max *= 2.0;
    }

    pub fn turn(&mut self, steps: i32) {
        Self::turn_many([self], [steps]);
    }

    pub fn turn_many<const N: usize>(selves: [&mut Motor; N], steps: [i32; N]) {
        fn array_zip<T, U, const N: usize>(a: [T; N], b: [U; N]) -> [(T, U); N] {
            let mut iter_a = IntoIterator::into_iter(a);
            let mut iter_b = IntoIterator::into_iter(b);
            std::array::from_fn(|_| (iter_a.next().unwrap(), iter_b.next().unwrap()))
        }

        let state = array_zip(selves, steps);

        run_many(state.map(|(this, steps): (&mut Motor, i32)| gen move {
            this.dir
                .write(if steps < 0 { Level::Low } else { Level::High });
            let steps = steps.unsigned_abs() * this.microsteps.value();

            for i in 0..steps {
                let t1 = trapezoid_profile_inv(i, steps, this.v_max, this.a_max);
                let t2 = trapezoid_profile_inv(i + 1, steps, this.v_max, this.a_max);
                let delay = Duration::from_secs_f64(t2 - t1) / 2;

                this.step.set_high();
                yield delay;
                this.step.set_low();
                yield delay;
            }
        }));
    }
}

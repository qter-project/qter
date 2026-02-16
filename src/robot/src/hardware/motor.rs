use crate::hardware::{
    UART0, UART4,
    config::{Face, Microsteps, RobotConfig},
    uart::{NodeAddress, UartId, UartNode, regs::IholdIrun},
};
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
fn trapezoid_profile_inv(step: u32, total_steps: u32, v_max: f64, a_max: f64) -> f64 {
    let step = step as f64;
    let total_steps = total_steps as f64;
    let thresh = v_max * v_max / a_max;
    if total_steps > thresh {
        let t1 = v_max / a_max;
        let t2 = total_steps / v_max;

        if step <= 0.5 * thresh {
            (step * 2.0 / a_max).sqrt()
        } else if total_steps - 0.5 * thresh <= step {
            (t1 + t2) - ((total_steps - step) * 2.0 / a_max).sqrt()
        } else {
            (step + 0.5 * thresh) / v_max
        }
    } else {
        let t1 = (total_steps / a_max).sqrt();

        if step <= total_steps / 2.0 {
            (step * 2.0 / a_max).sqrt()
        } else {
            2.0 * t1 - ((total_steps - step) * 2.0 / a_max).sqrt()
        }
    }
}

/*
#[derive(Clone, Copy, Debug)]
pub enum MotorCommand {
    StepCW,
    StepCCW,
}

fn reduce_commands(
    commands: impl Iterator<Item = (f64, MotorCommand)>,
) -> impl Iterator<Item = (f64, LowLevelMotorCommand)> {
    let mut is_cw: Option<bool> = None;

    let mut change_dir = move |new_dir: bool| {
        if is_cw == Some(new_dir) {
            is_cw = Some(new_dir);
            false
        } else {
            is_cw = Some(new_dir);
            true
        }
    };

    commands
        .map(Some)
        .chain(None)
        .tuple_windows()
        .flat_map(move |(a, b)| {
            let (t, a) = a.unwrap();

            let t2 = match b {
                Some((next_t, _)) => t.midpoint(next_t),
                None => t + 1.,
            };

            match a {
                MotorCommand::StepCW => [
                    change_dir(true).then_some((t, LowLevelMotorCommand::MakeCW)),
                    Some((t, LowLevelMotorCommand::StepEnable)),
                    Some((t2, LowLevelMotorCommand::StepDisable)),
                ],
                MotorCommand::StepCCW => [
                    change_dir(true).then_some((t, LowLevelMotorCommand::MakeCCW)),
                    Some((t, LowLevelMotorCommand::StepEnable)),
                    Some((t2, LowLevelMotorCommand::StepDisable)),
                ],
            }
        })
        .flatten()
}

#[derive(Clone, Copy, Debug)]
enum LowLevelMotorCommand {
    MakeCW,
    MakeCCW,
    StepEnable,
    StepDisable,
}
*/

pub struct Motor {
    step: OutputPin,
    dir: OutputPin,
    microsteps: Microsteps,
    v_max: f64,
    a_max: f64,
    overtemp_prewarning: bool,
    holding: bool,
    uart_bus: UartId,
    uart_address: NodeAddress,
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
            uart_bus: motor_config.uart_bus,
            uart_address: motor_config.uart_address,
            holding: false,
            overtemp_prewarning: false,
        }
    }

    pub fn enable_prewarning(&mut self) {
        self.overtemp_prewarning = true;
        self.float();
    }

    pub fn clear_prewarning(&mut self) {
        self.overtemp_prewarning = false
    }

    pub fn hold(&mut self) {
        if self.holding || self.overtemp_prewarning {
            return;
        }

        self.holding = true;

        self.do_uart(|mut uart| {
            let initial_pwmconf = uart.pwmconf();

            let new_pwmconf = initial_pwmconf.with_freewheel(0);

            if new_pwmconf != initial_pwmconf {
                uart.set_pwmconf(new_pwmconf);
            }

            let ihold_irun = IholdIrun::empty()
                .with_ihold(16)
                .with_irun(31)
                .with_iholddelay(0);

            uart.set_iholdirun(ihold_irun);
        })
    }

    pub fn float(&mut self) {
        if !self.holding {
            return;
        }

        self.holding = false;

        self.do_uart(|mut uart| {
            let initial_pwmconf = uart.pwmconf();

            let new_pwmconf = initial_pwmconf.with_freewheel(1);

            if new_pwmconf != initial_pwmconf {
                uart.set_pwmconf(new_pwmconf);
            }

            let ihold_irun = IholdIrun::empty()
                .with_ihold(0)
                .with_irun(31)
                .with_iholddelay(0);

            uart.set_iholdirun(ihold_irun);
        })
    }

    fn do_uart(&self, f: impl FnOnce(UartNode)) {
        let mut bus = match self.uart_bus {
            UartId::Uart0 => UART0.lock().unwrap(),
            UartId::Uart4 => UART4.lock().unwrap(),
        };

        let uart = bus.node(self.uart_address);

        f(uart)
    }

    /*
    fn execute(&mut self, cmd: LowLevelMotorCommand) {
        match cmd {
            LowLevelMotorCommand::MakeCW => {
                self.dir.write(Level::Low);
            }
            LowLevelMotorCommand::MakeCCW => {
                self.dir.write(Level::High);
            }
            LowLevelMotorCommand::StepEnable => {
                self.step.set_high();
            }
            LowLevelMotorCommand::StepDisable => {
                self.step.set_low();
            }
        }
    }
    */

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

            let scale = if this.overtemp_prewarning { 0.5 } else { 1. };

            for i in 0..steps {
                let t1 = trapezoid_profile_inv(i, steps, this.v_max * scale, this.a_max * scale);
                let t2 =
                    trapezoid_profile_inv(i + 1, steps, this.v_max * scale, this.a_max * scale);
                let delay = Duration::from_secs_f64(t2 - t1) / 2;

                this.step.set_high();
                yield delay;
                this.step.set_low();
                yield delay;
            }
        }));
    }
}

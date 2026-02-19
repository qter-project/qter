use std::time::Duration;

use crate::hardware::{
    FULLSTEPS_PER_QUARTER, UART0, UART4,
    config::{Face, RobotConfig},
    motors::{
        Dir,
        accel_profile::{mk_steps_from_inv, specify_dir, trapezoid_profile_inv},
    },
    uart::{NodeAddress, UartId, UartNode, regs::IholdIrun},
};
use log::debug;
use rppal::gpio::{Gpio, Level, OutputPin};

#[derive(Clone, Copy, Debug)]
pub enum MotorCommand {
    StepCW,
    StepCCW,
}

impl MotorCommand {
    pub fn flip_dir(self) -> MotorCommand {
        match self {
            MotorCommand::StepCW => MotorCommand::StepCCW,
            MotorCommand::StepCCW => MotorCommand::StepCW,
        }
    }
}

/// The `f64` is the time to start the _next_ move.
#[derive(Clone, Debug)]
pub struct MotorAction(pub Vec<(f64, MotorCommand)>);

impl MotorAction {
    pub fn time_of(&self) -> f64 {
        self.0.last().map(|v| v.0).unwrap_or(0.)
    }

    pub fn chain(&mut self, other: MotorAction) {
        let self_time = self.time_of();

        self.0.extend(other.0.into_iter().map(|v| {
            (v.0 + self_time, v.1)
        }));
    }

    pub fn reverse(&mut self) {
        let self_time = self.time_of();

        for item in &mut self.0 {
            item.0 *= -1.;
            item.0 += self_time;
        }

        self.0.reverse();
    }
}

fn lower_commands(
    mut commands: impl Iterator<Item = (f64, MotorCommand)>,
) -> impl Iterator<Item = (f64, LoweredMotorCommand)> {
    let mut prev_dir: Option<Dir> = None;

    let mut change_dir = move |new_dir: Dir| {
        if prev_dir == Some(new_dir) {
            false
        } else {
            prev_dir = Some(new_dir);
            true
        }
    };

    let mut prev_time: f64 = 0.;

    gen move {
        while let Some((t, command)) = commands.next() {
            match command {
                MotorCommand::StepCW => {
                    if change_dir(Dir::CW) {
                        yield (t, LoweredMotorCommand::MakeCW)
                    }

                    yield (prev_time.midpoint(t), LoweredMotorCommand::StepEnable);
                    yield (t, LoweredMotorCommand::StepDisable);
                }
                MotorCommand::StepCCW => {
                    if change_dir(Dir::CCW) {
                        yield (t, LoweredMotorCommand::MakeCCW)
                    }

                    yield (prev_time.midpoint(t), LoweredMotorCommand::StepEnable);
                    yield (t, LoweredMotorCommand::StepDisable);
                }
            }

            prev_time = t;
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum LoweredMotorCommand {
    MakeCW,
    MakeCCW,
    StepEnable,
    StepDisable,
}

pub struct Motor {
    step: OutputPin,
    dir: OutputPin,
    overtemp_prewarning: bool,
    holding: bool,
    uart_bus: UartId,
    uart_address: NodeAddress,
    face: Face,
    config: &'static RobotConfig,
}

impl Motor {
    pub fn new(config: &'static RobotConfig, face: Face) -> Self {
        fn mk_output_pin(gpio: u8) -> OutputPin {
            debug!(target: "gpio", "attempting to configure GPIO pin {gpio}");
            let mut pin = Gpio::new().unwrap().get(gpio).unwrap().into_output_low();
            pin.set_reset_on_drop(false);
            debug!(target: "gpio", "configured GPIO pin {gpio} as output (initial low)");
            pin
        }

        let motor_config = &config.motors[face];
        Self {
            step: mk_output_pin(motor_config.step_pin),
            dir: mk_output_pin(motor_config.dir_pin),
            uart_bus: motor_config.uart_bus,
            uart_address: motor_config.uart_address,
            holding: false,
            overtemp_prewarning: false,
            face,
            config,
        }
    }

    pub fn face(&self) -> Face {
        self.face
    }

    pub fn uart_bus(&self) -> UartId {
        self.uart_bus
    }

    pub fn uart_address(&self) -> NodeAddress {
        self.uart_address
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

        self.with_uart(|mut uart| {
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

        self.with_uart(|mut uart| {
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

    fn with_uart(&self, f: impl FnOnce(UartNode)) {
        let mut bus = match self.uart_bus {
            UartId::Uart0 => UART0.lock().unwrap(),
            UartId::Uart4 => UART4.lock().unwrap(),
        };

        let uart = bus.node(self.uart_address);

        f(uart)
    }

    pub fn perform_action(&mut self, commands: MotorAction) -> impl Iterator<Item = Duration> {
        gen move {
            let mut prev_time = 0.;

            let commands = lower_commands(commands.0.into_iter());

            for (time, command) in commands {
                self.perform(command);

                yield Duration::from_secs_f64(time - prev_time);
                prev_time = time;
            }
        }
    }

    fn perform(&mut self, cmd: LoweredMotorCommand) {
        match cmd {
            LoweredMotorCommand::MakeCW => {
                self.dir.write(Level::High);
            }
            LoweredMotorCommand::MakeCCW => {
                self.dir.write(Level::Low);
            }
            LoweredMotorCommand::StepEnable => {
                self.step.set_high();
            }
            LoweredMotorCommand::StepDisable => {
                self.step.set_low();
            }
        }
    }

    pub fn mk_corner_cut_help(&self, dir: Dir, time: f64) -> MotorAction {
        let v_max = self.config.v_max();
        let help_amt = self.config.corner_cut_help;

        let mut out = Vec::new();

        for i in 0..help_amt {
            out.push((i as f64 * v_max, dir.as_step()));
        }

        for i in (0..help_amt).rev() {
            out.push((i as f64 * -v_max + time, dir.opposite().as_step()));
        }

        MotorAction(out)
    }

    pub fn mk_quarter_turn(&self, dir: Dir) -> MotorAction {
        let scale = if self.overtemp_prewarning { 0.5 } else { 1. };
        let steps = FULLSTEPS_PER_QUARTER * self.config.microstep_resolution.value();

        specify_dir(dir, mk_steps_from_inv(trapezoid_profile_inv))(
            steps,
            self.config.v_max() * scale,
            self.config.a_max() * scale,
        )
    }

    pub fn mk_half_turn(&self, dir: Dir) -> MotorAction {
        let scale = if self.overtemp_prewarning { 0.5 } else { 1. };
        let steps = FULLSTEPS_PER_QUARTER * 2 * self.config.microstep_resolution.value();

        specify_dir(dir, mk_steps_from_inv(trapezoid_profile_inv))(
            steps,
            self.config.v_max() * scale,
            self.config.a_max() * scale,
        )
    }
}

use crate::hardware::{
    FULLSTEPS_PER_QUARTER, TurnDir, UART0, UART4,
    config::{Face, RobotConfig},
    uart::{NodeAddress, UartId, UartNode, regs::IholdIrun},
};
use itertools::Itertools;
use log::debug;
use rppal::gpio::{Gpio, Level, OutputPin};
use std::{
    ops::Index,
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

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Dir {
    CW,
    #[allow(clippy::upper_case_acronyms)]
    CCW,
}

impl Dir {
    fn as_step(self) -> HighLevelMotorCommand {
        match self {
            Dir::CW => HighLevelMotorCommand::StepCW,
            Dir::CCW => HighLevelMotorCommand::StepCCW,
        }
    }

    /// Whenever possible, choose the directions of double turns such that they are the same direction as forward turns
    fn same_often(turn1: TurnDir, turn2: TurnDir) -> (Dir, Dir) {
        match (Self::dir_from_turn(turn1), Self::dir_from_turn(turn2)) {
            (None, None) => (Dir::CW, Dir::CW),
            (None, Some(d)) | (Some(d), None) => (d, d),
            (Some(a), Some(b)) => (a, b),
        }
    }

    /// Get the direction of a turn. Returns `None` if the turn is a double turn.
    fn dir_from_turn(turn: TurnDir) -> Option<Dir> {
        match turn {
            TurnDir::Normal => Some(Dir::CW),
            TurnDir::Double => None,
            TurnDir::Prime => Some(Dir::CCW),
        }
    }

    fn opposite(self) -> Dir {
        match self {
            Dir::CW => Dir::CCW,
            Dir::CCW => Dir::CW,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum HighLevelMotorCommand {
    StepCW,
    StepCCW,
}

fn lower_commands(
    commands: impl Iterator<Item = (f64, HighLevelMotorCommand)>,
) -> impl Iterator<Item = (f64, MotorCommand)> {
    let mut prev_dir: Option<Dir> = None;

    let mut change_dir = move |new_dir: Dir| {
        if prev_dir == Some(new_dir) {
            false
        } else {
            prev_dir = Some(new_dir);
            true
        }
    };

    let mut commands = commands.peekable();

    gen move {
        while let Some((t, command)) = commands.next() {
            let t2 = match commands.peek() {
                Some((next_t, _)) => t.midpoint(*next_t),
                None => t + 0.001,
            };

            match command {
                HighLevelMotorCommand::StepCW => {
                    if change_dir(Dir::CW) {
                        yield (t, MotorCommand::MakeCW)
                    }

                    yield (t, MotorCommand::StepEnable);
                    yield (t2, MotorCommand::StepDisable);
                }
                HighLevelMotorCommand::StepCCW => {
                    if change_dir(Dir::CCW) {
                        yield (t, MotorCommand::MakeCCW)
                    }

                    yield (t, MotorCommand::StepEnable);
                    yield (t2, MotorCommand::StepDisable);
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum MotorCommand {
    MakeCW,
    MakeCCW,
    StepEnable,
    StepDisable,
}

type MotorAction = Vec<(f64, HighLevelMotorCommand)>;

fn time_of(action: &MotorAction) -> f64 {
    action.last().map(|v| v.0).unwrap_or(0.)
}

pub struct Motors([Motor; 6]);

impl Motors {
    pub fn new(robot_config: &'static RobotConfig) -> Motors {
        Motors(Face::ALL.map(|face| Motor::new(robot_config, face)))
    }

    pub fn motors(&mut self) -> &mut [Motor; 6] {
        &mut self.0
    }

    pub fn hold_all(&mut self) {
        for motor in &mut self.0 {
            motor.hold();
        }
    }

    pub fn float_all(&mut self) {
        for motor in &mut self.0 {
            motor.float();
        }
    }

    pub fn perform_single(&mut self, face: Face, turn: TurnDir) {
        self.hold_all();

        let (action, dir) = match turn {
            TurnDir::Normal => (self[face].mk_quarter_turn(Dir::CW), Dir::CW),
            TurnDir::Double => (self[face].mk_half_turn(Dir::CW), Dir::CW),
            TurnDir::Prime => (self[face].mk_quarter_turn(Dir::CCW), Dir::CCW),
        };

        let time = time_of(&action);

        let actions = self.0.each_mut().map(|motor| {
            if motor.face == face {
                action.clone()
            } else if motor.face.is_adjacent(face) {
                motor.mk_corner_cut_help(dir, time)
            } else {
                Vec::new()
            }
        });

        self.turn_many(actions);
    }

    pub fn perform_commutative(
        &mut self,
        face1: Face,
        turn1: TurnDir,
        face2: Face,
        turn2: TurnDir,
    ) {
        self.hold_all();

        let (dir1, dir2) = Dir::same_often(turn1, turn2);

        let turn1 = match turn1 {
            TurnDir::Double => self[face1].mk_half_turn(dir1),
            TurnDir::Normal | TurnDir::Prime => self[face1].mk_quarter_turn(dir1),
        };

        let turn2 = match turn2 {
            TurnDir::Double => self[face2].mk_half_turn(dir2),
            TurnDir::Normal | TurnDir::Prime => self[face2].mk_quarter_turn(dir2),
        };

        let time = time_of(&turn1).max(time_of(&turn2));

        let actions = self.0.each_mut().map(|motor| {
            if motor.face == face1 {
                turn1.clone()
            } else if motor.face == face2 {
                turn2.clone()
            } else if dir1 == dir2 {
                motor.mk_corner_cut_help(dir1, time)
            } else {
                Vec::new()
            }
        });

        self.turn_many(actions);
    }

    fn turn_many(&mut self, steps: [MotorAction; 6]) {
        fn array_zip<T, U, const N: usize>(a: [T; N], b: [U; N]) -> [(T, U); N] {
            let mut iter_a = IntoIterator::into_iter(a);
            let mut iter_b = IntoIterator::into_iter(b);
            std::array::from_fn(|_| (iter_a.next().unwrap(), iter_b.next().unwrap()))
        }

        let state = array_zip(self.0.each_mut(), steps);

        run_many(state.map(|(motor, commands)| gen move {
            let mut prev_time = 0.;

            let commands = lower_commands(commands.into_iter());

            for (time, command) in commands {
                yield Duration::from_secs_f64(time - prev_time);
                prev_time = time;

                motor.perform(command);
            }
        }))
    }
}

impl Index<Face> for Motors {
    type Output = Motor;

    fn index(&self, index: Face) -> &Self::Output {
        let (i, _) = Face::ALL.iter().find_position(|v| **v == index).unwrap();

        &self.0[i]
    }
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

    fn perform(&mut self, cmd: MotorCommand) {
        match cmd {
            MotorCommand::MakeCW => {
                self.dir.write(Level::Low);
            }
            MotorCommand::MakeCCW => {
                self.dir.write(Level::High);
            }
            MotorCommand::StepEnable => {
                self.step.set_high();
            }
            MotorCommand::StepDisable => {
                self.step.set_low();
            }
        }
    }

    fn mk_corner_cut_help(&self, dir: Dir, time: f64) -> MotorAction {
        let v_max = self.config.v_max();
        let help_amt = self.config.corner_cut_help;

        let mut out = Vec::new();

        for i in 0..help_amt {
            out.push((i as f64 * v_max, dir.as_step()));
        }

        for i in (0..help_amt).rev() {
            out.push((i as f64 * -v_max + time, dir.opposite().as_step()));
        }

        out
    }

    fn mk_quarter_turn(&self, dir: Dir) -> MotorAction {
        let mut out = Vec::new();

        let step = dir.as_step();
        let scale = if self.overtemp_prewarning { 0.5 } else { 1. };
        let steps = FULLSTEPS_PER_QUARTER * self.config.microstep_resolution.value();

        for i in 0..steps {
            let t = trapezoid_profile_inv(
                i,
                steps,
                self.config.v_max() * scale,
                self.config.a_max() * scale,
            );
            out.push((t, step));
        }

        out
    }

    fn mk_half_turn(&self, dir: Dir) -> MotorAction {
        let mut out = Vec::new();

        let step = dir.as_step();
        let scale = if self.overtemp_prewarning { 0.5 } else { 1. };
        let steps = FULLSTEPS_PER_QUARTER * 2 * self.config.microstep_resolution.value();

        for i in 0..steps {
            let t = trapezoid_profile_inv(
                i,
                steps,
                self.config.v_max() * scale,
                self.config.a_max() * scale,
            );
            out.push((t, step));
        }

        out
    }
}

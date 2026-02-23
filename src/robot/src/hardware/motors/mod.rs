use crate::hardware::{
    TurnDir, UART0, UART4,
    config::{Face, RobotConfig},
    motors::motor::{Motor, MotorAction, MotorCommand},
    uart::{UartId, UartNode},
};
use itertools::Itertools;
use std::{
    ops::Index,
    thread,
    time::{Duration, Instant},
};

mod accel_profile;
mod motor;

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

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Dir {
    CW,
    #[allow(clippy::upper_case_acronyms)]
    CCW,
}

impl Dir {
    fn as_step(self) -> MotorCommand {
        match self {
            Dir::CW => MotorCommand::StepCW,
            Dir::CCW => MotorCommand::StepCCW,
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

        let time = action.time_of();

        let actions = self.0.each_mut().map(|motor| {
            if motor.face() == face {
                action.clone()
            } else if motor.face().is_adjacent(face) {
                motor.mk_corner_cut_help(dir, time)
            } else {
                MotorAction(Vec::new())
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

        let time = turn1.time_of().max(turn2.time_of());

        let actions = self.0.each_mut().map(|motor| {
            if motor.face() == face1 {
                turn1.clone()
            } else if motor.face() == face2 {
                turn2.clone()
            } else if dir1 == dir2 {
                motor.mk_corner_cut_help(dir1, time)
            } else {
                MotorAction(Vec::new())
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

        run_many(state.map(|(motor, commands)| motor.perform_action(commands)))
    }

    pub fn with_uarts(&self, mut f: impl FnMut(UartNode)) {
        let mut uart0 = UART0.lock().unwrap();
        let mut uart4 = UART4.lock().unwrap();

        for face in Face::ALL {
            let motor = &self[face];

            let node = match motor.uart_bus() {
                UartId::Uart0 => uart0.node(motor.uart_address()),
                UartId::Uart4 => uart4.node(motor.uart_address()),
            };

            f(node);
        }
    }
}

impl Drop for Motors {
    fn drop(&mut self) {
        self.float_all();
    }
}

impl Index<Face> for Motors {
    type Output = Motor;

    fn index(&self, index: Face) -> &Self::Output {
        let (i, _) = Face::ALL.iter().find_position(|v| **v == index).unwrap();

        &self.0[i]
    }
}

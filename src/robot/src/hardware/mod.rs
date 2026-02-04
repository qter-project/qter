use clap::ValueEnum;
use log::{debug, info};
use puzzle_theory::permutations::Algorithm;
use std::{
    fmt::Display,
    iter::from_fn,
    ops::Add,
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};
use thread_priority::{
    Error, RealtimeThreadSchedulePolicy, ScheduleParams, ThreadPriority,
    set_thread_priority_and_policy, thread_native_id,
    unix::{ThreadSchedulePolicy, set_current_thread_priority},
};
use tokio::sync::oneshot;

use crate::hardware::{
    config::{Face, Priority, RobotConfig},
    motor::Motor,
    uart::{
        UartBus, UartId,
        regs::{GConf, IholdIrun, NodeConf},
    },
};

pub mod config;
mod motor;
pub mod uart;

pub const FULLSTEPS_PER_REVOLUTION: u32 = 200;
pub const FULLSTEPS_PER_QUARTER: u32 = FULLSTEPS_PER_REVOLUTION / 4;

#[derive(Debug, Clone, Copy)]
pub enum ErrorKind {
    MotorThreadDied,
    // TODO: IMPLEMENT!!!!!
    OverTemperature,
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ErrorKind::MotorThreadDied => "MotorThreadDied",
                ErrorKind::OverTemperature => "OverTemperature",
            }
        )
    }
}

#[derive(Debug, Clone)]
pub struct MotorError {
    kind: ErrorKind,
    message: String,
}

impl Display for MotorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ kind: {}, message: {} }}", self.kind, self.message)
    }
}

impl core::error::Error for MotorError {}

impl From<oneshot::error::RecvError> for MotorError {
    fn from(value: oneshot::error::RecvError) -> Self {
        MotorError {
            kind: ErrorKind::MotorThreadDied,
            message: value.to_string(),
        }
    }
}

impl<T> From<mpsc::SendError<T>> for MotorError {
    fn from(value: mpsc::SendError<T>) -> Self {
        MotorError {
            kind: ErrorKind::MotorThreadDied,
            message: value.to_string(),
        }
    }
}

enum MotorMessage {
    QueueMove((Face, Dir)),
    PrevMovesDone(oneshot::Sender<Result<(), MotorError>>),
}

pub struct RobotHandle {
    motor_thread_handle: mpsc::Sender<MotorMessage>,
    config: RobotConfig,
}

impl RobotHandle {
    /// Initialize the robot such that it is ready for use
    pub fn init(robot_config: RobotConfig) -> RobotHandle {
        uart_init(&robot_config);

        let (tx, rx) = mpsc::channel();

        {
            let robot_config = robot_config.clone();
            thread::spawn(move || motor_thread(rx, robot_config));
        }

        RobotHandle {
            motor_thread_handle: tx,
            config: robot_config,
        }
    }

    pub fn config(&self) -> &RobotConfig {
        &self.config
    }

    pub async fn loop_face_turn(&mut self, face: Face) -> Result<(), MotorError> {
        loop {
            self.motor_thread_handle
                .send(MotorMessage::QueueMove((face, Dir::Normal)))?;
            self.await_moves()?.await?;
        }
    }

    /// Queue a sequence of moves to be performed by the robot
    pub fn queue_move_seq(&mut self, alg: &Algorithm) -> Result<(), MotorError> {
        for move_ in alg.move_seq_iter() {
            let mut move_ = &**move_;
            let dir = if let Some(rest) = move_.strip_suffix('\'') {
                move_ = rest;
                Dir::Prime
            } else if let Some(rest) = move_.strip_suffix('2') {
                move_ = rest;
                Dir::Double
            } else {
                Dir::Normal
            };

            let face: Face = move_.parse().expect("invalid move: {move_}");

            self.motor_thread_handle
                .send(MotorMessage::QueueMove((face, dir)))?;
        }

        Ok(())
    }

    /// Wait for all moves in the queue to be performed
    pub fn await_moves(&self) -> Result<impl Future<Output = Result<(), MotorError>> + 'static, MotorError> {
        let (tx, rx) = oneshot::channel();

        self.motor_thread_handle
            .send(MotorMessage::PrevMovesDone(tx))?;

        Ok(async { rx.await? })
    }
}

/// Which UART port to use (BCM numbering context).
#[derive(Debug, Copy, Clone, ValueEnum)]
pub enum WhichUart {
    Uart0, // TX: 14, RX: 15 (BCM)
    Uart4, // TX: 8, RX: 9 (BCM)
}

/// Helper for accurate sleep intervals.
pub struct Ticker {
    now: Instant,
}

impl Face {
    fn is_opposite(self, rhs: Face) -> bool {
        matches!(
            (self, rhs),
            (Face::R, Face::L)
                | (Face::L, Face::R)
                | (Face::U, Face::D)
                | (Face::D, Face::U)
                | (Face::F, Face::B)
                | (Face::B, Face::F)
        )
    }
}

impl RobotConfig {
    fn compensation(&self, face: Face, dir: Dir) -> i32 {
        let sign = dir.qturns().signum();
        let motor_config = &self.motors[face];
        let for_motor = match sign {
            1 => motor_config.pos_compensation,
            -1 => motor_config.neg_compensation,
            _ => unreachable!(),
        };
        let unsigned = for_motor.unwrap_or(self.compensation);
        unsigned.cast_signed() * sign
    }
}

impl Ticker {
    pub fn new() -> Self {
        Self {
            now: Instant::now(),
        }
    }

    pub fn wait(&mut self, delay: Duration) {
        // Advance the expected next time and sleep until that instant.
        self.now += delay;
        thread::sleep(self.now.saturating_duration_since(Instant::now()));
    }
}

impl Default for Ticker {
    fn default() -> Self {
        Self::new()
    }
}

struct CommutativeMoveFsm {
    // stores the entire preceding commutative subsequence, which can always be
    // collapsed to up to two moves.
    // invariant: if only one of them is `Some`, it must be `state[0]`, not `state[1]`.
    state: [Option<(Face, Dir)>; 2],
}

#[derive(Debug, Clone, Copy)]
enum MoveInstruction {
    Single((Face, Dir)),
    Double([(Face, Dir); 2]),
}

impl CommutativeMoveFsm {
    fn new() -> Self {
        Self {
            state: [None, None],
        }
    }

    /// Flushes any backlog of moves. After executing the resulting moves, The
    /// actual state will be fully caught up with the moves fed into the FSM.
    ///
    /// Calling this method may mean that some commutative moves will not
    /// actually end up collapsed.
    fn flush(&mut self) -> Option<MoveInstruction> {
        let res = match self.state {
            [None, Some(_)] => unreachable!(),

            [None, None] => None,
            [Some(move1), None] => Some(MoveInstruction::Single(move1)),
            [Some(move1), Some(move2)] => Some(MoveInstruction::Double([move1, move2])),
        };
        self.state = [None, None];
        res
    }

    fn is_empty(&self) -> bool {
        self.state[0].is_none() && self.state[1].is_none()
    }

    /// Feed a new move into the FSM. Returns some moves to execute; executing
    /// the moves produced by this method will ultimately perform the same
    /// permutation as executing the moves fed into the FSM.
    fn next(&mut self, move_: (Face, Dir)) -> Option<MoveInstruction> {
        // attempts to add this move to the slot in-place, if they are on the *same* face.
        fn try_add(slot: &mut Option<(Face, Dir)>, move_: (Face, Dir)) -> bool {
            let Some((face, dir)) = slot else {
                return false;
            };

            if *face != move_.0 {
                return false;
            }

            if let Some(new_dir) = *dir + move_.1 {
                *dir = new_dir;
            } else {
                *slot = None;
            }

            true
        }

        // handle the case where the new move matches at least one of the moves we already have.
        if try_add(&mut self.state[0], move_) || try_add(&mut self.state[1], move_) {
            if self.state[0].is_none() && self.state[1].is_some() {
                self.state.swap(0, 1);
            }
            return None;
        }

        // handle the case where we have only one move and the new move is commutative.
        if let [Some((face, _)), slot2 @ None] = &mut self.state
            && face.is_opposite(move_.0)
        {
            *slot2 = Some(move_);
            return None;
        }

        // otherwise, this commutative move sequence is over, and we flush the state.
        // (note: this handles the [None, None] case as well)
        let res = self.flush();
        self.state = [Some(move_), None];
        res
    }
}

fn motor_thread(rx: mpsc::Receiver<MotorMessage>, robot_config: RobotConfig) {
    set_prio(robot_config.priority);

    let mut motors: [Motor; 6] = Face::ALL.map(|face| Motor::new(&robot_config, face));

    let mut fsm = CommutativeMoveFsm::new();

    // Unparkers from after the previously executed move
    let mut unparkers = Vec::<oneshot::Sender<Result<(), MotorError>>>::new();

    let iter = from_fn(move || {
        const SHORT_TIMEOUT: Duration = Duration::from_millis(50);
        const NO_TIMEOUT: Duration = Duration::MAX;

        for unparker in unparkers.drain(..) {
            let _ = unparker.send(Ok(()));
        }

        let mut timeout = SHORT_TIMEOUT;

        loop {
            match rx.recv_timeout(timeout) {
                Ok(MotorMessage::QueueMove(move_)) => {
                    // If we get a move, we're ok with waiting at most `SHORT_TIMEOUT` amount of time for one that might commute
                    timeout = SHORT_TIMEOUT;
                    if let Some(instr) = fsm.next(move_) {
                        return Some(instr);
                    }
                }
                Ok(MotorMessage::PrevMovesDone(signal)) => {
                    if fsm.is_empty() {
                        let _ = signal.send(Ok(()));
                    } else {
                        unparkers.push(signal);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    // If we time out, then just send whatever's in the FSM
                    if let Some(instr) = fsm.flush() {
                        return Some(instr);
                    }
                    // If there's nothing in the FSM, then just wait however long for the next move
                    timeout = NO_TIMEOUT;
                }
                // Empty channel
                Err(RecvTimeoutError::Disconnected) => return None,
            }
        }
    });

    for moves in iter {
        info!(
            target: "move_seq",
            "Requested moves: {moves:?}",
        );

        match moves {
            MoveInstruction::Single((face, dir)) => {
                let motor = &mut motors[face as usize];

                let steps = dir.qturns() * FULLSTEPS_PER_QUARTER.cast_signed();
                let comp = robot_config.compensation(face, dir);

                motor.turn(steps + comp);
                motor.turn(-comp);
            }
            MoveInstruction::Double([(face1, dir1), (face2, dir2)]) => {
                let [motor1, motor2] = motors
                    .get_disjoint_mut([face1 as usize, face2 as usize])
                    .unwrap();

                let steps1 = dir1.qturns() * FULLSTEPS_PER_QUARTER.cast_signed();
                let steps2 = dir2.qturns() * FULLSTEPS_PER_QUARTER.cast_signed();
                let comp1 = robot_config.compensation(face1, dir1);
                let comp2 = robot_config.compensation(face2, dir2);

                Motor::turn_many([motor1, motor2], [steps1 + comp1, steps2 + comp2]);
                Motor::turn_many([motor1, motor2], [-comp1, -comp2]);
            }
        }

        info!(
            target: "move_seq",
            "Completed moves: {moves:?}",
        );

        let wait = Duration::from_secs_f64(robot_config.wait_between_moves);
        info!(
            target: "move_seq",
            "Waiting for {wait:?}",
        );
        thread::sleep(wait);
    }

    println!("Completed move sequence");
}

pub fn set_prio(prio: Priority) {
    let res = match prio {
        // Do nothing
        Priority::Default => return,
        // Set niceness to the maximum (-20)
        Priority::MaxNonRT => set_current_thread_priority(ThreadPriority::Max),
        // Set a real-time priority. 80 is above interrupt handlers but below critical kernel functionalities
        // https://shuhaowu.com/blog/2022/04-linux-rt-appdev-part4.html
        Priority::RealTime => set_thread_priority_and_policy(
            thread_native_id(),
            ThreadPriority::from_posix(ScheduleParams { sched_priority: 80 }),
            ThreadSchedulePolicy::Realtime(RealtimeThreadSchedulePolicy::Fifo),
        ),
    };

    if let Err(e) = res {
        if matches!(e, Error::OS(13) | Error::OS(1)) {
            panic!(
                "{e} — You need to configure your system such that userspace applications have permission to raise their priorities (unless you're not on unix in which case idk what that error code means)"
            );
        } else {
            panic!("{e}");
        }
    }
}

pub fn uart_init(robot_config: &RobotConfig) {
    let mut uart0 = UartBus::new(UartId::Uart0);
    let mut uart4 = UartBus::new(UartId::Uart4);

    for face in Face::ALL {
        let config = &robot_config.motors[face];
        let mut uart = match config.uart_bus {
            UartId::Uart0 => &mut uart0,
            UartId::Uart4 => &mut uart4,
        }
        .node(config.uart_address);

        debug!(target: "uart_init", "Initializing {face:?}: uart_bus={:?} node_address={:?}", config.uart_bus, config.uart_address);

        // Set SENDDELAY without performing a read. We can't perform any reads yet *because* we
        // haven't set SENDDELAY. We set NODECONF again later regardless, because this could
        // fail without us knowing.
        // TODO: there has to be a better way to integrate this into the API of `uart`
        debug!(target: "uart_init", "Setting SENDDELAY");
        uart.write_raw(
            NodeConf::ADDRESS,
            NodeConf::empty().with_senddelay(2).bits(),
        );

        //
        // Configure GCONF
        //
        debug!(target: "uart_init", "Reading initial GCONF");
        let initial_gconf = uart.gconf();
        debug!(target: "uart_init", "Read initial GCONF: initial_value={initial_gconf:?}");
        let mut new_gconf = initial_gconf
            .union(GConf::MSTEP_REG_SELECT)
            .union(GConf::PDN_DISABLE)
            .union(GConf::INDEX_OTPW)
            // qter robot turns the opposite direction
            .union(GConf::SHAFT);
        new_gconf.set(GConf::EN_SPREADCYCLE, !robot_config.stealthchop);

        // if config.
        if initial_gconf == new_gconf {
            debug!(target: "uart_init", "GCONF already configured");
        } else {
            debug!(
                target: "uart_init",
                "Writing GCONF: new_value={new_gconf:?}",
            );
            uart.set_gconf(new_gconf);
        }

        //
        // Configure CHOPCONF
        //
        debug!(target: "uart_init", "Reading initial CHOPCONF");
        let initial_chopconf = uart.chopconf();
        debug!(target: "uart_init", "Read initial CHOPCONF: initial_value={initial_chopconf:?}");
        let new_chopconf =
            initial_chopconf.with_mres(robot_config.microstep_resolution.mres_value());
        if new_chopconf == initial_chopconf {
            debug!(target: "uart_init", "CHOPCONF already configured");
        } else {
            debug!(
                target: "uart_init",
                "Writing CHOPCONF: new_value={new_chopconf:?}",
            );
            uart.set_chopconf(new_chopconf);
        }

        //
        // Configure PWMCONF.
        //
        debug!(target: "uart_init", "Reading initial PwmConf");
        let initial_pwmconf = uart.pwmconf();
        debug!(target: "uart_init", "Read initial PWMCONF: initial_value={initial_pwmconf:?}");
        let new_pwmconf = initial_pwmconf
            // Freewheel mode
            .with_freewheel(if robot_config.float { 1 } else { 0 });
        if new_pwmconf == initial_pwmconf {
            debug!(target: "uart_init", "PWMCONF already configured");
        } else {
            debug!(
                target: "uart_init",
                "Writing PWMCONF: new_value={new_pwmconf:?}",
            );
            uart.set_pwmconf(new_pwmconf);
        }

        //
        // Configure IHOLD_IRUN. Note that IHOLD_IRUN is write-only.
        //
        let ihold_irun = IholdIrun::empty()
            .with_ihold(if robot_config.float { 0 } else { 31 })
            // Set IRUN to 31
            .with_irun(31)
            // Set IHOLDDELAY to 1
            .with_iholddelay(1);
        debug!(
            target: "uart_init",
            "Writing IHOLD_IRUN: value={ihold_irun:?}",
        );
        uart.set_iholdirun(ihold_irun);

        let tpowerdown = 2;
        debug!(
            target: "uart_init",
            "Writing TPOWERODNW: value={tpowerdown:?}",
        );
        uart.set_tpowerdown(tpowerdown);

        debug!(target: "uart_init", "Initialized{face:?}: uart_bus={:?} node_address={:?}", config.uart_bus, config.uart_address);
    }
}

pub fn float(robot_config: &RobotConfig) {
    let mut uart0 = UartBus::new(UartId::Uart0);
    let mut uart4 = UartBus::new(UartId::Uart4);

    for face in Face::ALL {
        let config = &robot_config.motors[face];
        let mut uart = match config.uart_bus {
            UartId::Uart0 => &mut uart0,
            UartId::Uart4 => &mut uart4,
        }
        .node(config.uart_address);

        let pwmconf = uart.pwmconf();
        uart.set_pwmconf(pwmconf.with_freewheel(1));

        uart.set_iholdirun(
            IholdIrun::empty()
                .with_ihold(0)
                .with_irun(0)
                .with_iholddelay(0),
        );
    }
}

pub fn estop(robot_config: &RobotConfig) {}

#[derive(Debug, Clone, Copy)]
enum Dir {
    Normal,
    Double,
    Prime,
}

impl Dir {
    fn qturns(self) -> i32 {
        match self {
            Dir::Normal => 1,
            Dir::Double => 2,
            Dir::Prime => -1,
        }
    }
}

impl Add<Dir> for Dir {
    type Output = Option<Dir>;

    fn add(self, rhs: Dir) -> Self::Output {
        match (self, rhs) {
            (Dir::Normal, Dir::Prime) => None,
            (Dir::Prime, Dir::Normal) => None,
            (Dir::Double, Dir::Double) => None,
            (Dir::Double, Dir::Prime) => Some(Dir::Normal),
            (Dir::Prime, Dir::Double) => Some(Dir::Normal),
            (Dir::Normal, Dir::Normal) => Some(Dir::Double),
            (Dir::Prime, Dir::Prime) => Some(Dir::Double),
            (Dir::Normal, Dir::Double) => Some(Dir::Prime),
            (Dir::Double, Dir::Normal) => Some(Dir::Prime),
        }
    }
}

impl Display for Dir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Dir::Normal => f.write_str("Normal"),
            Dir::Double => f.write_str("Double"),
            Dir::Prime => f.write_str("Prime"),
        }
    }
}

use chrono::{DateTime, Utc};
use clap::ValueEnum;
use log::{debug, error, info, warn};
use puzzle_theory::permutations::Algorithm;
use std::{
    fmt::Display,
    ops::Add,
    sync::{
        LazyLock, Mutex,
        mpsc::{self, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant},
};
use thread_priority::{
    Error, RealtimeThreadSchedulePolicy, ScheduleParams, ThreadPriority,
    set_thread_priority_and_policy, thread_native_id,
    unix::{ThreadSchedulePolicy, set_current_thread_priority},
};

use crate::{
    ErrorKind, QterRobotError,
    hardware::{
        config::{Face, Priority, RobotConfig},
        motor::Motors,
        uart::{
            UartBus, UartId,
            regs::{DrvStatus, GConf, IholdIrun, NodeConf},
        },
    },
};

pub mod config;
mod motor;
pub mod uart;

pub const FULLSTEPS_PER_REVOLUTION: u32 = 200;
pub const FULLSTEPS_PER_QUARTER: u32 = FULLSTEPS_PER_REVOLUTION / 4;

static UART0: LazyLock<Mutex<UartBus>> = LazyLock::new(|| Mutex::new(UartBus::new(UartId::Uart0)));
static UART4: LazyLock<Mutex<UartBus>> = LazyLock::new(|| Mutex::new(UartBus::new(UartId::Uart4)));

fn mpsc_err<T>(err: mpsc::SendError<T>) -> QterRobotError {
    QterRobotError {
        kind: ErrorKind::MotorThreadDied,
        message: err.to_string(),
    }
}

fn oneshot_err(err: tokio::sync::oneshot::error::RecvError) -> QterRobotError {
    QterRobotError {
        kind: ErrorKind::MotorThreadDied,
        message: err.to_string(),
    }
}

enum MotorMessage {
    QueueMove(
        (
            Face,
            TurnDir,
            tokio::sync::oneshot::Sender<Result<(), QterRobotError>>,
        ),
    ),
    PrevMovesDone(tokio::sync::oneshot::Sender<Result<(), QterRobotError>>),
}

pub struct RobotHandle {
    motor_thread_handle: mpsc::Sender<MotorMessage>,
    config: &'static RobotConfig,
}

impl RobotHandle {
    /// Initialize the robot such that it is ready for use
    pub fn init(robot_config: &'static RobotConfig, now: fn() -> DateTime<Utc>) -> RobotHandle {
        uart_init(robot_config);

        let (tx, rx) = mpsc::channel();

        {
            thread::spawn(move || motor_thread(rx, robot_config, now));
        }

        RobotHandle {
            motor_thread_handle: tx,
            config: robot_config,
        }
    }

    pub fn config(&self) -> &'static RobotConfig {
        self.config
    }

    pub async fn loop_face_turn(&self, face: Face) -> Result<(), QterRobotError> {
        loop {
            let (tx, rx) = tokio::sync::oneshot::channel();
            self.motor_thread_handle
                .send(MotorMessage::QueueMove((face, TurnDir::Normal, tx)))
                .map_err(mpsc_err)?;
            rx.await.map_err(oneshot_err)??;
            self.await_moves()?.await?;
        }
    }

    /// Queue a sequence of moves to be performed by the robot
    pub async fn queue_move_seq(&self, alg: &Algorithm) -> Result<(), QterRobotError> {
        let mut oneshots = Vec::new();

        for move_ in alg.move_seq_iter() {
            let mut move_ = &**move_;
            let dir = if let Some(rest) = move_.strip_suffix('\'') {
                move_ = rest;
                TurnDir::Prime
            } else if let Some(rest) = move_.strip_suffix('2') {
                move_ = rest;
                TurnDir::Double
            } else {
                TurnDir::Normal
            };

            let face: Face = move_.parse().expect("invalid move: {move_}");

            let (tx, rx) = tokio::sync::oneshot::channel();

            self.motor_thread_handle
                .send(MotorMessage::QueueMove((face, dir, tx)))
                .map_err(mpsc_err)?;

            oneshots.push(rx);
        }

        for oneshot in oneshots {
            oneshot.await.map_err(oneshot_err)??;
        }

        Ok(())
    }

    /// Wait for all moves in the queue to be performed
    pub fn await_moves(
        &self,
    ) -> Result<impl Future<Output = Result<(), QterRobotError>> + 'static, QterRobotError> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.motor_thread_handle
            .send(MotorMessage::PrevMovesDone(tx))
            .map_err(mpsc_err)?;

        let delay = self.config.await_moves_delay;

        Ok(async move {
            rx.await.map_err(oneshot_err)??;
            tokio::time::sleep(Duration::from_millis(delay.ceil() as u64)).await;
            Ok(())
        })
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
    fn compensation(&self, dir: TurnDir) -> i32 {
        let sign = dir.qturns().signum();
        self.compensation.cast_signed() * sign
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
    state: [Option<(Face, TurnDir)>; 2],
}

#[derive(Debug, Clone, Copy)]
enum MoveInstruction {
    Single((Face, TurnDir)),
    Double([(Face, TurnDir); 2]),
    Float,
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
    fn next(&mut self, move_: (Face, TurnDir)) -> Option<MoveInstruction> {
        // attempts to add this move to the slot in-place, if they are on the *same* face.
        fn try_add(slot: &mut Option<(Face, TurnDir)>, move_: (Face, TurnDir)) -> bool {
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

#[derive(Debug, Clone, Copy)]
enum MotorDriverTemperature {
    Normal,
    PreWarning,
    // we don't have a Warning state because we estop immediately when we detect
    // a warning
}

fn motor_driver_thread_watchdog(
    rx: mpsc::Receiver<oneshot::Sender<[MotorDriverTemperature; 6]>>,
    robot_config: RobotConfig,
) {
    let mut motor_driver_temperatures = Face::ALL.map(|_| MotorDriverTemperature::Normal);
    let mut prev_motor_currents = Face::ALL.map(|_| 0);
    loop {
        const POLL_TIMEOUT: Duration = Duration::from_secs(10);
        let signal = match rx.recv_timeout(POLL_TIMEOUT) {
            Ok(signal) => Some(signal),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => {
                return;
            }
        };
        let mut uart0 = UART0.lock().unwrap();
        let mut uart4 = UART4.lock().unwrap();
        for ((face, motor_driver_temperature), prev_motor_current) in Face::ALL
            .into_iter()
            .zip(motor_driver_temperatures.iter_mut())
            .zip(prev_motor_currents.iter_mut())
        {
            let config = &robot_config.motors[face];
            let mut uart = match config.uart_bus {
                UartId::Uart0 => &mut uart0,
                UartId::Uart4 => &mut uart4,
            }
            .node(config.uart_address);

            let drvstatus = uart.drvstatus();
            let motor_current = drvstatus.cs_actual();
            if motor_current != *prev_motor_current {
                debug!(
                    target: "watchdog",
                    "Motor {face:?} current: {:.2}%",
                    motor_current as f64 / 32.0 * 100.0,
                );
                *prev_motor_current = motor_current;
            }
            if drvstatus.contains(DrvStatus::OT) {
                error!(
                    target: "watchdog",
                    "Motor {face:?} overtemperature warning",
                );
                estop();
            }
            if drvstatus.contains(DrvStatus::S2GA) {
                error!(
                    target: "watchdog",
                    "Motor {face:?} short to ground on phase A",
                );
                estop();
            }
            if drvstatus.contains(DrvStatus::S2GB) {
                error!(
                    target: "watchdog",
                    "Motor {face:?} short to ground on phase B",
                );
                estop();
            }
            if drvstatus.contains(DrvStatus::S2VSA) {
                error!(
                    target: "watchdog",
                    "Motor {face:?} low-side short on phase A",
                );
                estop();
            }
            if drvstatus.contains(DrvStatus::S2VSB) {
                error!(
                    target: "watchdog",
                    "Motor {face:?} low-side short on phase B",
                );
                estop();
            }

            if drvstatus.contains(DrvStatus::OTPW) {
                warn!(
                    target: "watchdog",
                    "Motor {face:?} overtemperature pre-warning",
                );
                *motor_driver_temperature = MotorDriverTemperature::PreWarning;
            } else if let MotorDriverTemperature::PreWarning = motor_driver_temperature {
                warn!(
                    target: "watchdog",
                    "Motor {face:?} overtemperature pre-warning cleared",
                );
                *motor_driver_temperature = MotorDriverTemperature::Normal;
            }
            if drvstatus.contains(DrvStatus::OLA) {
                warn!(
                    target: "watchdog",
                    "Motor {face:?} open load detected on phase A",
                );
            }
            if drvstatus.contains(DrvStatus::OLB) {
                warn!(
                    target: "watchdog",
                    "Motor {face:?} open load detected on phase B",
                );
            }
        }
        if let Some(signal) = signal {
            signal.send(motor_driver_temperatures).unwrap();
        }
    }
}

fn motor_thread(
    rx: mpsc::Receiver<MotorMessage>,
    robot_config: &'static RobotConfig,
    now: fn() -> DateTime<Utc>,
) {
    let (watchdox_tx, watchdog_rx) = mpsc::channel();
    {
        let robot_config = robot_config.clone();
        thread::spawn(move || motor_driver_thread_watchdog(watchdog_rx, robot_config));
    }

    set_prio(robot_config.priority);

    let mut motors: Motors = Motors::new(robot_config);

    let mut fsm = CommutativeMoveFsm::new();

    let err_status = || {
        let conference_status = g4g_program::status(now());

        if !conference_status.robot_enabled() {
            return Err(QterRobotError {
                kind: ErrorKind::ActionDuringTalks,
                message: "Cannot use robot while talks are going on".to_owned(),
            });
        }

        Ok(())
    };

    // Unparkers from after the previously executed move
    let mut unparkers = Vec::<tokio::sync::oneshot::Sender<Result<(), QterRobotError>>>::new();

    let iter = gen move {
        const SHORT_TIMEOUT: Duration = Duration::from_millis(50);
        const NO_TIMEOUT: Duration = Duration::MAX;

        loop {
            let err_state = err_status();
            for unparker in unparkers.drain(..) {
                let _ = unparker.send(err_state.clone());
            }

            let mut timeout = SHORT_TIMEOUT;

            loop {
                match rx.recv_timeout(timeout) {
                    Ok(MotorMessage::QueueMove((face, dir, ack))) => {
                        // If we get a move, we're ok with waiting at most `SHORT_TIMEOUT` amount of time for one that might commute
                        timeout = SHORT_TIMEOUT;
                        if let Some(instr) = fsm.next((face, dir)) {
                            match err_status() {
                                Ok(()) => {
                                    ack.send(Ok(())).unwrap();
                                    yield instr;
                                    break;
                                }
                                Err(err) => {
                                    ack.send(Err(err)).unwrap();
                                }
                            };
                        } else {
                            ack.send(Ok(())).unwrap();
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
                        if let Some(instr) = fsm.flush()
                            && err_status().is_ok()
                        {
                            yield instr;
                            break;
                        } else {
                            // If there's nothing in the FSM, then just float and wait however long for the next move
                            yield MoveInstruction::Float;
                            timeout = NO_TIMEOUT;
                        }
                    }
                    // Empty channel
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        }
    };

    let mut prev_motor_driver_temperatures = Face::ALL.map(|_| MotorDriverTemperature::Normal);
    for moves in iter {
        info!(
            target: "move_seq",
            "Requested moves: {moves:?}",
        );

        let (signal_tx, signal_rx) = oneshot::channel();

        watchdox_tx.send(signal_tx).unwrap();
        for ((motor_driver_temperature, prev_motor_driver_temperature), motor) in signal_rx
            .recv()
            .unwrap()
            .iter()
            .zip(prev_motor_driver_temperatures.iter_mut())
            .zip(motors.motors().iter_mut())
        {
            match (motor_driver_temperature, *prev_motor_driver_temperature) {
                (MotorDriverTemperature::PreWarning, MotorDriverTemperature::Normal) => {
                    warn!(
                        target: "move_seq",
                        "Halving the speed of the motors",
                    );
                    motor.enable_prewarning();
                }
                (MotorDriverTemperature::Normal, MotorDriverTemperature::PreWarning) => {
                    warn!(
                        target: "move_seq",
                        "Restoring the speed of the motors",
                    );
                    motor.clear_prewarning();
                }
                _ => {}
            }
            *prev_motor_driver_temperature = *motor_driver_temperature;
        }

        match moves {
            MoveInstruction::Single((face, dir)) => {
                if robot_config.compensation(dir) != 0 {
                    todo!()
                }

                motors.perform_single(face, dir);
            }
            MoveInstruction::Double([(face1, dir1), (face2, dir2)]) => {
                if robot_config.compensation(dir1) != 0 || robot_config.compensation(dir1) != 0 {
                    todo!()
                }

                motors.perform_commutative(face1, dir1, face2, dir2);
            }
            MoveInstruction::Float => {
                motors.float_all();
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
    let mut uart0 = UART0.lock().unwrap();
    let mut uart4 = UART4.lock().unwrap();
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
            .with_freewheel(1);
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
            .with_ihold(0)
            // Set IRUN to 31
            .with_irun(31)
            // Set IHOLDDELAY to 0
            .with_iholddelay(0);
        debug!(
            target: "uart_init",
            "Writing IHOLD_IRUN: value={ihold_irun:?}",
        );
        uart.set_iholdirun(ihold_irun);

        let tpowerdown = 0;
        debug!(
            target: "uart_init",
            "Writing TPOWERODNW: value={tpowerdown:?}",
        );
        uart.set_tpowerdown(tpowerdown);

        debug!(target: "uart_init", "Initialized{face:?}: uart_bus={:?} node_address={:?}", config.uart_bus, config.uart_address);
    }
}

pub fn float(robot_config: &RobotConfig) {
    let mut uart0 = UART0.lock().unwrap();
    let mut uart4 = UART4.lock().unwrap();
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

pub fn estop() -> ! {
    error!("Emergency stop triggered. Immediately stopping all motors and exiting the process.");
    std::process::exit(1);
}

#[derive(Debug, Clone, Copy)]
enum TurnDir {
    Normal,
    Double,
    Prime,
}

impl TurnDir {
    fn qturns(self) -> i32 {
        match self {
            TurnDir::Normal => 1,
            TurnDir::Double => 2,
            TurnDir::Prime => -1,
        }
    }
}

impl Add<TurnDir> for TurnDir {
    type Output = Option<TurnDir>;

    fn add(self, rhs: TurnDir) -> Self::Output {
        match (self, rhs) {
            (TurnDir::Normal, TurnDir::Prime) => None,
            (TurnDir::Prime, TurnDir::Normal) => None,
            (TurnDir::Double, TurnDir::Double) => None,
            (TurnDir::Double, TurnDir::Prime) => Some(TurnDir::Normal),
            (TurnDir::Prime, TurnDir::Double) => Some(TurnDir::Normal),
            (TurnDir::Normal, TurnDir::Normal) => Some(TurnDir::Double),
            (TurnDir::Prime, TurnDir::Prime) => Some(TurnDir::Double),
            (TurnDir::Normal, TurnDir::Double) => Some(TurnDir::Prime),
            (TurnDir::Double, TurnDir::Normal) => Some(TurnDir::Prime),
        }
    }
}

impl Display for TurnDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TurnDir::Normal => f.write_str("Normal"),
            TurnDir::Double => f.write_str("Double"),
            TurnDir::Prime => f.write_str("Prime"),
        }
    }
}

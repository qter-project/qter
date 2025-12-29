use super::{InterpretationCommand, interpreter_plugin::InterpretationEvent};
use crate::PROGRAMS;
use crossbeam_channel::{Receiver, RecvError, Sender, TryRecvError};
use interpreter::{
    ActionPerformed, ExecutionState, Interpreter, PausedState,
    puzzle_states::{PuzzleState, RobotLike, RobotLikeDyn},
};
use puzzle_theory::{numbers::{Int, U, lcm_iter}, permutations::{Algorithm, Permutation, PermutationGroup}, puzzle_geometry::parsing::puzzle};
use qter_core::{
    Facelets, 
    architectures::{PuzzleDefinition, chromatic_orders_by_facelets, with_presets},
};
use std::{
    collections::VecDeque,
    sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock},
    thread,
    time::Duration,
};

struct RobotHandle {
    robot: &'static mut (dyn RobotLikeDyn + Send + 'static),
    event_tx: Sender<InterpretationEvent>,
}

static ROBOT_HANDLE: OnceLock<Mutex<RobotHandle>> = OnceLock::new();

pub static CUBE3_DEF: LazyLock<PuzzleDefinition> =
    LazyLock::new(|| with_presets(puzzle("3x3")).into_inner());

pub static CUBE3: LazyLock<Arc<PermutationGroup>> =
    LazyLock::new(|| Arc::clone(&CUBE3_DEF.perm_group));

fn robot_handle() -> MutexGuard<'static, RobotHandle> {
    ROBOT_HANDLE.get().unwrap().lock().unwrap()
}

struct TrackedRobotState;

impl TrackedRobotState {
    /// This WILL NOT TAKE THE INVERSE OF `generator` which is necessary for `print` but not for `repeat until`
    fn halt_quiet(&mut self, facelets: &[usize], generator: &Algorithm) -> Option<Int<U>> {
        let mut sum = Int::<U>::zero();

        let chromatic_orders = chromatic_orders_by_facelets(&generator);
        let order = lcm_iter(facelets.iter().map(|&i| chromatic_orders[i]));

        while !self.facelets_solved(facelets) {
            sum += Int::<U>::one();

            if sum >= order {
                eprintln!(
                    "Decoding failure! Performed as many cycles as the size of the register."
                );
                return None;
            }

            self.compose_into(generator);
        }

        Some(sum)
    }
}

impl PuzzleState for TrackedRobotState {
    type InitializationArgs = ();

    fn initialize(_: Arc<PermutationGroup>, (): ()) -> Self {
        robot_handle().robot.solve();

        TrackedRobotState
    }

    fn compose_into(&mut self, alg: &Algorithm) {
        robot_handle().robot.compose_into(alg);
    }

    fn facelets_solved(&mut self, facelets: &[usize]) -> bool {
        let mut handle = robot_handle();
        let state = handle.robot.take_picture().clone();

        handle
            .event_tx
            .send(InterpretationEvent::CubeState(state.to_owned()))
            .unwrap();

        for &facelet in facelets {
            let maps_to = state.mapping().get(facelet);
            if CUBE3.facelet_colors()[maps_to] != CUBE3.facelet_colors()[facelet] {
                return false;
            }
        }

        true
    }

    fn print(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Option<Int<U>> {
        let before = {
            let mut handle = robot_handle();

            let state = handle.robot.take_picture().to_owned();

            handle
                .event_tx
                .send(InterpretationEvent::CubeState(state.clone()))
                .unwrap();

            state
        };

        let mut generator = generator.to_owned();
        generator.exponentiate(-Int::<U>::one());

        let c = self.halt_quiet(facelets, &generator)?;

        let mut exponentiated = generator.clone();
        exponentiated.exponentiate(c.into());

        self.compose_into(&exponentiated);

        let mut handle = robot_handle();

        if &before != handle.robot.take_picture() {
            eprintln!("Printing did not return the cube to the original state!");
            return None;
        }
        Some(c)
    }

    fn halt(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Option<Int<U>> {
        {
            robot_handle()
                .event_tx
                .send(InterpretationEvent::BeginHalt {
                    facelets: Facelets(facelets.to_owned()),
                })
                .unwrap();
        }

        let mut generator = generator.to_owned();
        generator.exponentiate(-Int::<U>::one());

        let mut sum = Int::<U>::zero();

        let chromatic_orders = chromatic_orders_by_facelets(&generator);
        let order = lcm_iter(facelets.iter().map(|&i| chromatic_orders[i]));

        while !self.facelets_solved(facelets) {
            sum += Int::<U>::one();

            {
                robot_handle()
                    .event_tx
                    .send(InterpretationEvent::HaltCountUp(sum))
                    .unwrap();
            }

            if sum >= order {
                eprintln!(
                    "Decoding failure! Performed as many cycles as the size of the register."
                );
                return None;
            }

            self.compose_into(&generator);
        }

        Some(sum)
    }

    fn repeat_until(&mut self, facelets: &[usize], generator: &Algorithm) -> Option<()> {
        // repeat_until has the same behavior as halt
        self.halt_quiet(facelets, generator).map(|_| ())
    }

    fn solve(&mut self) {
        let mut handle = robot_handle();

        handle
            .event_tx
            .send(InterpretationEvent::CubeState(Permutation::identity()))
            .unwrap();

        handle.robot.solve();
    }
}

struct CommandRx {
    command_rx: Receiver<InterpretationCommand>,
    earlier_commands: VecDeque<InterpretationCommand>,
}

impl CommandRx {
    fn next(&mut self) -> Result<InterpretationCommand, RecvError> {
        match self.earlier_commands.pop_front() {
            Some(cmd) => Ok(cmd),
            None => self.command_rx.recv(),
        }
    }

    fn try_next(&mut self) -> Result<InterpretationCommand, TryRecvError> {
        match self.earlier_commands.pop_front() {
            Some(cmd) => Ok(cmd),
            None => self.command_rx.try_recv(),
        }
    }

    fn try_peek(&mut self) -> Option<&InterpretationCommand> {
        match self.try_next() {
            Ok(v) => {
                self.earlier_commands.push_back(v);
                self.earlier_commands.back()
            }
            Err(_) => None,
        }
    }
}

pub fn interpreter_loop<R: RobotLike + Send + 'static>(
    event_tx: Sender<InterpretationEvent>,
    command_rx: Receiver<InterpretationCommand>,
    args: R::InitializationArgs,
) {
    if ROBOT_HANDLE
        .set(Mutex::new(RobotHandle {
            robot: Box::leak(Box::from(R::initialize(Arc::clone(&CUBE3), args))),
            event_tx,
        }))
        .is_err()
    {
        panic!("Cannot create multiple interpreter threads")
    }

    let mut maybe_interpreter = None;

    let mut command_rx = CommandRx {
        command_rx,
        earlier_commands: VecDeque::new(),
    };

    while let Ok(command) = command_rx.next() {
        use InterpretationCommand as C;

        let mut halted = false;

        match command {
            C::Execute(name) => {
                maybe_interpreter = Some(Interpreter::<TrackedRobotState>::new_only_one_puzzle(
                    Arc::clone(&PROGRAMS.get(&name).unwrap().program),
                    (),
                ));

                robot_handle()
                    .event_tx
                    .send(InterpretationEvent::BeganProgram(name))
                    .unwrap();
            }
            C::Step => {
                let Some(interpreter) = &mut maybe_interpreter else {
                    eprintln!("Cannot step while the interpreter is closed");
                    continue;
                };

                thread::sleep(Duration::from_millis(250));

                use ActionPerformed as A;

                let instr = interpreter.state().program_counter();

                robot_handle()
                    .event_tx
                    .send(InterpretationEvent::ExecutingInstruction { which_one: instr })
                    .unwrap();

                match interpreter.step() {
                    A::Goto { instruction_idx: _ }
                    | A::Added(_)
                    | A::Solved(_)
                    | A::RepeatedUntil {
                        puzzle_idx: _,
                        facelets: _,
                        alg: _,
                    }
                    | A::None => {}
                    A::Paused => match interpreter.state().execution_state() {
                        ExecutionState::Running => unreachable!(),
                        ExecutionState::Paused(paused_state) => match paused_state {
                            PausedState::Halt {
                                maybe_puzzle_idx_and_register: _,
                            } => {
                                robot_handle()
                                    .event_tx
                                    .send(InterpretationEvent::FinishedProgram)
                                    .unwrap();
                                halted = true;
                            }
                            PausedState::Input { max_input, data: _ } => {
                                robot_handle()
                                    .event_tx
                                    .send(InterpretationEvent::Input(*max_input))
                                    .unwrap();
                            }
                            PausedState::Panicked => unreachable!(),
                        },
                    },
                    A::FailedSolvedGoto(by_puzzle_type) => match by_puzzle_type {
                        qter_core::ByPuzzleType::Theoretical(_) => unreachable!(),
                        qter_core::ByPuzzleType::Puzzle((_, facelets)) => robot_handle()
                            .event_tx
                            .send(InterpretationEvent::SolvedGoto {
                                facelets: facelets.clone(),
                            })
                            .unwrap(),
                    },
                    A::SucceededSolvedGoto(by_puzzle_type) => match by_puzzle_type {
                        qter_core::ByPuzzleType::Theoretical(_) => unreachable!(),
                        qter_core::ByPuzzleType::Puzzle((_, _, facelets)) => robot_handle()
                            .event_tx
                            .send(InterpretationEvent::SolvedGoto {
                                facelets: facelets.clone(),
                            })
                            .unwrap(),
                    },
                    A::Panicked => {
                        eprintln!("The interpreter panicked!");
                        halted = true;
                        robot_handle()
                            .event_tx
                            .send(InterpretationEvent::FinishedProgram)
                            .unwrap();
                    }
                }

                while let Some(interpreter_message) = interpreter.state_mut().messages().pop_front()
                {
                    robot_handle()
                        .event_tx
                        .send(InterpretationEvent::Message(interpreter_message))
                        .unwrap();
                }

                while let Some(InterpretationCommand::Step) = command_rx.try_peek() {
                    command_rx.try_next().unwrap();
                }

                robot_handle()
                    .event_tx
                    .send(InterpretationEvent::DoneExecuting)
                    .unwrap();

                if halted {
                    maybe_interpreter = None;
                }
            }
            C::GiveInput(int) => {
                let Some(interpreter) = &mut maybe_interpreter else {
                    robot_handle()
                        .event_tx
                        .send(InterpretationEvent::Message("There is no program loaded. Try loading a program and stepping until you reach an input instruction.".to_owned()))
                        .unwrap();
                    continue;
                };

                if let ExecutionState::Paused(PausedState::Input {
                    max_input: _,
                    data: _,
                }) = interpreter.state().execution_state()
                {
                    if let Err(msg) = interpreter.give_input(int) {
                        robot_handle()
                            .event_tx
                            .send(InterpretationEvent::Message(msg))
                            .unwrap();
                    } else {
                        let mut handle = robot_handle();

                        let state = handle.robot.take_picture().clone();
                        handle.event_tx.send(InterpretationEvent::CubeState(state))
                            .unwrap();
                        handle.event_tx.send(InterpretationEvent::GaveInput).unwrap();
                    }
                } else {
                    robot_handle()
                        .event_tx
                        .send(InterpretationEvent::Message("Cannot give input when an input instruction is not being executed. Try stepping until you reach an input instruction.".to_owned()))
                        .unwrap();
                }
            }
            C::Solve => {
                maybe_interpreter = None;

                TrackedRobotState.solve();
            }
        }
    }
}

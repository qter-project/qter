use std::sync::Arc;

use interpreter::{
    ExecutionState, PausedState,
    puzzle_states::{RemoteRobot, RobotState, SimulatedPuzzle},
};
use puzzle_theory::{permutations::Permutation, puzzle_geometry::PuzzleGeometry};
use qter_core::architectures::Architecture;
use serde::{Deserialize, Serialize};
use tsify::Tsify;
use wasm_bindgen::prelude::*;
use web_sys::js_sys::Function;

use crate::{
    BigInt,
    connection::Connection,
    cube::CubeState,
    program::Program,
    robot_like::{CaptureCubeState, Either},
};

#[derive(Tsify, Serialize)]
#[serde(tag = "kind")]
#[tsify(into_wasm_abi)]
pub enum InterpreterState {
    Running,
    NeedsInput { max_input: BigInt },
    Halted,
}

impl From<&ExecutionState> for InterpreterState {
    fn from(execution_state: &ExecutionState) -> Self {
        match execution_state {
            ExecutionState::Running => Self::Running,
            ExecutionState::Paused(PausedState::Input { max_input, .. }) => Self::NeedsInput {
                max_input: BigInt::from(max_input),
            },
            ExecutionState::Paused(PausedState::Halt { .. }) => Self::Halted,
            ExecutionState::Paused(PausedState::Panicked) => Self::Halted,
        }
    }
}

#[derive(Tsify, Deserialize)]
#[tsify(from_wasm_abi)]
pub struct Callbacks {
    #[serde(with = "serde_wasm_bindgen::preserve")]
    #[tsify(type = "(cube: CubeState) => void")]
    cube_state: Function,
    #[serde(with = "serde_wasm_bindgen::preserve")]
    #[tsify(type = "(newMsg: string) => void")]
    message: Function,
}

type CubeStateCb = impl FnMut(&Permutation);

type Robot = Either<RemoteRobot<Connection>, SimulatedPuzzle>;

#[wasm_bindgen]
pub struct Interpreter {
    inner: interpreter::Interpreter<RobotState<CaptureCubeState<Robot, CubeStateCb>>>,
    message_cb: Function,
}

#[define_opaque(CubeStateCb)]
fn mk_cube_state_cb(
    f: Function,
    puzzle: Arc<PuzzleGeometry>,
    arch: Arc<Architecture>,
) -> CubeStateCb {
    move |perm: &Permutation| {
        let res = f.call1(
            &JsValue::null(),
            &CubeState::new(perm, &puzzle, &arch).into(),
        );
        if let Err(e) = res {
            web_sys::console::error_1(&e);
        }
    }
}

#[wasm_bindgen]
impl Interpreter {
    // #[wasm_bindgen(constructor)]
    pub async fn init(
        program: &Program,
        connection: Option<Connection>,
        callbacks: Callbacks,
    ) -> Result<Self, JsError> {
        let interpreter = interpreter::Interpreter::new_only_one_puzzle(
            program.inner.clone(),
            (
                connection.map_or(Either::Right(()), Either::Left),
                mk_cube_state_cb(
                    callbacks.cube_state,
                    program.puzzle.clone(),
                    program.arch.clone(),
                ),
            ),
        )
        .await?;

        let mut this = Self {
            inner: interpreter,
            message_cb: callbacks.message,
        };
        this.send_queued_messages();
        Ok(this)
    }

    #[wasm_bindgen(getter)]
    pub fn state(&self) -> InterpreterState {
        InterpreterState::from(self.inner.state().execution_state())
    }

    fn send_queued_messages(&mut self) {
        for msg in self.inner.state_mut().messages().drain(..) {
            let res = self.message_cb.call1(&JsValue::null(), &msg.into());
            if let Err(e) = res {
                web_sys::console::error_1(&e);
            }
        }
    }

    pub async fn step(&mut self) -> Result<(), JsError> {
        self.inner.step().await?;
        self.send_queued_messages();
        Ok(())
    }

    pub async fn give_input(&mut self, input: i64) -> Result<(), JsError> {
        self.inner
            .give_input(input.into())
            .await?
            .map_err(|msg| JsError::new(&msg))?;
        self.send_queued_messages();
        Ok(())
    }

    #[wasm_bindgen(getter)]
    pub fn program_counter(&self) -> usize {
        self.inner.state().program_counter()
    }
}

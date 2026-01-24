use std::sync::Arc;

use interpreter::{
    ExecutionState, PausedState,
    puzzle_states::{RemoteRobot, RobotLike, RobotState},
};
use puzzle_theory::{
    permutations::{Algorithm, Permutation, PermutationGroup},
    puzzle_geometry::PuzzleGeometry,
};
use qter_core::architectures::Architecture;
use serde::Serialize;
use tokio::io::BufReader;
use tokio_util::compat::{FuturesAsyncReadCompatExt, FuturesAsyncWriteCompatExt};
use tsify::Tsify;
use wasm_bindgen::prelude::*;
use wasm_streams::{readable::IntoAsyncRead, writable::IntoAsyncWrite};
use web_sys::{ReadableStream, WritableStream, js_sys::Function};

use crate::{BigInt, cube::CubeState, program::Program};

#[derive(Tsify, Serialize)]
#[serde(tag = "kind")]
#[tsify(into_wasm_abi)]
pub enum StepResult {
    Running,
    NeedsInput { max_input: BigInt },
    Halted,
}

struct CaptureCubeState<T, F>(T, F);

impl<T: RobotLike, F: FnMut(&Permutation)> RobotLike for CaptureCubeState<T, F> {
    type InitializationArg = (T::InitializationArg, F);
    type Error = T::Error;

    async fn initialize(
        perm_group: std::sync::Arc<PermutationGroup>,
        (args, cb): Self::InitializationArg,
    ) -> Result<Self, Self::Error> {
        let mut this = Self(T::initialize(perm_group, args).await?, cb);
        this.0.solve().await?;
        this.1(&Permutation::identity());
        Ok(this)
    }

    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Self::Error> {
        self.0.compose_into(alg).await
    }

    async fn take_picture(&mut self) -> Result<&Permutation, Self::Error> {
        let perm = self.0.take_picture().await?;
        self.1(perm);
        Ok(perm)
    }

    async fn solve(&mut self) -> Result<(), Self::Error> {
        self.0.solve().await
    }
}

type WsConn = (
    BufReader<tokio_util::compat::Compat<IntoAsyncRead<'static>>>,
    tokio_util::compat::Compat<IntoAsyncWrite<'static>>,
);

type CubeStateCb = impl FnMut(&Permutation);

// type Robot = RemoteRobot<WsConn>;
type Robot = interpreter::puzzle_states::SimulatedPuzzle;

#[wasm_bindgen]
pub struct Interpreter {
    inner: interpreter::Interpreter<RobotState<CaptureCubeState<Robot, CubeStateCb>>>,
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
        read: ReadableStream,
        write: WritableStream,
        #[wasm_bindgen(unchecked_param_type = "(cube: CubeState) => void")] cube_state_cb: Function,
    ) -> Result<Self, JsError> {
        // let conn: WsConn = (
        //     BufReader::new(
        //         wasm_streams::ReadableStream::from_raw(read)
        //             .into_async_read()
        //             .compat(),
        //     ),
        //     wasm_streams::WritableStream::from_raw(write)
        //         .into_async_write()
        //         .compat_write(),
        // );
        let conn = ();
        let interpreter = interpreter::Interpreter::new_only_one_puzzle(
            program.inner.clone(),
            (
                conn,
                mk_cube_state_cb(cube_state_cb, program.puzzle.clone(), program.arch.clone()),
            ),
        )
        .await?;
        Ok(Self { inner: interpreter })
    }

    pub async fn step(&mut self) -> Result<StepResult, JsError> {
        self.inner.step().await?;
        Ok(match self.inner.state().execution_state() {
            ExecutionState::Running => StepResult::Running,
            ExecutionState::Paused(PausedState::Input { max_input, .. }) => {
                StepResult::NeedsInput {
                    max_input: BigInt::from(max_input),
                }
            }
            ExecutionState::Paused(PausedState::Halt { .. }) => StepResult::Halted,
            ExecutionState::Paused(PausedState::Panicked) => StepResult::Halted,
        })
    }

    pub fn messages(&mut self) -> Vec<String> {
        self.inner.state_mut().messages().iter().cloned().collect()
    }

    pub fn program_counter(&self) -> usize {
        self.inner.state().program_counter()
    }

    pub async fn give_input(&mut self, input: i64) -> Result<Option<String>, JsError> {
        Ok(self.inner.give_input(input.into()).await?.err())
    }
}

#![feature(gen_blocks)]

use crate::{hardware::RobotHandle, qvis_app::QvisAppHandle, rob_twophase::solve_rob_twophase};
use interpreter::puzzle_states::RobotLike;
use log::trace;
use puzzle_theory::{
    permutations::{Algorithm, Permutation, PermutationGroup},
    puzzle_geometry::parsing::puzzle,
};
use std::{
    error::Error,
    fmt::Display,
    sync::{Arc, LazyLock},
};

pub mod hardware;
pub mod qvis_app;
pub mod rob_twophase;

pub static CUBE3: LazyLock<Arc<PermutationGroup>> =
    LazyLock::new(|| puzzle("3x3").permutation_group());

pub struct QterRobot<'a> {
    robot_handle: &'a mut RobotHandle,
    qvis_app_handle: &'a mut QvisAppHandle,
    cached_picture_state: Option<Permutation>,
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorKind {
    MotorThreadDied,
    ComposeInto,
    Calibration,
    IncorrectPermGroup,
    RobTwophase,
    // TODO: IMPLEMENT!!!!!
    OverTemperature,
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ErrorKind::MotorThreadDied => "Motor thread died",
                ErrorKind::OverTemperature => "Over-temperature",
                ErrorKind::ComposeInto => "Compose-into",
                ErrorKind::Calibration => "Calibration",
                ErrorKind::IncorrectPermGroup => "Incorrect permutation group",
                ErrorKind::RobTwophase => "rob-twophase",
            }
        )
    }
}

#[derive(Debug)]
pub struct QterRobotError {
    kind: ErrorKind,
    message: String,
}

impl Display for QterRobotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{ kind: \"{}\", message:\"{}\" }}",
            self.kind, self.message
        )
    }
}

impl Error for QterRobotError {}

impl<'a> RobotLike for QterRobot<'a> {
    type InitializationArg = (&'a mut RobotHandle, &'a mut QvisAppHandle);
    // TODO: Overtemperature warning, comms issue with the phone camera
    type Error = QterRobotError;

    async fn initialize(
        permutation_group: Arc<PermutationGroup>,
        (robot_handle, qvis_app_handle): Self::InitializationArg,
    ) -> Result<Self, Self::Error> {
        if permutation_group != *CUBE3 {
            return Err(QterRobotError {
                kind: ErrorKind::IncorrectPermGroup,
                message: match permutation_group.maybe_def() {
                    Some(v) => v.to_string(),
                    None => format!("{permutation_group:?}"),
                },
            });
        }

        Ok(Self {
            robot_handle,
            qvis_app_handle,
            cached_picture_state: None,
        })
    }

    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Self::Error> {
        self.robot_handle.queue_move_seq(alg)?;
        self.cached_picture_state.take();

        Ok(())
    }

    async fn take_picture(&mut self) -> Result<&Permutation, Self::Error> {
        trace!("QterRobot: taking picture");
        if self.cached_picture_state.is_none() {
            trace!("QterRobot: no cache, taking picture");
            self.robot_handle.await_moves()?.await?;
            let ret = self
                .qvis_app_handle
                .take_picture()
                .await
                .map_err(|message| QterRobotError {
                    kind: ErrorKind::ComposeInto,
                    message,
                })?;
            self.cached_picture_state = Some(ret);
        } else {
            trace!("QterRobot: using cache");
        }
        Ok(self.cached_picture_state.as_ref().unwrap())
    }

    async fn compose_perm(&mut self, perm: &Permutation) -> Result<(), Self::Error> {
        let mut perm = perm.to_owned();
        perm.invert();
        self.compose_into(&solve_rob_twophase(&perm).map_err(|e| QterRobotError {
            kind: ErrorKind::RobTwophase,
            message: e.to_string(),
        })?)
        .await
    }
}

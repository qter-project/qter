#![feature(gen_blocks)]

use crate::{hardware::RobotHandle, qvis_app::QvisAppHandle, rob_twophase::solve_rob_twophase};
use interpreter::puzzle_states::RobotLike;
use puzzle_theory::{
    permutations::{Algorithm, Permutation, PermutationGroup},
    puzzle_geometry::parsing::puzzle,
};
use std::{convert::Infallible, sync::{Arc, LazyLock}};

pub mod hardware;
pub mod qvis_app;
pub mod rob_twophase;

pub static CUBE3: LazyLock<Arc<PermutationGroup>> =
    LazyLock::new(|| puzzle("3x3").permutation_group());

pub struct QterRobot {
    simulated_state: Permutation,
    robot_handle: RobotHandle,
    qvis_app_handle: QvisAppHandle,
    cached_picture_state: Option<Permutation>,
}

impl RobotLike for QterRobot {
    type InitializationArgs = (RobotHandle, QvisAppHandle);
    // TODO: Overtemperature warning, comms issue with the phone camera
    type Error = Infallible;

    async fn initialize(
        _: Arc<PermutationGroup>,
        robot_and_qvis_app_handles: (RobotHandle, QvisAppHandle),
    ) -> Result<Self, Self::Error> {
        Ok(QterRobot {
            robot_handle: robot_and_qvis_app_handles.0,
            qvis_app_handle: robot_and_qvis_app_handles.1,
            simulated_state: Permutation::identity(),
            cached_picture_state: Some(Permutation::identity()),
        })
    }

    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Self::Error> {
        self.simulated_state.compose_into(alg.permutation());

        self.robot_handle.queue_move_seq(alg);
        self.cached_picture_state.take();

        Ok(())
    }

    async fn take_picture(&mut self) -> Result<&Permutation, Self::Error> {
        Ok(self.cached_picture_state.get_or_insert_with(|| {
            self.robot_handle.await_moves();

            let ret = self.qvis_app_handle.take_picture();
            assert_eq!(
                ret, self.simulated_state,
                "Simulated state does not match actual state."
            );

            ret
        }))
    }

    async fn solve(&mut self) -> Result<(), Self::Error> {
        let alg = solve_rob_twophase(self.take_picture().await?).unwrap();

        self.compose_into(&alg).await?;

        Ok(())
    }
}

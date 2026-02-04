#![feature(gen_blocks)]

use crate::{
    hardware::{MotorError, RobotHandle},
    qvis_app::QvisAppHandle,
    rob_twophase::solve_rob_twophase,
};
use interpreter::puzzle_states::RobotLike;
use puzzle_theory::{
    permutations::{Algorithm, Permutation, PermutationGroup},
    puzzle_geometry::parsing::puzzle,
};
use std::sync::{Arc, LazyLock};

pub mod hardware;
pub mod qvis_app;
pub mod rob_twophase;

pub static CUBE3: LazyLock<Arc<PermutationGroup>> =
    LazyLock::new(|| puzzle("3x3").permutation_group());

const CALIBRATION_ALGORITHM: &str = "L2 U2 B D2 L R D D2 B F' U D D U' D R' U L D' U' D2 F2 U2 R2 U2 D' U U F' L2 F' F' L D2 F' D' B B D D U' L' R R' D' B2 L2 F D' B' L2 F2 B' D2 B2 R' L2 F' B2 U L B' R' R2 F' D' R2 R B R' D' B' R' U2 B L2 R' B2 R2 D B' L2 F2 D2 L D R U' B R2 R2 R B' F' D2 D' D L2 F' F R' D R' U2 L2 R' D U' R' F' U2 F' D' R2 U L R2";

pub struct QterRobot {
    simulated_state: Permutation,
    robot_handle: RobotHandle,
    qvis_app_handle: QvisAppHandle,
    cached_picture_state: Option<Permutation>,
}

impl RobotLike for QterRobot {
    type InitializationArg = (RobotHandle, QvisAppHandle);
    // TODO: Overtemperature warning, comms issue with the phone camera
    type Error = MotorError;

    async fn initialize(
        cube3_permutation_group: Arc<PermutationGroup>,
        robot_and_qvis_app_handles: (RobotHandle, QvisAppHandle),
    ) -> Result<Self, Self::Error> {
        let (robot_handle, qvis_app_handle) = robot_and_qvis_app_handles;

        let mut acc = Permutation::identity();
        qvis_app_handle.take_picture(Some(acc.clone()));
        for move_ in Algorithm::parse_from_string(
            Arc::clone(&cube3_permutation_group),
            CALIBRATION_ALGORITHM,
        )
        .unwrap()
        .move_seq_iter()
        {
            let move_ = cube3_permutation_group.get_generator(move_).unwrap();
            acc.compose_into(move_);
            qvis_app_handle.take_picture(Some(acc.clone()));
        }

        Ok(QterRobot {
            robot_handle,
            qvis_app_handle,
            simulated_state: Permutation::identity(),
            cached_picture_state: Some(Permutation::identity()),
        })
    }

    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Self::Error> {
        self.simulated_state.compose_into(alg.permutation());

        self.robot_handle.queue_move_seq(alg)?;
        self.cached_picture_state.take();

        Ok(())
    }

    async fn take_picture(&mut self) -> Result<&Permutation, Self::Error> {
        // Refactor once polonius exists
        if self.cached_picture_state.is_none() {
            self.robot_handle.await_moves()?.await?;

            let ret = self.qvis_app_handle.take_picture(None);
            assert_eq!(
                ret, self.simulated_state,
                "Simulated state does not match actual state."
            );

            self.cached_picture_state = Some(ret);
        }

        Ok(self.cached_picture_state.as_ref().unwrap())
    }

    async fn solve(&mut self) -> Result<(), Self::Error> {
        let alg = solve_rob_twophase(self.take_picture().await?).unwrap();

        self.compose_into(&alg).await?;

        Ok(())
    }
}

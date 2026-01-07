#![feature(gen_blocks)]

use crate::{hardware::RobotHandle, qvis_app::QvisAppHandle, rob_twophase::solve_rob_twophase};
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

pub struct QterRobot {
    simulated_state: Permutation,
    robot_handle: RobotHandle,
    qvis_app_handle: QvisAppHandle,
    cached_picture_state: Option<Permutation>,
}

impl RobotLike for QterRobot {
    type InitializationArgs = (RobotHandle, QvisAppHandle);

    fn initialize(
        _: Arc<PermutationGroup>,
        robot_and_qvis_app_handles: (RobotHandle, QvisAppHandle),
    ) -> Self {
        QterRobot {
            robot_handle: robot_and_qvis_app_handles.0,
            qvis_app_handle: robot_and_qvis_app_handles.1,
            simulated_state: Permutation::identity(),
            cached_picture_state: Some(Permutation::identity()),
        }
    }

    fn compose_into(&mut self, alg: &Algorithm) {
        self.simulated_state.compose_into(alg.permutation());

        self.robot_handle.queue_move_seq(alg);
        self.cached_picture_state.take();
    }

    fn take_picture(&mut self) -> &Permutation {
        self.cached_picture_state.get_or_insert_with(|| {
            self.robot_handle.await_moves();

            let ret = self.qvis_app_handle.take_picture();
            assert_eq!(
                ret, self.simulated_state,
                "Simulated state does not match actual state."
            );

            ret
        })
    }

    fn solve(&mut self) {
        let alg = solve_rob_twophase(self.take_picture()).unwrap();

        self.compose_into(&alg);
    }
}

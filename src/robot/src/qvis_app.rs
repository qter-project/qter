use log::{info, warn};
use puzzle_theory::permutations::{Algorithm, Permutation};
use std::{
    path::PathBuf,
    process::Stdio,
    sync::{Arc, LazyLock},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout},
};

use crate::{CUBE3, ErrorKind, QterRobotError, hardware::RobotHandle};

pub struct QvisAppHandle {
    _child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
}

impl QvisAppHandle {
    pub fn init(qvis_app_path: PathBuf) -> Self {
        let mut child = tokio::process::Command::new("cargo")
            .args(["make", "prod"])
            .current_dir(qvis_app_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();

        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap()).lines();

        QvisAppHandle {
            _child: child,
            stdin,
            stdout,
        }
    }

    async fn calibrate_permutation(
        &mut self,
        calibration_permutation: Permutation,
    ) -> Result<(), String> {
        let calibration_command = format!("CALIBRATE {calibration_permutation}\n");
        info!("Sending calibration command: {}", calibration_command);
        self.stdin
            .write_all(calibration_command.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        self.stdin.flush().await.map_err(|e| e.to_string())?;

        while let Some(line) = self.stdout.next_line().await.map_err(|e| e.to_string())? {
            if line.trim() == "DONE" {
                info!("Calibrated permutation");
                return Ok(());
            } else {
                warn!("Received unexpected line from qvis app during calibration: {line}");
            }
        }

        Err("Process exited before sending DONE".into())
    }

    pub async fn take_picture(&mut self) -> Result<Permutation, String> {
        self.stdin
            .write_all(b"TAKE_PICTURE\n")
            .await
            .map_err(|e| e.to_string())?;
        self.stdin.flush().await.map_err(|e| e.to_string())?;

        while let Some(line) = self.stdout.next_line().await.map_err(|e| e.to_string())? {
            if line.starts_with("DONE") {
                let perm_str = line.trim_start_matches("DONE").trim();
                let mut iter = perm_str.split_whitespace();
                let perm = iter.next().unwrap().parse::<Permutation>().map_err(|e| e.to_string());
                let confidence = iter.next().unwrap().parse::<f64>().map_err(|e| e.to_string())?;
                info!("Taken picture of {perm:?} with confidence {confidence}");
                return perm;
            } else {
                warn!("Received unexpected line from qvis app during picture taking: {line}");
            }
        }

        Err("Process exited before sending DONE".into())
    }
}

pub async fn calibrate(
    handle: &mut QvisAppHandle,
    robot: &mut RobotHandle,
) -> Result<(), QterRobotError> {
    static CALIBRATION_ALGORITHM: LazyLock<Algorithm> = LazyLock::new(|| {
        Algorithm::parse_from_string(
            Arc::clone(&CUBE3),
            "L2 U2 B D2 L R D D2 B F' U D D U' D R' U L D' U' D2 F2 U2 R2 U2 D' U U F' L2 F' F' L D2 F' D' B B D D U' L' R R' D' B2 L2 F D' B' L2 F2 B' D2 B2 R' L2 F' B2 U L B' R' R2 F' D' R2 R B R' D' B' R' U2 B L2 R' B2 R2 D B' L2 F2 D2 L D R U' B R2 R2 R B' F' D2 D' D L2 F' F R' D R' U2 L2 R' D U' R' F' U2 F' D' R2 U L R2",
        ).unwrap()
    });

    let mut acc = Permutation::identity();
    handle
        .calibrate_permutation(acc.clone())
        .await
        .map_err(|message| QterRobotError {
            kind: ErrorKind::Calibration,
            message,
        })?;

    info!("Waiting for READY");
    let lines = std::io::stdin().lines();
    for line in lines {
        if line.unwrap().trim() == "READY" {
            break;
        }
    }

    for move_str in CALIBRATION_ALGORITHM.move_seq_iter() {
        let move_perm = CUBE3.get_generator(move_str).unwrap();
        let move_alg =
            Algorithm::new_from_move_seq(Arc::clone(&CUBE3), vec![move_str.clone()]).unwrap();
        acc.compose_into(move_perm);
        robot.queue_move_seq(&move_alg)?;
        robot.await_moves()?.await?;
        info!("Waiting for enter");
        std::io::stdin().lines().next().unwrap().unwrap();
        handle
            .calibrate_permutation(acc.clone())
            .await
            .map_err(|message| QterRobotError {
                kind: ErrorKind::Calibration,
                message,
            })?;
    }

    Ok(())
}

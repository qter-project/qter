use log::warn;
use puzzle_theory::permutations::Permutation;
use std::{path::PathBuf, process::Stdio};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout},
};

pub struct QvisAppHandle {
    _child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
}

impl QvisAppHandle {
    pub fn init(qvis_app_path: PathBuf) -> Self {
        let mut child = tokio::process::Command::new("cargo make prod")
            .current_dir(qvis_app_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
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

    pub async fn calibrate_permutation(
        &mut self,
        calibration_permutation: Permutation,
    ) -> Result<(), String> {
        self.stdin
            .write_all(format!("CALIBRATE {calibration_permutation}\n").as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        self.stdin.flush().await.map_err(|e| e.to_string())?;

        while let Some(line) = self.stdout.next_line().await.map_err(|e| e.to_string())? {
            if line.trim() == "DONE" {
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
                return perm_str
                    .parse::<Permutation>()
                    .map_err(|e| e.to_string());
            } else {
                warn!("Received unexpected line from qvis app during picture taking: {line}");
            }
        }

        Err("Process exited before sending DONE".into())
    }
}

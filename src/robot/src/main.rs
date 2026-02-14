#![warn(clippy::pedantic)]
#![allow(clippy::too_many_lines)]

use clap::{Parser, Subcommand};
use env_logger::TimestampPrecision;
use interpreter::puzzle_states::{RobotLike, SimulatedPuzzle, run_robot_server};
use log::{LevelFilter, debug, warn};
use puzzle_theory::permutations::Algorithm;
use robot::{
    CUBE3, QterRobot,
    hardware::{
        RobotHandle,
        config::{Face, Priority, RobotConfig},
        set_prio,
    },
    qvis_app::{self, QvisAppHandle},
    rob_twophase::solve_rob_twophase,
};
use std::{
    path::PathBuf,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use tiny_http::{Header, Response};
use tokio::io::BufReader;
use wtransport::{Endpoint, Identity, ServerConfig};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// The robot configuration file to use, in TOML format.
    #[arg(long, short = 'c', default_value = "robot_config.toml")]
    robot_config: PathBuf,

    /// Increase logging verbosity (can be repeated)
    #[arg(short, long, action = clap::ArgAction::Count)]
    log_level: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a sequence of moves.
    MoveSeq {
        /// The move sequence to execute, e.g. "R U' F2".
        sequence: String,
    },
    /// Run a motor REPL to control a single motor.
    Motor {
        /// The face to control.
        face: Face,
    },
    /// Stop holding position across all motors.
    Float,
    /// Test latencies at the different options for priority level
    TestPrio {
        prio: Priority,
    },
    /// Host a server to allow the robot to be remote-controlled
    Server {
        server_port: u16,
        #[arg(long)]
        simulated: bool,
    },
    Calibrate,
    Solve,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let cli = Cli::parse();

    env_logger::Builder::new()
        .filter_level(match cli.log_level {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        })
        .format_timestamp(Some(TimestampPrecision::Millis))
        .init();

    let robot_config = toml::from_str::<RobotConfig>(
        &std::fs::read_to_string(&cli.robot_config)
            .expect("Failed to read robot configuration file"),
    )
    .expect("Failed to parse robot configuration file");

    match cli.command {
        Commands::MoveSeq { sequence } => {
            let robot_handle = RobotHandle::init(robot_config);
            robot_handle.queue_move_seq(
                &Algorithm::parse_from_string(Arc::clone(&CUBE3), &sequence)
                    .expect("The algorithm is invalid"),
            )?;
            robot_handle.await_moves()?.await?;
        }
        Commands::Motor { face } => {
            let robot_handle = RobotHandle::init(robot_config);
            robot_handle.loop_face_turn(face).await?;
        }
        Commands::Float => {
            robot::hardware::float(&robot_config);
        }
        Commands::TestPrio { prio } => {
            const SAMPLES: usize = 2048;

            set_prio(prio);
            loop {
                let mut latencies = Vec::<i128>::with_capacity(SAMPLES);

                for _ in 0..SAMPLES {
                    let before = Instant::now();
                    thread::sleep(Duration::from_millis(1));
                    let after = Instant::now();

                    let time = after - before;
                    let nanos: i128 = time.as_nanos().try_into().unwrap();

                    let wrongness = nanos - 1_000_000;
                    latencies.push(wrongness / 1000);
                }

                latencies.sort_unstable();

                println!("M ≈ {}μs", latencies[SAMPLES / 2]);
                println!(
                    "IQR ≈ {}μs",
                    (latencies[SAMPLES * 3 / 4] - latencies[SAMPLES / 4])
                );
                println!("Top 5 = {:?}", &latencies[SAMPLES - 5..SAMPLES]);
            }
        }
        Commands::Server {
            server_port,
            simulated,
        } => {
            let identity = Identity::self_signed(["10.42.0.1", "192.168.191.3"]).unwrap();
            let cert_digest = identity.certificate_chain().as_slice()[0].hash();
            let server_config = ServerConfig::builder()
                .with_bind_default(server_port)
                .with_identity(identity)
                .build();
            let endpoint = Endpoint::server(server_config)?;

            println!(
                "Certificate hash: {}",
                cert_digest.fmt(wtransport::tls::Sha256DigestFmt::DottedHex)
            );
            let server = tiny_http::Server::http(("0.0.0.0", server_port)).unwrap();
            std::thread::spawn(move || {
                for req in server.incoming_requests() {
                    if req.url() == "/cert.json" {
                        let _ = req.respond(
                            Response::from_string(
                                cert_digest.fmt(wtransport::tls::Sha256DigestFmt::BytesArray),
                            )
                            .with_header(
                                Header::from_bytes("Content-Type", "application/json").unwrap(),
                            )
                            .with_header(
                                Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap(),
                            ),
                        );
                    } else {
                        let _ = req.respond(Response::empty(404));
                    }
                }
            });

            let mut maybe_handles = if simulated {
                None
            } else {
                let qvis_app_handle = QvisAppHandle::init(robot_config.qvis_app_path.clone()).await.unwrap();
                let robot_handle = RobotHandle::init(robot_config);
                Some((qvis_app_handle, robot_handle))
            };

            loop {
                debug!("Waiting for connection...");
                let res: color_eyre::Result<()> = async {
                    let session = endpoint.accept().await;
                    debug!("Connection accepted, waiting for request...");
                    let request = session.await?;
                    debug!("Request accepted, waiting for connection...");
                    let conn = request.accept().await?;
                    debug!("Connection accepted, initializing robot server...");
                    let (send, recv) = conn.accept_bi().await?;
                    debug!("Bi-directional stream accepted, running robot server...");
                    let conn = (BufReader::new(recv), send);

                    if simulated {
                        run_robot_server::<_, SimulatedPuzzle>(conn, ()).await?;
                    } else {
                        let (qvis_app_handle, robot_handle) = maybe_handles.as_mut().unwrap();
                        run_robot_server::<_, QterRobot>(conn, (robot_handle, qvis_app_handle))
                            .await?;
                    }

                    Ok(())
                }
                .await;

                if let Err(e) = res {
                    warn!("Error handling connection: {e:?}");
                } else {
                    debug!("Connection handled successfully");
                }
            }
        }
        Commands::Calibrate => {
            let mut qvis_app_handle = QvisAppHandle::init(robot_config.qvis_app_path.clone()).await.unwrap();
            let mut robot_handle = RobotHandle::init(robot_config);
            qvis_app::calibrate(&mut qvis_app_handle, &mut robot_handle).await?;
        }
        Commands::Solve => {
            let mut qvis_app_handle = QvisAppHandle::init(robot_config.qvis_app_path.clone()).await.unwrap();
            let mut robot_handle = RobotHandle::init(robot_config);
            QterRobot::initialize(Arc::clone(&CUBE3), (&mut robot_handle, &mut qvis_app_handle)).await?;
        }
    }

    println!("Exiting");

    Ok(())
}

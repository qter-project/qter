#![warn(clippy::pedantic)]
#![allow(clippy::too_many_lines)]

use clap::{Parser, Subcommand};
use env_logger::TimestampPrecision;
use interpreter::puzzle_states::{RobotLike, run_robot_server};
use log::{LevelFilter, warn};
use puzzle_theory::{permutations::Algorithm, puzzle_geometry::parsing::puzzle};
use robot::{
    CUBE3, QterRobot,
    hardware::{
        RobotHandle,
        config::{Face, Priority, RobotConfig},
        set_prio,
    },
    qvis_app::QvisAppHandle,
    rob_twophase::solve_rob_twophase_string,
};
use std::{
    io::BufReader,
    net::TcpListener,
    path::PathBuf,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

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
        qvis_app_port: u16,
    },
    Solve {
        rob_twophase_string: String,
    },
}

fn main() {
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
            let mut robot_handle = RobotHandle::init(robot_config);
            robot_handle.queue_move_seq(
                &Algorithm::parse_from_string(Arc::clone(&CUBE3), &sequence)
                    .expect("The algorithm is invalid"),
            );
            robot_handle.await_moves();
        }
        Commands::Motor { face } => {
            let mut robot_handle = RobotHandle::init(robot_config);
            robot_handle.loop_face_turn(face);
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
            qvis_app_port,
        } => {
            let listener = TcpListener::bind(format!("0.0.0.0:{server_port}")).unwrap();

            let robot_handle = RobotHandle::init(robot_config);
            let qvis_app_handle = QvisAppHandle::init(qvis_app_port);
            let group = puzzle("3x3").permutation_group();
            let mut robot =
                QterRobot::initialize(Arc::clone(&group), (robot_handle, qvis_app_handle));

            loop {
                let (socket, _) = listener.accept().unwrap();

                run_robot_server::<_, QterRobot>(BufReader::new(socket), &mut robot, &group)
                    .unwrap();
            }
        }
        Commands::Solve {
            rob_twophase_string,
        } => {
            let alg = solve_rob_twophase_string(&rob_twophase_string).unwrap();

            let mut robot_handle = RobotHandle::init(robot_config);
            robot_handle.queue_move_seq(&alg);
            robot_handle.await_moves();
        }
    }
    println!("Exiting");
}

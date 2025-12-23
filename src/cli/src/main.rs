#![warn(clippy::pedantic)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::needless_pass_by_value)]

use std::{fs, io, net::SocketAddr, path::PathBuf, sync::Arc};

use ariadne::{Color, Label, Report, ReportKind, Source};
use clap::{ArgAction, Parser};
use color_eyre::{
    eyre::{OptionExt, eyre},
    owo_colors::OwoColorize,
};
use compiler::compile;
use internment::ArcIntern;
use interpreter::{
    ActionPerformed, ExecutionState, InputRet, Interpreter, PausedState,
    puzzle_states::{PuzzleState, SimulatedPuzzle},
};
use itertools::Itertools;
use puzzle_theory::{numbers::{I, Int}, span::File};
use qter_core::{
    ByPuzzleType, 
    table_encoding::{decode_table, encode_table},
};

/// Compiles and interprets qter programs
#[derive(Parser)]
#[command(version, about)]
enum Commands {
    /// Compile a QAT file to Q
    Compile {
        /// Which file to compile; must be a .q file
        file: PathBuf,
    },
    /// Interpret a QAT or a Q file
    Interpret {
        /// Which file to interpret; must be a .qat or .q file
        file: PathBuf,
        /// The level of execution trace to send to stderr. Can be set zero to three times.
        #[arg(short, action = ArgAction::Count)]
        trace_level: u8,
    },
    /// Step through a QAT or a Q program
    Debug {
        /// Which file to interpret; must be a .qat or .q file
        file: PathBuf,
    },
    /// Evaluate unit tests in a QAT program
    Test {
        /// Which file to test; must be a .qat file
        file: PathBuf,
    },
    /// Execute the opensauce demo
    Demo {
        #[arg(long)]
        remote: Option<SocketAddr>,
    },
    #[cfg(debug_assertions)]
    /// Compress an algorithm table into the special format (This subcommand will not be visible in release mode)
    Compress {
        /// The input alg table
        input: PathBuf,
        /// The output compressed data
        output: PathBuf,
    },
    #[cfg(debug_assertions)]
    /// Print the contents of a compressed algorithm table to stdout (This subcommand will not be visible in release mode)
    Dump {
        /// The input alg table
        input: PathBuf,
    },
}

fn main() -> color_eyre::Result<()> {
    let args = Commands::parse();

    match args {
        Commands::Compile { file: _ } => todo!(),
        Commands::Interpret { file, trace_level } => {
            let program = match file.extension().and_then(|v| v.to_str()) {
                Some("q") => todo!(),
                Some("qat") => {
                    let qat = File::from(fs::read_to_string(&file)?);

                    match compile(&qat, |name| {
                        let path = PathBuf::from(name);

                        if path.ancestors().count() > 1 {
                            // Easier not to implement relative paths and stuff
                            return Err("Imported files must be in the same path".to_owned());
                        }

                        match fs::read_to_string(path) {
                            Ok(s) => Ok(ArcIntern::from(s)),
                            Err(e) => Err(e.to_string()),
                        }
                    }) {
                        Ok(v) => v,
                        Err(errs) => {
                            for err in &errs {
                                Report::build(ReportKind::Error, err.span().clone())
                                    .with_config(
                                        ariadne::Config::new()
                                            .with_index_type(ariadne::IndexType::Byte),
                                    )
                                    .with_message(err.to_string())
                                    .with_label(
                                        Label::new(err.span().clone())
                                            .with_message(err.reason().to_string())
                                            .with_color(Color::Red),
                                    )
                                    .finish()
                                    .eprint(Source::from(qat.inner()))
                                    .unwrap();
                            }

                            return Err(eyre!(
                                "Could not compile {} due to {} errors.",
                                file.display(),
                                errs.len()
                            ));
                        }
                    }
                }
                _ => {
                    return Err(eyre!(
                        "The file {file:?} must have an extension of `.qat` or `.q`."
                    ));
                }
            };

            let interpreter = Interpreter::<SimulatedPuzzle>::new(Arc::new(program), ());
            interpret(interpreter, trace_level)?;
        }
        Commands::Debug { file: _ } => todo!(),
        Commands::Test { file: _ } => todo!(),
        #[cfg(debug_assertions)]
        Commands::Compress { input, output } => {
            let data = fs::read_to_string(input)?;

            let to_encode = data
                .split('\n')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(|alg| {
                    alg.split_whitespace()
                        .filter(|v| !v.is_empty())
                        .map(ArcIntern::from)
                        .collect_vec()
                })
                .collect_vec();

            // for alg in &to_encode {
            //     println!("{}", alg.iter().join(" "));
            // }

            let (data, _) =
                encode_table(&to_encode).ok_or_eyre("Too many unique generators, contact Henry")?;

            fs::write(output, data)?;
        }
        #[cfg(debug_assertions)]
        Commands::Dump { input } => {
            let data = fs::read(input)?;

            let decoded =
                decode_table(&mut data.iter().copied()).ok_or_eyre("Could not decode the table")?;

            for moves in decoded {
                println!("{}", moves.iter().join(" "));
            }
        }
        Commands::Demo { remote } => {
            visualizer::visualizer(remote);
        }
    }

    Ok(())
}

fn interpret<P: PuzzleState>(
    mut interpreter: Interpreter<P>,
    trace_level: u8,
) -> color_eyre::Result<()> {
    if trace_level > 0 {
        return interpret_traced(interpreter, trace_level);
    }
    loop {
        let paused_state = interpreter.step_until_halt();

        let is_input_state = matches!(
            paused_state,
            PausedState::Input {
                max_input: _,
                data: _,
            }
        );

        while let Some(message) = interpreter.state_mut().messages().pop_front() {
            println!("{message}");
        }

        if is_input_state {
            give_number_input(&mut interpreter)?;
        } else {
            break Ok(());
        }
    }
}

fn give_number_input<P: PuzzleState>(
    interpreter: &mut Interpreter<P>,
) -> color_eyre::Result<ByPuzzleType<'static, InputRet>> {
    loop {
        let mut number = String::new();
        io::stdin().read_line(&mut number)?;
        match number.parse::<Int<I>>() {
            Ok(value) => match interpreter.give_input(value) {
                Ok(input_ret) => {
                    break Ok(input_ret);
                }
                Err(e) => println!("{e}"),
            },
            Err(_) => println!("Please input an integer"),
        }
    }
}

fn interpret_traced<P: PuzzleState>(
    mut interpreter: Interpreter<P>,
    trace_level: u8,
) -> color_eyre::Result<()> {
    loop {
        let program_counter = interpreter.state().program_counter() + 1;

        let action = interpreter.step();

        if trace_level >= 3 {
            eprint!("{program_counter} | ");
        }

        let mut should_give_input = false;
        let mut halted = false;

        match action {
            ActionPerformed::None => {
                if trace_level >= 2 {
                    eprintln!("Printing");
                }
            }
            ActionPerformed::Paused => {
                let is_input = matches!(
                    interpreter.state().execution_state(),
                    ExecutionState::Paused(PausedState::Input {
                        max_input: _,
                        data: _
                    })
                );

                if is_input {
                    if trace_level >= 2 {
                        eprintln!("Accepting input");
                    }

                    should_give_input = true;
                } else {
                    if trace_level >= 2 {
                        eprintln!("Halting");
                    }

                    halted = true;
                }
            }
            ActionPerformed::Goto { instruction_idx: _ } => {
                if trace_level >= 3 {
                    eprintln!("Jumping");
                }
            }
            ActionPerformed::FailedSolvedGoto(ByPuzzleType::Theoretical(idx)) => {
                if trace_level >= 2 {
                    eprintln!("Inspect theoretical {} - {}", idx.0, "NOT TAKEN".red());
                }
            }
            ActionPerformed::FailedSolvedGoto(ByPuzzleType::Puzzle((idx, _))) => {
                if trace_level >= 2 {
                    eprintln!("Inspect puzzle {} - {}", idx.0, "NOT TAKEN".red());
                }
            }
            ActionPerformed::SucceededSolvedGoto(ByPuzzleType::Theoretical((_, idx))) => {
                if trace_level >= 2 {
                    eprintln!("Inspect theoretical {} - {}", idx.0, "TAKEN".green());
                }
            }
            ActionPerformed::SucceededSolvedGoto(ByPuzzleType::Puzzle((_, idx, _))) => {
                if trace_level >= 2 {
                    eprintln!("Inspect puzzle {} - {}", idx.0, "TAKEN".green());
                }
            }
            ActionPerformed::Added(ByPuzzleType::Theoretical((idx, amt))) => {
                eprintln!("Theoretical {} += {amt}", idx.0);
            }
            ActionPerformed::Added(ByPuzzleType::Puzzle((idx, alg))) => {
                eprint!("Puzzle {}:", idx.0);

                for move_ in alg.move_seq_iter() {
                    eprint!(" {move_}");
                }

                eprintln!();
            }
            ActionPerformed::Panicked => {
                eprintln!("{}", "Panicked!".red());
                halted = true;
            }
            ActionPerformed::Solved(idx) => {
                eprintln!(
                    "Solved {}",
                    match idx {
                        ByPuzzleType::Theoretical(idx) => idx.0,
                        ByPuzzleType::Puzzle(idx) => idx.0,
                    }
                );
            }
            ActionPerformed::RepeatedUntil {
                puzzle_idx,
                facelets: _,
                alg,
            } => {
                eprint!("Repeated on puzzle {}:", puzzle_idx.0);

                for move_ in alg.move_seq_iter() {
                    eprint!(" {move_}");
                }
            }
        }

        while let Some(interpreter_message) = interpreter.state_mut().messages().pop_front() {
            println!("{interpreter_message}");
        }

        if halted {
            break Ok(());
        }

        if should_give_input {
            let input_ret = give_number_input(&mut interpreter)?;

            match input_ret {
                ByPuzzleType::Theoretical(_) => {}
                ByPuzzleType::Puzzle((idx, alg)) => {
                    eprint!("Puzzle {}:", idx.0);

                    for move_ in alg.move_seq_iter() {
                        eprint!(" {move_}");
                    }

                    eprintln!();
                }
            }
        }
    }
}

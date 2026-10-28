#![warn(clippy::pedantic)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::needless_pass_by_value)]

use std::{
    cell::RefCell,
    collections::HashMap,
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use clap::{ArgAction, Parser};
use color_eyre::{
    eyre::{OptionExt, eyre},
    owo_colors::OwoColorize,
};
use compiler::{Reporter, compile, q_emitter::emit_q};
use crashlog::cargo_metadata;
use interpreter::{
    ActionPerformed, ExecutionState, Interpreter, PausedState,
    puzzle_states::{PuzzleState, SimulatedPuzzle},
};
use itertools::Itertools;
use puzzle_theory::{
    numbers::{I, Int},
    permutations::Algorithm,
};
use qter_core::{
    Program,
    table_encoding::{decode_table, encode_table},
};
use qter_core::{PuzzleIdx, span::File};
use sarlacc::Intern;

/// Compiles and interprets qter programs
#[derive(Parser)]
#[command(version, about)]
enum Commands {
    /// Compile a QAT file to Q
    Compile {
        /// Which file to compile; must be a QAT file
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
    #[cfg(debug_assertions)]
    /// Step through a QAT or a Q program
    Debug {
        /// Which file to interpret; must be a .qat or .q file
        file: PathBuf,
    },
    #[cfg(debug_assertions)]
    /// Evaluate unit tests in a QAT program
    Test {
        /// Which file to test; must be a .qat file
        file: PathBuf,
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

fn print_reports(reporter: &Reporter, sources: &HashMap<Intern<str>, Intern<str>>) {
    let mut cache = ariadne::sources(sources.iter().map(|(name, contents)| (*name, *contents)));

    for (_, report) in reporter.iter() {
        report
            .eprint(&mut cache)
            .unwrap_or_else(|e| eprintln!("Failed to print report: {e}"));
    }
}

fn compile_qat(file: &Path) -> color_eyre::Result<(Program, File)> {
    let path: Intern<str> = Intern::from_owned(format!("{}", file.display()));
    let contents: Intern<str> = Intern::from_owned(fs::read_to_string(file)?);
    let qat = File::new(path, contents);

    let sources: Rc<RefCell<HashMap<Intern<str>, Intern<str>>>> =
        Rc::new(RefCell::new(HashMap::new()));
    sources.borrow_mut().insert(path, contents);
    sources.borrow_mut().insert(
        Intern::from_ref("prelude.qat"),
        Intern::from_ref(include_str!("../../qter_core/prelude.qat")),
    );

    let reporter = Reporter::default();

    if let Some((v, _)) = compile(
        qat,
        {
            let sources = Rc::clone(&sources);
            move |name| {
                let path = PathBuf::from(name);

                if path.ancestors().count() > 1 {
                    // Easier not to implement relative paths and stuff
                    return Err("Imported files must be in the same path".to_owned());
                }

                match fs::read_to_string(path) {
                    Ok(s) => {
                        let contents: Intern<str> = Intern::from_owned(s);
                        sources
                            .borrow_mut()
                            .insert(Intern::from_ref(name), contents);
                        Ok(contents)
                    }
                    Err(e) => Err(e.to_string()),
                }
            }
        },
        &reporter,
    ) {
        Ok((v, qat))
    } else {
        print_reports(&reporter, &sources.borrow());
        Err(eyre!(
            "Could not compile {} due to {} errors.",
            file.display(),
            reporter.iter().count()
        ))
    }
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    crashlog::setup!(cargo_metadata!(), false);

    let args = Commands::parse();

    match args {
        Commands::Compile { file } => {
            if file.extension().and_then(|v| v.to_str()) != Some("qat") {
                return Err(eyre!(
                    "The file `{}` does not have an extension of `.qat`.",
                    file.display()
                ));
            }

            let (program, qat) = compile_qat(&file)?;

            let path = file.with_extension("q");

            let reporter = Reporter::default();

            let q_code = if let Some(v) = emit_q(
                &program,
                Intern::from_owned(path.display().to_string()),
                &reporter,
            ) {
                v.0
            } else {
                let sources = std::iter::once((qat.name(), qat.inner())).collect();
                print_reports(&reporter, &sources);
                return Err(eyre!(
                    "Could not compile {} due to {} errors.",
                    file.display(),
                    reporter.iter().count()
                ));
            };

            fs::write(path, &*q_code.inner())?;
        }
        Commands::Interpret { file, trace_level } => {
            let program = match file.extension().and_then(|v| v.to_str()) {
                Some("q") => todo!(),
                Some("qat") => compile_qat(&file)?.0,
                _ => {
                    return Err(eyre!(
                        "The file `{}` does not have an extension of `.qat` or `.q`.",
                        file.display(),
                    ));
                }
            };

            let interpreter = Interpreter::<SimulatedPuzzle>::new(Arc::new(program), ()).await?;
            interpret(interpreter, trace_level).await?;
        }
        #[cfg(debug_assertions)]
        Commands::Debug { file: _ } => todo!(),
        #[cfg(debug_assertions)]
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
                        .map(Intern::from_ref)
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
    }

    Ok(())
}

async fn interpret<P: PuzzleState>(
    mut interpreter: Interpreter<P>,
    trace_level: u8,
) -> color_eyre::Result<()>
where
    P::Error: Error + Send + Sync + 'static,
{
    if trace_level > 0 {
        return interpret_traced(interpreter, trace_level).await;
    }
    loop {
        let paused_state = interpreter.step_until_halt().await?;

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
            give_number_input(&mut interpreter).await?;
        } else {
            break Ok(());
        }
    }
}

async fn give_number_input<P: PuzzleState>(
    interpreter: &mut Interpreter<P>,
) -> color_eyre::Result<(PuzzleIdx, Algorithm)>
where
    P::Error: Error + Send + Sync + 'static,
{
    loop {
        let mut number = String::new();
        io::stdin().read_line(&mut number)?;
        match number.parse::<Int<I>>() {
            Ok(value) => match interpreter.give_input(value).await? {
                Ok(input_ret) => {
                    break Ok(input_ret);
                }
                Err(e) => println!("{e}"),
            },
            Err(_) => println!("Please input an integer"),
        }
    }
}

async fn interpret_traced<P: PuzzleState>(
    mut interpreter: Interpreter<P>,
    trace_level: u8,
) -> color_eyre::Result<()>
where
    P::Error: Error + Send + Sync + 'static,
{
    loop {
        let program_counter = interpreter.state().program_counter() + 1;

        let action = interpreter.step().await?;

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
            ActionPerformed::FailedSolvedGoto {
                puzzle,
                facelets: _,
            } => {
                if trace_level >= 2 {
                    eprintln!("Inspect puzzle {} - {}", puzzle.0, "NOT TAKEN".red());
                }
            }
            ActionPerformed::SucceededSolvedGoto {
                jumped_to: _,
                puzzle,
                facelets: _,
            } => {
                if trace_level >= 2 {
                    eprintln!("Inspect puzzle {} - {}", puzzle.0, "TAKEN".green());
                }
            }
            ActionPerformed::Added { puzzle, alg } => {
                eprint!("Puzzle {}:", puzzle.0);

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
                eprintln!("Solved {}", idx.0);
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
            let (idx, alg) = give_number_input(&mut interpreter).await?;

            eprint!("Puzzle {}:", idx.0);

            for move_ in alg.move_seq_iter() {
                eprint!(" {move_}");
            }

            eprintln!();
        }
    }
}

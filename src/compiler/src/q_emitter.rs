use chumsky::prelude::Rich;
use internment::ArcIntern;
use itertools::Itertools;
use puzzle_theory::{
    numbers::{Int, U},
    permutations::Algorithm,
    span::{File, Span, WithSpan},
};
use qter_core::{
    Facelets, Halt, Input, Instruction, PerformAlgorithm, Print, Program, RepeatUntil,
    SeparatesByPuzzleType, Solve, SolvedGoto,
};
use std::fmt::Write;

const ALG_MAX_CHARS_WIDTH: usize = 50;

enum QInstruction {
    Goto { instruction_idx: usize },
    SolvedGoto(<SolvedGoto as SeparatesByPuzzleType>::Puzzle<'static>),
    Input(<Input as SeparatesByPuzzleType>::Puzzle<'static>),
    Halt(<Halt as SeparatesByPuzzleType>::Puzzle<'static>),
    Print(<Print as SeparatesByPuzzleType>::Puzzle<'static>),
    PerformAlgorithm(<PerformAlgorithm as SeparatesByPuzzleType>::Puzzle<'static>),
    Solve(<Solve as SeparatesByPuzzleType>::Puzzle<'static>),
    RepeatUntil(<RepeatUntil as SeparatesByPuzzleType>::Puzzle<'static>),
}

/// Convert a `Program` into Q code. The file name will be inserted into all of the spans. Also returns a list of spans for each instruction in the program.
///
/// # Errors
///
/// Returns compile errors if `theoretical` registers are present.
pub fn emit_q(program: &Program, file_name: ArcIntern<str>) -> Result<(File, Box<[Span]>), Vec<Rich<'static, char, Span>>> {
    let mut errors = Vec::new();
    for theoretical in &program.theoretical {
        errors.push(Rich::custom(
            theoretical.span().clone(),
            "Cannot compile a QAT program with theoretical registers",
        ));
    }

    if program.puzzles.len() > 1 {
        errors.push(Rich::custom(
            program.puzzles[1].span().clone(),
            "Compiling with multiple puzzles is unsupported (for now)",
        ));
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    let mut out = String::new();

    out.push_str("Puzzles\n");
    if let Some(puzzle) = program.puzzles.first() {
        writeln!(&mut out, "A: {}", puzzle.span().slice()).unwrap();
    }
    out.push('\n');

    let instrs = convert_instructions(&program.instructions);

    let digits = (instrs.len().max(2) - 1).ilog10() as usize + 1;

    let padding = " ".repeat(digits + 3);

    let mut spans = Vec::new();

    for (i, instr) in instrs.iter().enumerate() {
        let mut num = i.to_string();

        while num.len() < digits {
            num.push(' ');
        }

        let instr = match &**instr {
            QInstruction::Goto { instruction_idx } => format!("goto {instruction_idx}"),
            QInstruction::SolvedGoto((solved_goto, _, facelets)) => {
                format!(
                    "solved-goto {} {}",
                    facelets.pieces().iter().map(|v| &**v).join(" "),
                    solved_goto.instruction_idx
                )
            }
            QInstruction::Input((input, _, algorithm, facelets)) => {
                format!(
                    "input \"{}\"\n{}\n{padding}      max-input {}",
                    input.message,
                    stringify_alg(algorithm, padding.len() + 6, true),
                    facelets.order() - Int::<U>::one(),
                )
            }
            QInstruction::Halt((halt, None)) => {
                format!("halt \"{}\"", halt.message)
            }
            QInstruction::Halt((halt, Some((_, alg, facelets)))) => {
                let mut inverse_alg = alg.clone();
                inverse_alg.exponentiate(-Int::<U>::one());
                format!(
                    "halt \"{}\"\n{}\n{padding}     counting-until {}",
                    halt.message,
                    stringify_alg(&inverse_alg, padding.len() + 5, true),
                    stringify_facelets(facelets),
                )
            }
            QInstruction::Print((halt, None)) => {
                format!("print \"{}\"", halt.message)
            }
            QInstruction::Print((halt, Some((_, alg, facelets)))) => {
                let mut inverse_alg = alg.clone();
                inverse_alg.exponentiate(-Int::<U>::one());
                format!(
                    "print \"{}\"\n{}\n{padding}      counting-until {}",
                    halt.message,
                    stringify_alg(&inverse_alg, padding.len() + 6, true),
                    stringify_facelets(facelets),
                )
            }
            QInstruction::PerformAlgorithm((_, alg)) => stringify_alg(alg, padding.len(), false),
            QInstruction::Solve(_) => "solve".to_string(),
            QInstruction::RepeatUntil(RepeatUntil {
                puzzle_idx: _,
                facelets,
                alg,
            }) => {
                format!(
                    "repeat until {} solved\n{}",
                    stringify_facelets(facelets),
                    stringify_alg(alg, padding.len() + 7, true)
                )
            }
        };

        let start = out.len();
        writeln!(&mut out, "{num} | {instr}").unwrap();
        let end = out.len();

        spans.push((start, end));
    }

    let file = File::new(file_name, ArcIntern::from(out));

    if errors.is_empty() {
        let spans = spans.into_iter().map(|(start, end)| Span::new(file.clone(), start, end)).collect();
        Ok((file, spans))
    } else {
        Err(errors)
    }
}

fn stringify_alg(alg: &Algorithm, padding: usize, pad_first: bool) -> String {
    let padding_str = " ".repeat(padding);
    split_strings(
        &alg.move_seq_iter()
            .map(ArcIntern::clone)
            .collect::<Vec<_>>(),
        ALG_MAX_CHARS_WIDTH - padding,
    )
    .into_iter()
    .enumerate()
    .map(|(i, v)| {
        if i == 0 && !pad_first {
            v
        } else {
            format!("{padding_str}{v}")
        }
    })
    .join("\n")
}

/// Separate a string of characters into lines by the following constraints
/// 1. Lines stay within character bound
///     - When impossible, the string that is longer than the line width is on its own line
/// 2. Minimize line count
/// 3. Make the lines as close to equal length as possible
fn split_strings(mut strings: &[ArcIntern<str>], line_width: usize) -> Vec<String> {
    let mut out = Vec::new();

    while let Some((i, s)) = strings.iter().find_position(|v| v.len() >= line_width) {
        let (before, after_inclusive) = strings.split_at(i);
        strings = &after_inclusive[1..];

        out.extend(split_all_short_enough(before, line_width));
        out.push((**s).to_owned());
    }

    out.extend(split_all_short_enough(strings, line_width));

    out
}

fn split_all_short_enough(strings: &[ArcIntern<str>], max_line_width: usize) -> Vec<String> {
    if strings.is_empty() {
        return Vec::new();
    }

    // Allow calculating line lengths in constant time
    let mut cumulative = vec![0];

    let mut total = 0;
    for string in strings {
        total += string.len();
        cumulative.push(total);
    }

    let line_length = |start: usize, end: usize| {
        let num_spaces = (end - start).saturating_sub(1);
        cumulative[end] - cumulative[start] + num_spaces
    };

    // First indices represent number of splits, second indices represent length of the prefix, items store a tuple of (size of last partition, length of longest line)
    // Any positions where i >= j are meaningless
    let mut dp = vec![
        (0..=strings.len())
            .map(|i| line_length(0, i))
            .take_while(|v| *v <= max_line_width)
            .enumerate()
            .collect_vec(),
    ];

    while dp.last().unwrap().len() != strings.len() + 1 {
        let mut next_row = Vec::new();

        for _ in 0..=dp.len() {
            next_row.push((0, 0));
        }

        for i in dp.len() + 1..=strings.len() {
            let mut optimal: Option<(usize, usize)> = None;

            for j in 1..=i - dp.len() {
                let line_length = line_length(i - j, i);

                if line_length > max_line_width {
                    break;
                }
                let Some((_, max_len)) = dp.last().unwrap().get(i - j) else {
                    continue;
                };

                let max_len = (*max_len).max(line_length);

                if optimal.is_none_or(|(_, min)| min > max_len) {
                    optimal = Some((j, max_len));
                }
            }

            if let Some(item) = optimal {
                next_row.push(item);
            } else {
                break;
            }
        }

        dp.push(next_row);
    }

    let mut out = Vec::new();

    let mut not_taken = strings;

    for i in (0..dp.len()).rev() {
        let (amt_taken, _) = dp[i][not_taken.len()];
        let (before, after) = not_taken.split_at(not_taken.len() - amt_taken);
        out.push(after.iter().map(|v| &**v).join(" "));
        not_taken = before;
    }

    out.reverse();
    out
}

fn stringify_facelets(facelets: &Facelets) -> String {
    facelets.pieces().iter().map(|v| &**v).join(" ")
}

fn convert_instructions(instructions: &[WithSpan<Instruction>]) -> Vec<WithSpan<QInstruction>> {
    let mut out = Vec::new();

    for instr in instructions {
        let span = instr.span().clone();
        out.push(span.with(match &**instr {
            Instruction::Goto { instruction_idx } => QInstruction::Goto {
                instruction_idx: *instruction_idx,
            },
            Instruction::SolvedGoto(v) => QInstruction::SolvedGoto(v.unwrap_puzzle().clone()),
            Instruction::Input(v) => QInstruction::Input(v.unwrap_puzzle().clone()),
            Instruction::Halt(v) => QInstruction::Halt(v.unwrap_puzzle().clone()),
            Instruction::Print(v) => QInstruction::Print(v.unwrap_puzzle().clone()),
            Instruction::PerformAlgorithm(v) => {
                QInstruction::PerformAlgorithm(v.unwrap_puzzle().clone())
            }
            Instruction::Solve(v) => QInstruction::Solve(*v.unwrap_puzzle()),
            Instruction::RepeatUntil(v) => QInstruction::RepeatUntil(v.unwrap_puzzle().clone()),
        }));
    }

    out
}

#[cfg(test)]
mod tests {
    use internment::ArcIntern;

    use crate::q_emitter::split_strings;

    #[test]
    fn test_split_strings() {
        assert_eq!(
            split_strings(&["A", "B", "C", "D"].map(ArcIntern::from), 5),
            vec!["A B", "C D"]
        );

        assert_eq!(
            split_strings(&["A", "B", "C", "D"].map(ArcIntern::from), 2),
            vec!["A", "B", "C", "D"]
        );

        assert_eq!(
            split_strings(&["A", "BRUH", "C", "D"].map(ArcIntern::from), 3),
            vec!["A", "BRUH", "C D"]
        );
    }
}

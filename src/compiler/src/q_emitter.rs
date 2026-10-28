use ariadne::{Report, ReportKind};
use itertools::Itertools;
use puzzle_theory::{
    numbers::{Int, U},
    permutations::Algorithm,
};
use qter_core::span::{File, Span};
use qter_core::{Decoding, Facelets, Instruction, Program};
use sarlacc::Intern;
use std::fmt::Write;

use crate::Reporter;

const ALG_MAX_CHARS_WIDTH: usize = 50;

/// Convert a `Program` into Q code. The file name will be inserted into all of the spans. Also returns a list of spans for each instruction in the program.
///
/// # Errors
///
/// Returns `None` and pushes errors to the reporter if there are multiple puzzles.
#[allow(clippy::missing_panics_doc)]
#[must_use]
pub fn emit_q(
    program: &Program,
    file_name: Intern<str>,
    r: &Reporter,
) -> Option<(File, Box<[Span]>)> {
    let before = r.count();

    if program.puzzles.len() > 1 {
        r.push(
            Report::build(ReportKind::Error, program.puzzles[1].span())
                .with_message("Compiling with multiple puzzles is unsupported (for now)")
                .finish(),
        );
    }

    if r.count() - before != 0 {
        return None;
    }

    let mut out = String::new();

    out.push_str("Puzzles\n");
    if let Some(puzzle) = program.puzzles.first() {
        writeln!(&mut out, "A: {}", puzzle.span().slice()).unwrap();
    }
    out.push('\n');

    let instrs = &program.instructions;

    let digits = (instrs.len().max(2) - 1).ilog10() as usize + 1;

    let padding = " ".repeat(digits + 3);

    let mut spans = Vec::new();

    for (i, instr) in instrs.iter().enumerate() {
        let mut num = i.to_string();

        while num.len() < digits {
            num.push(' ');
        }

        let instr = match &**instr {
            Instruction::Goto { instruction_idx } => format!("goto {instruction_idx}"),
            Instruction::SolvedGoto {
                instruction_idx,
                puzzle_idx: _,
                facelets,
            } => {
                format!(
                    "solved-goto {} {}",
                    facelets.pieces().iter().map(|v| &**v).join(" "),
                    instruction_idx
                )
            }
            Instruction::Input { message, decoding: encoding } => {
                format!(
                    "input \"{}\"\n{}\n{padding}      max-input {}",
                    message,
                    stringify_alg(&encoding.algorithm, padding.len() + 6, true),
                    encoding.facelets.order() - Int::<U>::one(),
                )
            }
            Instruction::Halt {
                message,
                maybe_reg: None,
            } => {
                format!("halt \"{message}\"")
            }
            Instruction::Halt {
                message,
                maybe_reg:
                    Some(Decoding {
                        puzzle_idx: _,
                        facelets,
                        algorithm,
                    }),
            } => {
                let mut inverse_alg = algorithm.clone();
                inverse_alg.exponentiate(-Int::<U>::one());
                format!(
                    "halt \"{}\"\n{}\n{padding}     counting-until {}",
                    message,
                    stringify_alg(&inverse_alg, padding.len() + 5, true),
                    stringify_facelets(facelets),
                )
            }
            Instruction::Print {
                message,
                maybe_reg: None,
            } => {
                format!("print \"{message}\"")
            }
            Instruction::Print {
                message,
                maybe_reg:
                    Some(Decoding {
                        puzzle_idx: _,
                        facelets,
                        algorithm,
                    }),
            } => {
                let mut inverse_alg = algorithm.clone();
                inverse_alg.exponentiate(-Int::<U>::one());
                format!(
                    "print \"{}\"\n{}\n{padding}      counting-until {}",
                    message,
                    stringify_alg(&inverse_alg, padding.len() + 6, true),
                    stringify_facelets(facelets),
                )
            }
            Instruction::PerformAlgorithm{ puzzle: _, alg } => stringify_alg(alg, padding.len(), false),
            Instruction::Solve(_) => "solve".to_string(),
            Instruction::RepeatUntil { puzzle_idx: _, facelets, alg } => {
                format!(
                    "repeat until {} solved\n{}",
                    stringify_facelets(facelets),
                    stringify_alg(alg, padding.len() + 7, true)
                )
            }
        };

        let start = out.chars().count();
        writeln!(&mut out, "{num} | {instr}").unwrap();
        let end = out.chars().count();

        spans.push((start, end));
    }

    assert_eq!(spans.len(), program.instructions.len());

    let file = File::new(file_name, Intern::from_owned(out));

    let spans = spans
        .into_iter()
        .map(|(start, end)| Span::new(file, start, end))
        .collect();

    Some((file, spans))
}

fn stringify_alg(alg: &Algorithm, padding: usize, pad_first: bool) -> String {
    let padding_str = " ".repeat(padding);
    split_strings(
        &alg.move_seq_iter().collect::<Vec<_>>(),
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
fn split_strings(mut strings: &[Intern<str>], line_width: usize) -> Vec<String> {
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

fn split_all_short_enough(strings: &[Intern<str>], max_line_width: usize) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use sarlacc::Intern;

    use crate::q_emitter::split_strings;

    #[test]
    fn test_split_strings() {
        assert_eq!(
            split_strings(&["A", "B", "C", "D"].map(Intern::from_ref), 5),
            vec!["A B", "C D"]
        );

        assert_eq!(
            split_strings(&["A", "B", "C", "D"].map(Intern::from_ref), 2),
            vec!["A", "B", "C", "D"]
        );

        assert_eq!(
            split_strings(&["A", "BRUH", "C", "D"].map(Intern::from_ref), 3),
            vec!["A", "BRUH", "C D"]
        );
    }
}

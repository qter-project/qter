use ariadne::{Report, ReportKind};
use internment::ArcIntern;
use puzzle_theory::span::{File, Span, WithSpan};

use crate::{
    BlockID, ExpansionInfo, LabelReference, Macro, Primitive, RegisterReference, Reporter,
    ResolvedValue, Value,
};

use std::collections::HashMap;

fn expect_reg(
    reg_value: &WithSpan<Value>,
    block_id: BlockID,
    syntax: &ExpansionInfo,
    r: &Reporter,
) -> Option<RegisterReference> {
    match syntax.block_info.resolve_ref(block_id, reg_value) {
        Some(value) => match value.as_reg(syntax) {
            Some(Ok(reg)) => Some(reg.0.clone()),
            Some(Err(reg_name)) => {
                r.push(
                    Report::build(ReportKind::Error, reg_value.span().clone())
                        .with_message(format!("The register {} does not exist", &**reg_name))
                        .finish(),
                );
                None
            }
            None => {
                r.push(
                    Report::build(ReportKind::Error, reg_value.span().clone())
                        .with_message("Expected a register")
                        .finish(),
                );
                None
            }
        },
        None => {
            r.push(
                Report::build(ReportKind::Error, reg_value.span().clone())
                    .with_message("Constant not found in this scope")
                    .finish(),
            );
            None
        }
    }
}

fn expect_label(
    label_value: &WithSpan<Value>,
    block_id: BlockID,
    syntax: &ExpansionInfo,
    r: &Reporter,
) -> Option<WithSpan<LabelReference>> {
    match syntax.block_info.resolve_ref(block_id, label_value) {
        Some(ResolvedValue::Ident { ident, as_reg: _ }) => Some(WithSpan::new(
            LabelReference {
                name: ArcIntern::clone(ident),
                block_id,
                branch_key: None,
            },
            label_value.span().to_owned(),
        )),
        Some(_) => {
            r.push(
                Report::build(ReportKind::Error, label_value.span().clone())
                    .with_message("Expected a label")
                    .finish(),
            );
            None
        }
        None => {
            r.push(
                Report::build(ReportKind::Error, label_value.span().clone())
                    .with_message("Constant not found in this scope")
                    .finish(),
            );
            None
        }
    }
}

fn print_like(
    syntax: &ExpansionInfo,
    mut args: WithSpan<Vec<WithSpan<Value>>>,
    block_id: BlockID,
    r: &Reporter,
) -> Option<(Option<RegisterReference>, WithSpan<String>)> {
    if args.len() > 2 || args.is_empty() {
        r.push(
            Report::build(ReportKind::Error, args.span().clone())
                .with_message(format!(
                    "Expected one or two arguments, found {}",
                    args.len()
                ))
                .finish(),
        );
        return None;
    }

    let maybe_reg = if args.len() == 2 {
        Some(expect_reg(
            args.pop().as_ref().unwrap(),
            block_id,
            syntax,
            r,
        )?)
    } else {
        None
    };

    let message = args.pop().unwrap();
    let span = message.span().to_owned();
    let message = match syntax.block_info.resolve(block_id, message.into_inner()) {
        Some(ResolvedValue::Ident { ident, as_reg: _ }) => {
            WithSpan::new((**ident).to_owned(), span)
        }
        Some(_) => {
            r.push(
                Report::build(ReportKind::Error, span.clone())
                    .with_message("Expected a message")
                    .finish(),
            );
            return None;
        }
        None => {
            r.push(
                Report::build(ReportKind::Error, span.clone())
                    .with_message("Constant not found in this scope")
                    .finish(),
            );
            return None;
        }
    };

    Some((maybe_reg, message))
}

pub fn builtin_macros(prelude: &File) -> HashMap<(File, ArcIntern<str>), WithSpan<Macro>> {
    let mut macros = HashMap::new();

    let dummy_span = Span::new(
        File::new(ArcIntern::from("BUILTIN"), ArcIntern::from(" ")),
        0,
        0,
    );

    macros.insert(
        (prelude.clone(), ArcIntern::from("add")),
        WithSpan::new(
            Macro::Builtin(|syntax, mut args, block_id, r| {
                if args.len() != 2 {
                    r.push(
                        Report::build(ReportKind::Error, args.span().clone())
                            .with_message(format!("Expected two arguments, found {}", args.len()))
                            .finish(),
                    );
                    return None;
                }

                let second_arg = args.pop().unwrap();
                let span = second_arg.span().clone();
                let amt = match syntax.block_info.resolve(block_id, second_arg.into_inner()) {
                    Some(ResolvedValue::Int(int)) => WithSpan::new(int, span),
                    Some(_) => {
                        r.push(
                            Report::build(ReportKind::Error, span)
                                .with_message("Expected a number")
                                .finish(),
                        );
                        return None;
                    }
                    None => {
                        r.push(
                            Report::build(ReportKind::Error, span)
                                .with_message("Constant not found in this scope")
                                .finish(),
                        );
                        return None;
                    }
                };

                let register = expect_reg(args.pop().as_ref().unwrap(), block_id, syntax, r)?;

                Some(Primitive::Add { amt, register })
            }),
            dummy_span.clone(),
        ),
    );

    macros.insert(
        (prelude.to_owned(), ArcIntern::from("goto")),
        WithSpan::new(
            Macro::Builtin(|syntax, mut args, block_id, r| {
                if args.len() != 1 {
                    r.push(
                        Report::build(ReportKind::Error, args.span().clone())
                            .with_message(format!("Expected one argument, found {}", args.len()))
                            .finish(),
                    );
                    return None;
                }

                let label = expect_label(args.pop().as_ref().unwrap(), block_id, syntax, r)?;

                Some(Primitive::Goto { label })
            }),
            dummy_span.clone(),
        ),
    );

    macros.insert(
        (prelude.to_owned(), ArcIntern::from("solved-goto")),
        WithSpan::new(
            Macro::Builtin(|syntax, mut args, block_id, r| {
                if args.len() != 2 {
                    r.push(
                        Report::build(ReportKind::Error, args.span().clone())
                            .with_message(format!("Expected two arguments, found {}", args.len()))
                            .finish(),
                    );
                    return None;
                }

                let label = expect_label(args.pop().as_ref().unwrap(), block_id, syntax, r)?;
                let register = expect_reg(args.pop().as_ref().unwrap(), block_id, syntax, r)?;

                Some(Primitive::SolvedGoto { register, label })
            }),
            dummy_span.clone(),
        ),
    );

    macros.insert(
        (prelude.to_owned(), ArcIntern::from("input")),
        WithSpan::new(
            Macro::Builtin(|syntax, mut args, block_id, r| {
                if args.len() != 2 {
                    r.push(
                        Report::build(ReportKind::Error, args.span().clone())
                            .with_message(format!("Expected two arguments, found {}", args.len()))
                            .finish(),
                    );
                    return None;
                }

                let register = expect_reg(args.pop().as_ref().unwrap(), block_id, syntax, r)?;

                let second_arg = args.pop().unwrap();
                let span = second_arg.span().to_owned();
                let message = match syntax.block_info.resolve(block_id, second_arg.into_inner()) {
                    Some(ResolvedValue::Ident { ident, as_reg: _ }) => {
                        WithSpan::new(ident.trim_matches('"').to_owned(), span)
                    }
                    Some(_) => {
                        r.push(
                            Report::build(ReportKind::Error, span)
                                .with_message("Expected a message")
                                .finish(),
                        );
                        return None;
                    }
                    None => {
                        r.push(
                            Report::build(ReportKind::Error, span)
                                .with_message("Constant not found in this scope")
                                .finish(),
                        );
                        return None;
                    }
                };

                Some(Primitive::Input { register, message })
            }),
            dummy_span.clone(),
        ),
    );

    macros.insert(
        (prelude.to_owned(), ArcIntern::from("halt")),
        WithSpan::new(
            Macro::Builtin(|syntax, args, block_id, r| {
                let (register, message) = print_like(syntax, args, block_id, r)?;

                Some(Primitive::Halt { register, message })
            }),
            dummy_span.clone(),
        ),
    );

    macros.insert(
        (prelude.to_owned(), ArcIntern::from("print")),
        WithSpan::new(
            Macro::Builtin(|syntax, args, block_id, r| {
                let (register, message) = print_like(syntax, args, block_id, r)?;

                Some(Primitive::Print { register, message })
            }),
            dummy_span.clone(),
        ),
    );

    macros
}

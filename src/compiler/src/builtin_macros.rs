use ariadne::{Label, Report, ReportKind};
use internment::ArcIntern;
use puzzle_theory::span::{File, Span, WithSpan};

use crate::{
    BlockID, ExpansionInfo, LabelReference, Macro, MaybeReg, Primitive, RegisterReference,
    Reporter, ResolvedValue, Value,
};

use std::collections::HashMap;

fn expect_reg(
    reg_value: WithSpan<Value>,
    block_id: BlockID,
    syntax: &ExpansionInfo,
    r: &Reporter,
) -> Option<RegisterReference> {
    let span = reg_value.span().clone();

    let value = syntax.block_info.resolve(block_id, reg_value)?;

    match value.as_reg(syntax) {
        MaybeReg::Reg(reg, _) => Some(reg.clone()),
        MaybeReg::RegNotFound(reg_name) => {
            r.push(
                Report::build(ReportKind::Error, span.clone())
                    .with_label(
                        Label::new(span.clone())
                            .with_message(format!("The register {reg_name} does not exist")),
                    )
                    .finish(),
            );
            None
        }
        MaybeReg::NotAReg(actually) => {
            r.push(
                Report::build(ReportKind::Error, span.clone())
                    .with_label(
                        Label::new(span.clone())
                            .with_message(format!("Expected a register, found {actually}")),
                    )
                    .finish(),
            );
            None
        }
    }
}

fn expect_label(
    label_value: WithSpan<Value>,
    block_id: BlockID,
    syntax: &ExpansionInfo,
    r: &Reporter,
) -> Option<WithSpan<LabelReference>> {
    let span = label_value.span().clone();
    match syntax.block_info.resolve(block_id, label_value)?.into_inner() {
        ResolvedValue::Ident(name) => Some(WithSpan::new(
            LabelReference {
                name: name.into_inner(),
                block_id,
                branch_key: None,
            },
            span,
        )),
        _ => {
            r.push(
                Report::build(ReportKind::Error, span)
                    .with_message("Expected a label")
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
            args.pop().unwrap(),
            block_id,
            syntax,
            r,
        )?)
    } else {
        None
    };

    let message = args.pop().unwrap();
    let span = message.span().to_owned();
    let message = match syntax.block_info.resolve(block_id, message)?.into_inner() {
        ResolvedValue::Ident(ident) => WithSpan::new((**ident).to_owned(), span),
        _ => {
            r.push(
                Report::build(ReportKind::Error, span.clone())
                    .with_message("Expected a message")
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
        1,
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
                let amt = match syntax
                    .block_info
                    .resolve(block_id, second_arg)?
                    .into_inner()
                {
                    ResolvedValue::Int(int) => WithSpan::new(int, span),
                    _ => {
                        r.push(
                            Report::build(ReportKind::Error, span)
                                .with_message("Expected a number")
                                .finish(),
                        );
                        return None;
                    }
                };

                let register = expect_reg(args.pop().unwrap(), block_id, syntax, r)?;

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

                let label = expect_label(args.pop().unwrap(), block_id, syntax, r)?;

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

                let label = expect_label(args.pop().unwrap(), block_id, syntax, r)?;
                let register = expect_reg(args.pop().unwrap(), block_id, syntax, r)?;

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

                let register = expect_reg(args.pop().unwrap(), block_id, syntax, r)?;

                let second_arg = args.pop().unwrap();
                let span = second_arg.span().to_owned();
                let message = match syntax
                    .block_info
                    .resolve(block_id, second_arg)?
                    .into_inner()
                {
                    ResolvedValue::Ident(ident) => {
                        WithSpan::new(ident.trim_matches('"').to_owned(), span)
                    }
                    _ => {
                        r.push(
                            Report::build(ReportKind::Error, span)
                                .with_message("Expected a message")
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

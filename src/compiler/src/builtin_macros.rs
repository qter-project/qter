use ariadne::{Label, Report, ReportKind};
use qter_core::span::{File, Span, WithSpan};
use sarlacc::Intern;

use crate::{
    BlockID, ExpansionInfo, LabelReference, Macro, MacroBranchKey, MaybeReg, Primitive,
    RegisterReference, Reporter, ResolvedValue, Value, reporter::OrErrorProof,
};

use std::collections::HashMap;

fn expect_reg(
    reg_value: WithSpan<Value>,
    block_id: BlockID,
    syntax: &ExpansionInfo,
    r: &Reporter,
) -> OrErrorProof<RegisterReference> {
    let span = reg_value.span();

    let value = syntax.block_info.resolve(block_id, reg_value)?;

    match value.as_reg(syntax) {
        MaybeReg::Reg(reg, _) => Ok(reg),
        MaybeReg::RegNotFound(reg_name) => {
            Err(r.push(
                Report::build(ReportKind::Error, span)
                    .with_label(
                        Label::new(span)
                            .with_message(format!("The register {reg_name} does not exist")),
                    )
                    .finish(),
            ))
        }
        MaybeReg::NotAReg(actually) => {
            Err(r.push(
                Report::build(ReportKind::Error, span)
                    .with_label(
                        Label::new(span)
                            .with_message(format!("Expected a register, found {actually}")),
                    )
                    .finish(),
            ))
        }
    }
}

fn expect_label(
    label_value: WithSpan<Value>,
    block_id: BlockID,
    syntax: &ExpansionInfo,
    branch_key: MacroBranchKey,
    r: &Reporter,
) -> OrErrorProof<WithSpan<LabelReference>> {
    let span = label_value.span();
    match syntax
        .block_info
        .resolve(block_id, label_value)?
        .into_inner()
    {
        ResolvedValue::Ident(name) => Ok(WithSpan::new(
            LabelReference {
                name: name.into_inner(),
                block_id,
                branch_key,
            },
            span,
        )),
        _ => {
            Err(r.push(
                Report::build(ReportKind::Error, span)
                    .with_label(Label::new(span).with_message("Expected a label"))
                    .finish(),
            ))
        }
    }
}

fn print_like(
    syntax: &ExpansionInfo,
    mut args: WithSpan<Vec<WithSpan<Value>>>,
    block_id: BlockID,
    r: &Reporter,
) -> OrErrorProof<(Option<RegisterReference>, WithSpan<String>)> {
    if args.len() > 2 || args.is_empty() {
        return Err(r.push(
            Report::build(ReportKind::Error, args.span())
                .with_label(Label::new(args.span()).with_message(format!(
                    "Expected one or two arguments, found {}",
                    args.len()
                )))
                .finish(),
        ));
    }

    let maybe_reg = if args.len() == 2 {
        Some(expect_reg(args.pop().unwrap(), block_id, syntax, r)?)
    } else {
        None
    };

    let message = args.pop().unwrap();
    let span = message.span();
    let message = match syntax.block_info.resolve(block_id, message)?.into_inner() {
        ResolvedValue::Ident(ident) => WithSpan::new((**ident).to_owned(), span),
        _ => {
            return Err(r.push(
                Report::build(ReportKind::Error, span)
                    .with_label(Label::new(span).with_message("Expected a message"))
                    .finish(),
            ));
        }
    };

    Ok((maybe_reg, message))
}

pub fn builtin_macros(prelude: File) -> HashMap<(File, Intern<str>), WithSpan<Macro>> {
    let mut macros = HashMap::new();

    let dummy_span = Span::new(
        File::new(Intern::from_ref("BUILTIN"), Intern::from_ref(" ")),
        0,
        1,
    );

    macros.insert(
        (prelude, Intern::from_ref("add")),
        WithSpan::new(
            Macro::Builtin(|syntax, mut args, block_id, _, r| {
                if args.len() != 2 {
                    return Err(r.push(
                        Report::build(ReportKind::Error, args.span())
                            .with_label(Label::new(args.span()).with_message(format!(
                                "Expected two arguments, found {}",
                                args.len()
                            )))
                            .finish(),
                    ));
                }

                let second_arg = args.pop().unwrap();
                let span = second_arg.span();
                let amt = match syntax
                    .block_info
                    .resolve(block_id, second_arg)?
                    .into_inner()
                {
                    ResolvedValue::Int(int) => WithSpan::new(int, span),
                    _ => {
                        return Err(r.push(
                            Report::build(ReportKind::Error, span)
                                .with_label(Label::new(span).with_message("Expected a number"))
                                .finish(),
                        ));
                    }
                };

                let register = expect_reg(args.pop().unwrap(), block_id, syntax, r)?;

                Ok(Primitive::Add { amt, register })
            }),
            dummy_span,
        ),
    );

    macros.insert(
        (prelude, Intern::from_ref("goto")),
        WithSpan::new(
            Macro::Builtin(|syntax, mut args, block_id, branch_key, r| {
                if args.len() != 1 {
                    return Err(r.push(
                        Report::build(ReportKind::Error, args.span())
                            .with_label(Label::new(args.span()).with_message(format!("Expected one argument, found {}", args.len())))
                            .finish(),
                    ));
                }

                let label = expect_label(args.pop().unwrap(), block_id, syntax, branch_key, r)?;

                Ok(Primitive::Goto { label })
            }),
            dummy_span,
        ),
    );

    macros.insert(
        (prelude, Intern::from_ref("solved-goto")),
        WithSpan::new(
            Macro::Builtin(|syntax, mut args, block_id, branch_key, r| {
                if args.len() != 2 {
                    return Err(r.push(
                        Report::build(ReportKind::Error, args.span())
                            .with_label(Label::new(args.span()).with_message(format!("Expected two arguments, found {}", args.len())))
                            .finish(),
                    ));
                }

                let label = expect_label(args.pop().unwrap(), block_id, syntax, branch_key, r)?;
                let register = expect_reg(args.pop().unwrap(), block_id, syntax, r)?;

                Ok(Primitive::SolvedGoto { register, label })
            }),
            dummy_span,
        ),
    );

    macros.insert(
        (prelude, Intern::from_ref("input")),
        WithSpan::new(
            Macro::Builtin(|syntax, mut args, block_id, _, r| {
                if args.len() != 2 {
                    return Err(r.push(
                        Report::build(ReportKind::Error, args.span())
                            .with_label(Label::new(args.span()).with_message(format!("Expected two arguments, found {}", args.len())))
                            .finish(),
                    ));
                }

                let register = expect_reg(args.pop().unwrap(), block_id, syntax, r)?;

                let second_arg = args.pop().unwrap();
                let span = second_arg.span();
                let message = match syntax
                    .block_info
                    .resolve(block_id, second_arg)?
                    .into_inner()
                {
                    ResolvedValue::Ident(ident) => {
                        WithSpan::new(ident.trim_matches('"').to_owned(), span)
                    }
                    _ => {
                        return Err(r.push(
                            Report::build(ReportKind::Error, span)
                                .with_label(Label::new(span).with_message("Expected a message"))
                                .finish(),
                        ));
                    }
                };

                Ok(Primitive::Input { register, message })
            }),
            dummy_span,
        ),
    );

    macros.insert(
        (prelude, Intern::from_ref("halt")),
        WithSpan::new(
            Macro::Builtin(|syntax, args, block_id, _, r| {
                let (register, message) = print_like(syntax, args, block_id, r)?;

                Ok(Primitive::Halt { register, message })
            }),
            dummy_span,
        ),
    );

    macros.insert(
        (prelude, Intern::from_ref("print")),
        WithSpan::new(
            Macro::Builtin(|syntax, args, block_id, _, r| {
                let (register, message) = print_like(syntax, args, block_id, r)?;

                Ok(Primitive::Print { register, message })
            }),
            dummy_span,
        ),
    );

    macros
}

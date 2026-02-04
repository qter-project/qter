use chumsky::error::Rich;
use internment::ArcIntern;
use puzzle_theory::span::{File, Span, WithSpan};

use crate::{
    BlockID, Code, ExpansionInfo, Instruction, LabelReference, Macro, Primitive, RegisterReference,
    ResolvedValue, Value,
};

use std::collections::HashMap;

fn expect_reg(
    reg_value: &WithSpan<Value>,
    block_id: BlockID,
    syntax: &ExpansionInfo,
) -> Result<RegisterReference, Rich<'static, char, Span>> {
    match syntax.block_info.resolve_ref(block_id, reg_value) {
        Some(value) => match value.as_reg(syntax) {
            Some(Ok(reg)) => Ok(reg.0.clone()),
            Some(Err(reg_name)) => Err(Rich::custom(
                reg_value.span().clone(),
                format!("The register {} does not exist", &**reg_name),
            )),
            None => Err(Rich::custom(
                reg_value.span().clone(),
                "Expected a register",
            )),
        },
        None => Err(Rich::custom(
            reg_value.span().clone(),
            "Constant not found in this scope",
        )),
    }
}

fn expect_label(
    label_value: &WithSpan<Value>,
    block_id: BlockID,
    syntax: &ExpansionInfo,
) -> Result<WithSpan<LabelReference>, Rich<'static, char, Span>> {
    match syntax.block_info.resolve_ref(block_id, label_value) {
        Some(ResolvedValue::Ident { ident, as_reg: _ }) => Ok(WithSpan::new(
            LabelReference {
                name: ArcIntern::clone(ident),
                block_id,
            },
            label_value.span().to_owned(),
        )),
        Some(_) => Err(Rich::custom(label_value.span().clone(), "Expected a label")),
        None => Err(Rich::custom(
            label_value.span().clone(),
            "Constant not found in this scope",
        )),
    }
}

fn print_like(
    syntax: &ExpansionInfo,
    mut args: WithSpan<Vec<WithSpan<Value>>>,
    block_id: BlockID,
) -> Result<(Option<RegisterReference>, WithSpan<String>), Rich<'static, char, Span>> {
    if args.len() > 2 {
        return Err(Rich::custom(
            args.span().clone(),
            format!("Expected one or two arguments, found {}", args.len()),
        ));
    }

    let maybe_reg = if args.len() == 2 {
        Some(expect_reg(args.pop().as_ref().unwrap(), block_id, syntax)?)
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
            return Err(Rich::custom(span, "Expected a message"));
        }
        None => {
            return Err(Rich::custom(span, "Constant not found in this scope"));
        }
    };

    Ok((maybe_reg, message))
}

pub fn builtin_macros(
    prelude: &File,
) -> HashMap<(File, ArcIntern<str>), WithSpan<Macro>> {
    let mut macros = HashMap::new();

    let dummy_span = Span::new(File::new(ArcIntern::from("BUILTIN"), ArcIntern::from(" ")), 0, 0);

    macros.insert(
        (prelude.clone(), ArcIntern::from("add")),
        WithSpan::new(
            Macro::Builtin(|syntax, mut args, block_id| {
                if args.len() != 2 {
                    return Err(Rich::custom(
                        args.span().clone(),
                        format!("Expected two arguments, found {}", args.len()),
                    ));
                }

                let second_arg = args.pop().unwrap();
                let span = second_arg.span().clone();
                let amt = match syntax.block_info.resolve(block_id, second_arg.into_inner()) {
                    Some(ResolvedValue::Int(int)) => WithSpan::new(int, span),
                    Some(_) => {
                        return Err(Rich::custom(span, "Expected a number"));
                    }
                    None => {
                        return Err(Rich::custom(span, "Constant not found in this scope"));
                    }
                };

                let register = expect_reg(args.pop().as_ref().unwrap(), block_id, syntax)?;

                Ok(vec![Instruction::Code(Code::Primitive(Primitive::Add {
                    amt,
                    register,
                }))])
            }),
            dummy_span.clone(),
        ),
    );

    macros.insert(
        (prelude.to_owned(), ArcIntern::from("goto")),
        WithSpan::new(
            Macro::Builtin(|syntax, mut args, block_id| {
                if args.len() != 1 {
                    return Err(Rich::custom(
                        args.span().clone(),
                        format!("Expected one argument, found {}", args.len()),
                    ));
                }

                let label = expect_label(args.pop().as_ref().unwrap(), block_id, syntax)?;

                Ok(vec![Instruction::Code(Code::Primitive(Primitive::Goto {
                    label,
                }))])
            }),
            dummy_span.clone(),
        ),
    );

    macros.insert(
        (prelude.to_owned(), ArcIntern::from("solved-goto")),
        WithSpan::new(
            Macro::Builtin(|syntax, mut args, block_id| {
                if args.len() != 2 {
                    return Err(Rich::custom(
                        args.span().clone(),
                        format!("Expected two arguments, found {}", args.len()),
                    ));
                }

                let label = expect_label(args.pop().as_ref().unwrap(), block_id, syntax)?;
                let register = expect_reg(args.pop().as_ref().unwrap(), block_id, syntax)?;

                Ok(vec![Instruction::Code(Code::Primitive(
                    Primitive::SolvedGoto { register, label },
                ))])
            }),
            dummy_span.clone(),
        ),
    );

    macros.insert(
        (prelude.to_owned(), ArcIntern::from("input")),
        WithSpan::new(
            Macro::Builtin(|syntax, mut args, block_id| {
                if args.len() != 2 {
                    return Err(Rich::custom(
                        args.span().clone(),
                        format!("Expected two arguments, found {}", args.len()),
                    ));
                }

                let register = expect_reg(args.pop().as_ref().unwrap(), block_id, syntax)?;

                let second_arg = args.pop().unwrap();
                let span = second_arg.span().to_owned();
                let message = match syntax.block_info.resolve(block_id, second_arg.into_inner()) {
                    Some(ResolvedValue::Ident { ident, as_reg: _ }) => {
                        WithSpan::new(ident.trim_matches('"').to_owned(), span)
                    }
                    Some(_) => {
                        return Err(Rich::custom(span, "Expected a message"));
                    }
                    None => {
                        return Err(Rich::custom(span, "Constant not found in this scope"));
                    }
                };

                Ok(vec![Instruction::Code(Code::Primitive(Primitive::Input {
                    register,
                    message,
                }))])
            }),
            dummy_span.clone(),
        ),
    );

    macros.insert(
        (prelude.to_owned(), ArcIntern::from("halt")),
        WithSpan::new(
            Macro::Builtin(|syntax, args, block_id| {
                let (register, message) = print_like(syntax, args, block_id)?;

                Ok(vec![Instruction::Code(Code::Primitive(Primitive::Halt {
                    register,
                    message,
                }))])
            }),
            dummy_span.clone(),
        ),
    );

    macros.insert(
        (prelude.to_owned(), ArcIntern::from("print")),
        WithSpan::new(
            Macro::Builtin(|syntax, args, block_id| {
                let (register, message) = print_like(syntax, args, block_id)?;

                Ok(vec![Instruction::Code(Code::Primitive(Primitive::Print {
                    register,
                    message,
                }))])
            }),
            dummy_span.clone(),
        ),
    );

    macros
}

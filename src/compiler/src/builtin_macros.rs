use chumsky::error::Rich;
use internment::ArcIntern;
use puzzle_theory::span::{Span, WithSpan};

use crate::{
    BlockID, Code, ExpansionInfo, Instruction, LabelReference, Macro, Primitive, RegisterReference,
    Value,
};

use std::collections::HashMap;

fn expect_reg(
    reg_value: &WithSpan<Value>,
    syntax: &ExpansionInfo,
) -> Result<RegisterReference, Rich<'static, char, Span>> {
    match &**reg_value {
        Value::Ident(reg_name) => match syntax.get_register(
            &RegisterReference::parse(WithSpan::new(
                ArcIntern::clone(reg_name),
                reg_value.span().to_owned(),
            ))
            .map_err(|e| {
                Rich::custom(
                    reg_value.span().clone(),
                    format!("Could not parse the modulus as a string: {e}"),
                )
            })?,
        ) {
            Some((reg, _)) => Ok(reg),
            None => Err(Rich::custom(
                reg_value.span().clone(),
                format!("The register {reg_name} does not exist"),
            )),
        },
        _ => Err(Rich::custom(
            reg_value.span().clone(),
            "Expected a register",
        )),
    }
}

fn expect_label(
    label_value: &WithSpan<Value>,
    block_id: BlockID,
) -> Result<WithSpan<LabelReference>, Rich<'static, char, Span>> {
    match &**label_value {
        Value::Ident(label_name) => Ok(WithSpan::new(
            LabelReference {
                name: ArcIntern::clone(label_name),
                block_id,
            },
            label_value.span().to_owned(),
        )),
        _ => Err(Rich::custom(label_value.span().clone(), "Expected a label")),
    }
}

fn print_like(
    syntax: &ExpansionInfo,
    mut args: WithSpan<Vec<WithSpan<Value>>>,
) -> Result<(Option<RegisterReference>, WithSpan<String>), Rich<'static, char, Span>> {
    if args.len() > 2 {
        return Err(Rich::custom(
            args.span().clone(),
            format!("Expected one or two arguments, found {}", args.len()),
        ));
    }

    let maybe_reg = if args.len() == 2 {
        Some(expect_reg(args.pop().as_ref().unwrap(), syntax)?)
    } else {
        None
    };

    let message = args.pop().unwrap();
    let span = message.span().to_owned();
    let message = match message.into_inner() {
        Value::Ident(raw_message) => WithSpan::new((*raw_message).to_owned(), span),
        _ => {
            return Err(Rich::custom(span, "Expected a message"));
        }
    };

    Ok((maybe_reg, message))
}

pub fn builtin_macros(
    prelude: &ArcIntern<str>,
) -> HashMap<(ArcIntern<str>, ArcIntern<str>), WithSpan<Macro>> {
    let mut macros = HashMap::new();

    let dummy_span = Span::new(ArcIntern::from(" "), 0, 0);

    macros.insert(
        (prelude.clone(), ArcIntern::from("add")),
        WithSpan::new(
            Macro::Builtin(|syntax, mut args, _| {
                if args.len() != 2 {
                    return Err(Rich::custom(
                        args.span().clone(),
                        format!("Expected two arguments, found {}", args.len()),
                    ));
                }

                let second_arg = args.pop().unwrap();
                let amt = match *second_arg {
                    Value::Int(int) => WithSpan::new(int, second_arg.span().to_owned()),
                    _ => {
                        return Err(Rich::custom(second_arg.span().clone(), "Expected a number"));
                    }
                };

                let register = expect_reg(args.pop().as_ref().unwrap(), syntax)?;

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
            Macro::Builtin(|_syntax, mut args, block_id| {
                if args.len() != 1 {
                    return Err(Rich::custom(
                        args.span().clone(),
                        format!("Expected one argument, found {}", args.len()),
                    ));
                }

                let label = expect_label(args.pop().as_ref().unwrap(), block_id)?;

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

                let label = expect_label(args.pop().as_ref().unwrap(), block_id)?;
                let register = expect_reg(args.pop().as_ref().unwrap(), syntax)?;

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
            Macro::Builtin(|syntax, mut args, _| {
                if args.len() != 2 {
                    return Err(Rich::custom(
                        args.span().clone(),
                        format!("Expected two arguments, found {}", args.len()),
                    ));
                }

                let register = expect_reg(args.pop().as_ref().unwrap(), syntax)?;

                let second_arg = args.pop().unwrap();
                let span = second_arg.span().to_owned();
                let message = match second_arg.into_inner() {
                    Value::Ident(raw_message) => {
                        WithSpan::new(raw_message.trim_matches('"').to_owned(), span)
                    }
                    _ => {
                        return Err(Rich::custom(span, "Expected a message"));
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
            Macro::Builtin(|syntax, args, _| {
                let (register, message) = print_like(syntax, args)?;

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
            Macro::Builtin(|syntax, args, _| {
                let (register, message) = print_like(syntax, args)?;

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

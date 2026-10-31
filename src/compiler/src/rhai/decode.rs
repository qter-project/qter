use std::any::Any;

use ariadne::{Label, Report, ReportKind};
use itertools::Itertools;
use puzzle_theory::numbers::{I, Int, U};
use qter_core::span::Span;
use rhai::{Array, Dynamic, ImmutableString};
use sarlacc::Intern;

use crate::{
    Block, Code, Define, DefineValue, Instruction, MacroBranchKey, MacroCall, RegisterModifiers,
    Reporter, ResolvedValue, RhaiCall, Value,
    rhai::{Ty, WInt, WRegisterInfo},
};

pub fn from_rhai(
    value: Dynamic,
    span: Span,
    branch_key: MacroBranchKey,
    r: &Reporter,
) -> Option<Value> {
    let value = match int_from_rhai(value, span, r)? {
        Ok(int) => return Some(Value::Resolved(ResolvedValue::Int(int))),
        Err(v) => v,
    };

    let value = match value.try_cast_result::<ImmutableString>() {
        Ok(str) => {
            return Some(Value::Resolved(ResolvedValue::Ident(
                span.with(Intern::from_ref(&*str)),
            )));
        }
        Err(v) => v,
    };

    let value = match value.try_cast_result::<WRegisterInfo>() {
        Ok(WRegisterInfo(str)) => {
            return Some(Value::Resolved(ResolvedValue::Register(str.0)));
        }
        Err(v) => v,
    };

    let value = match value.try_cast_result::<rhai::Map>() {
        Ok(mut map) => {
            let ty = get_ty(&mut map, span, r)?;

            if ty != Ty::Const {
                r.push(
                    Report::build(ReportKind::Error, span)
                        .with_label(
                            Label::new(span)
                                .with_message(format!("A map of type `{ty}` cannot be a value")),
                        )
                        .finish(),
                );
                return None;
            }

            return get_const(map, span, r);
        }
        Err(v) => v,
    };

    Some(Value::Resolved(ResolvedValue::Block(try_decode_block(
        value, span, branch_key, r,
    )?)))
}

fn get_const(mut map: rhai::Map, span: Span, r: &Reporter) -> Option<Value> {
    let name = get_str(&mut map, "name", span, r)?;

    let mods = if map.contains_key("mods") {
        let mut mods = get_map(&mut map, "mods", span, r)?;

        let RegisterModifiers {
            mut modulus,
            mut negative,
        } = RegisterModifiers::default();

        if let Some(value) = mods.get("modulus") {
            let Ok(v) = int_from_rhai(value.clone(), span, r)? else {
                r.push(
                    Report::build(ReportKind::Error, span)
                        .with_label(Label::new(span).with_message(
                            "Expected the `modulus` field of register modifiers to be an integer",
                        ))
                        .finish(),
                );
                return None;
            };
            modulus = Some(v);
        }

        if mods.contains_key("negative") {
            negative = get::<bool>(&mut mods, "negative", "bool", span, r)?;
        }

        done(mods, span, r)?;

        RegisterModifiers { modulus, negative }
    } else {
        RegisterModifiers::default()
    };

    done(map, span, r)?;

    Some(Value::Constant {
        name: Intern::from_ref(&name),
        mods,
    })
}

fn int_from_rhai(value: Dynamic, span: Span, r: &Reporter) -> Option<Result<Int<U>, Dynamic>> {
    if let Ok(int) = value.as_int() {
        let Ok(v) = u64::try_from(int) else {
            r.push(
                Report::build(ReportKind::Error, span)
                    .with_message(format!("Integer values must be positive. Found {int}"))
                    .finish(),
            );
            return None;
        };

        return Some(Ok(Int::<U>::from(v)));
    }

    let value = match value.try_cast_result::<WInt>() {
        Ok(WInt(int)) => {
            if int < Int::<I>::zero() {
                r.push(
                    Report::build(ReportKind::Error, span)
                        .with_message(format!("Integer values must be positive. Found {int}"))
                        .finish(),
                );
                return None;
            }

            return Some(Ok(int.abs()));
        }
        Err(v) => v,
    };

    Some(Err(value))
}

fn get_ty(map: &mut rhai::Map, span: Span, r: &Reporter) -> Option<Ty> {
    let v = get_str(map, "ty", span, r)?;

    Some(match &*v {
        "label" => Ty::Label,
        "call" => Ty::Call,
        "const" => Ty::Const,
        "define" => Ty::Define,
        "rhai call" => Ty::RhaiCall,
        x => {
            r.push(
                Report::build(ReportKind::Error, span)
                    .with_label(
                        Label::new(span)
                            .with_message(format!("Unknown value of the `ty` field; found `{x}`")),
                    )
                    .finish(),
            );
            return None;
        }
    })
}

fn get_str(map: &mut rhai::Map, field: &str, span: Span, r: &Reporter) -> Option<ImmutableString> {
    get(map, field, "string", span, r)
}

fn get_map(map: &mut rhai::Map, field: &str, span: Span, r: &Reporter) -> Option<rhai::Map> {
    get(map, field, "map", span, r)
}

fn get<T: Any>(
    map: &mut rhai::Map,
    field: &str,
    type_name: &str,
    span: Span,
    r: &Reporter,
) -> Option<T> {
    match map.remove(field) {
        Some(v) => {
            match v.clone().try_cast::<T>() {
                Some(s) => Some(s),
                None => {
                    r.push(
                        Report::build(ReportKind::Error, span)
                            .with_label(Label::new(span).with_message(format!(
                                "The field `{field}` should be a {type_name}"
                            )))
                            .finish(),
                    );
                    None
                }
            }
        }
        None => {
            r.push(
                Report::build(ReportKind::Error, span)
                    .with_label(
                        Label::new(span)
                            .with_message(format!("Map should come with a field `{field}`")),
                    )
                    .finish(),
            );
            None
        }
    }
}

fn done(map: rhai::Map, span: Span, r: &Reporter) -> Option<()> {
    if !map.is_empty() {
        r.push(
            Report::build(ReportKind::Error, span)
                .with_label(Label::new(span).with_message(format!(
                    "Map contains unknown fields: {}",
                    map.keys().format_with(", ", |elt, f| f(&format_args!("`{elt}`")))
                )))
                .finish(),
        );

        return None;
    }

    Some(())
}

fn try_decode_block(
    value: Dynamic,
    span: Span,
    branch_key: MacroBranchKey,
    r: &Reporter,
) -> Option<Block> {
    let value = match value.try_cast_result::<Array>() {
        Ok(v) => v,
        Err(value) => {
            r.push(
                Report::build(ReportKind::Error, span)
                    .with_message(format!(
                        "Unable to interpret `{value}` as a positive integer, identifier, register, constant, or code block."
                    ))
                    .finish(),
            );
            return None;
        }
    };

    try_decode_block_from_array(value, span, branch_key, r)
}

/// Returns `None` if there are no instructions in the block (an empty array)
fn try_decode_block_from_array(
    value: Vec<Dynamic>,
    span: Span,
    branch_key: MacroBranchKey,
    r: &Reporter,
) -> Option<Block> {
    let mut code = Vec::new();

    for value in value {
        let value = match value.try_cast_result::<Array>() {
            Ok(instrs) => {
                let block = try_decode_block_from_array(instrs, span, branch_key, r)?;
                code.push(span.with((Instruction::Block(block), None, branch_key)));
                continue;
            }
            Err(val) => val,
        };

        let Some(mut map) = value.try_cast::<rhai::Map>() else {
            r.push(
                Report::build(ReportKind::Error, span)
                    .with_label(
                        Label::new(span).with_message("All elements of a block should be maps or arrays".to_string()),
                    )
                    .finish(),
            );
            return None;
        };

        let ty = get_ty(&mut map, span, r)?;

        let instr = match ty {
            Ty::Label => {
                let name = get_str(&mut map, "name", span, r)?;
                let public =
                    map.contains_key("public") && get::<bool>(&mut map, "public", "bool", span, r)?;

                done(map, span, r)?;

                Instruction::Label(crate::Label {
                    name: Intern::from_ref(&name),
                    maybe_block_id: None,
                    public,
                    branch_key,
                })
            }
            Ty::Call => {
                let name = get_str(&mut map, "name", span, r)?;
                let args = get::<Array>(&mut map, "args", "array", span, r)?;

                Instruction::Code(Code::Macro(MacroCall {
                    name: span.with(Intern::from_ref(&name)),
                    arguments: span.with(
                        args.into_iter()
                            .map(|value| {
                                from_rhai(value, span, branch_key, r).map(|v| span.with(v))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    ),
                }))
            }
            Ty::Const => {
                let name = get_str(&mut map, "name", span, r)?;
                done(map, span, r);

                Instruction::Constant(Intern::from_ref(&name))
            }
            Ty::Define => {
                let name = get_str(&mut map, "name", span, r)?;
                let dynamic = get::<Dynamic>(&mut map, "value", "dynamic", span, r)?;
                let value = if let Some(mut map) = dynamic.clone().try_cast::<rhai::Map>()
                    && get_ty(&mut map, span, r)? == Ty::RhaiCall
                {
                    DefineValue::RhaiCall(span.with(decode_rhai_call(map, span, branch_key, r)?))
                } else {
                    DefineValue::Value(span.with(from_rhai(dynamic, span, branch_key, r)?))
                };

                Instruction::Define(Define {
                    name: span.with(Intern::from_ref(&name)),
                    value,
                })
            }
            Ty::RhaiCall => Instruction::RhaiCall(decode_rhai_call(map, span, branch_key, r)?),
        };

        code.push(span.with((instr, None, branch_key)));
    }

    Some(Block { code })
}

fn decode_rhai_call(
    mut map: rhai::Map,
    span: Span,
    branch_key: MacroBranchKey,
    r: &Reporter,
) -> Option<RhaiCall> {
    let name = get_str(&mut map, "name", span, r)?;
    let args = get::<Array>(&mut map, "args", "array", span, r)?;

    Some(crate::RhaiCall {
        function_name: span.with(Intern::from_ref(&name)),
        args: args
            .into_iter()
            .map(|value| from_rhai(value, span, branch_key, r).map(|v| span.with(v)))
            .collect::<Option<Box<[_]>>>()?,
    })
}

use std::fmt::Debug;

use ariadne::{Color, Label, Report, ReportKind, Span as _};
use internment::ArcIntern;
use itertools::Itertools;
use puzzle_theory::{
    numbers::{I, Int, U},
};
use qter_core::span::{Span, WithSpan};
use rhai::{AST, Array, CustomType, Dynamic, Engine, ImmutableString, Position, Scope};

use crate::{
    Block, Code, ExpansionInfo, Instruction, MacroBranchKey, MacroCall, RegisterInfo, Reporter,
    ResolvedValue, Value,
};

thread_local! {
    static ENGINE: Engine = {
        let mut engine = Engine::new();

        register_int(&mut engine);
        engine.build_type::<WRegisterInfo>();

        engine.register_fn("big", |v: i64| WInt(Int::<I>::from(v)));
        engine.set_max_expr_depths(256, 256);

        engine
    };
}

#[derive(Clone, Copy, Debug)]
struct WInt(Int<I>);

impl CustomType for WInt {
    fn build(mut builder: rhai::TypeBuilder<Self>) {
        builder.with_name("BigInt").on_print(|v| v.0.to_string());
    }
}

fn register_int(engine: &mut Engine) {
    engine
        .build_type::<WInt>()
        .register_fn("-", |a: WInt| WInt(-a.0));

    register_op(engine, "+", |a, b| WInt(a + b));
    register_op(engine, "-", |a, b| WInt(a - b));
    register_op(engine, "*", |a, b| WInt(a * b));
    register_op(engine, "/", |a, b| WInt(a / b));
    register_op(engine, "%", |a, b| WInt(Int::<I>::from(a % b)));
    register_op(engine, "==", |a, b| a == b);
    register_op(engine, "!=", |a, b| a != b);
    register_op(engine, ">=", |a, b| a >= b);
    register_op(engine, ">", |a, b| a > b);
    register_op(engine, "<=", |a, b| a <= b);
    register_op(engine, "<", |a, b| a < b);
}

fn register_op<T: rhai::Variant + Clone>(
    engine: &mut Engine,
    op: &str,
    f: fn(Int<I>, Int<I>) -> T,
) {
    engine
        .register_fn(op, move |a: WInt, b: WInt| f(a.0, b.0))
        .register_fn(op, move |a: i64, b: WInt| f(Int::<I>::from(a), b.0))
        .register_fn(op, move |a: WInt, b: i64| f(a.0, Int::<I>::from(b)));
}

#[derive(Clone, Debug)]
struct WRegisterInfo(RegisterInfo);

impl CustomType for WRegisterInfo {
    fn build(mut builder: rhai::TypeBuilder<Self>) {
        builder
            .with_name("Register")
            .on_print(|v| v.0.0.to_string())
            .with_get("modulus", |v: &mut WRegisterInfo| {
                WInt(Int::<I>::from(v.0.1.modulus))
            })
            .with_get("order", |v: &mut WRegisterInfo| {
                WInt(Int::<I>::from(v.0.1.order))
            })
            .with_get("name", |v: &mut WRegisterInfo| {
                ImmutableString::from(&**v.0.0.reg_name)
            });
    }
}

#[derive(Clone, Debug)]
pub struct RhaiCode(Span);

impl RhaiCode {
    pub fn new(span: Span) -> RhaiCode {
        RhaiCode(span)
    }

    pub fn span(&self) -> &Span {
        &self.0
    }

    pub fn pos_to_span(&self, pos: Position) -> Option<Span> {
        let (mut line, pos) = match (pos.line(), pos.position()) {
            (Some(line), Some(pos)) => (line - 1, pos - 1),
            (Some(line), None) => (line - 1, 0),
            _ => return None,
        };

        let source = self.span().source().inner();

        let mut start = self.span().start();

        while line != 0 {
            start += source[start..].find('\n')? + 1;
            line -= 1;
        }

        start += pos;

        Some(Span::new(self.span().source(), start, start + 1))
    }
}

#[derive(Clone, Debug)]
pub struct RhaiMacros {
    rhai_asts: Vec<(RhaiCode, AST)>,
}

impl RhaiMacros {
    pub fn new() -> RhaiMacros {
        RhaiMacros {
            rhai_asts: Vec::new(),
        }
    }

    pub fn add_code(&mut self, rhai_code: RhaiCode, r: &Reporter) -> Option<()> {
        let compiled = match ENGINE.with(|engine| engine.compile(rhai_code.span().slice())) {
            Ok(v) => v,
            Err(err) => {
                let pos = err.position();
                let (span, default) = match rhai_code.pos_to_span(pos) {
                    Some(span) => (span, false),
                    None => (rhai_code.span().clone(), true),
                };

                let mut report = Report::build(ReportKind::Error, span.clone()).with_label(
                    Label::new(span)
                        .with_color(Color::Red)
                        .with_message(err.to_string()),
                );

                if default {
                    report = report.with_note("The Rhai compiler did not provide span information");
                }

                r.push(report.finish());

                return None;
            }
        };

        self.rhai_asts.push((rhai_code, compiled));

        Some(())
    }

    pub fn do_rhai_call(
        &self,
        name: &str,
        args: Vec<WithSpan<ResolvedValue>>,
        span: Span,
        info: &ExpansionInfo,
        r: &Reporter,
    ) -> Option<ResolvedValue> {
        let args = args
            .into_iter()
            .map(|v| into_rhai(v.into_inner(), info, r))
            .collect::<Option<Vec<_>>>()?;

        let mut result = None;
        let mut found_with_args = None;

        'done: for (code, ast) in &self.rhai_asts {
            for f in ast.iter_functions() {
                if f.name == name {
                    found_with_args = Some(f.params.len());
                    if f.params.len() == args.len() {
                        // Found the right AST!

                        result = Some((
                            code,
                            ENGINE.with(|engine| {
                                let mut scope = Scope::new();
                                engine.call_fn::<Dynamic>(&mut scope, ast, name, args)
                            }),
                        ));

                        break 'done;
                    }
                }
            }
        }

        let value = match result {
            Some((_, Ok(v))) => v,
            Some((code, Err(e))) => {
                let rhai_span = code.pos_to_span(e.position()).unwrap();
                r.push(
                    Report::build(ReportKind::Error, rhai_span.clone())
                        .with_label(Label::new(rhai_span).with_message(e.to_string()))
                        .with_label(Label::new(span).with_message("Rhai function invoked here"))
                        .finish(),
                );
                return None;
            }
            None => {
                let mut report = Report::build(ReportKind::Error, span.clone()).with_label(
                    Label::new(span).with_message("Function not declared in any Rhai block"),
                );

                if let Some(n) = found_with_args {
                    report = report.with_help(format!(
                        "Found a function with the same name that takes {n} parameters"
                    ));
                }

                r.push(report.finish());
                return None;
            }
        };

        let branch_key = info.fresh_branch_key()();

        from_rhai(value, span.clone(), branch_key, r)
    }
}

fn into_rhai(arg: ResolvedValue, info: &ExpansionInfo, r: &Reporter) -> Option<Dynamic> {
    Some(match arg {
        ResolvedValue::Int(int) => Dynamic::from(WInt(int.into())),
        ResolvedValue::Register(_) => match arg.as_reg(info) {
            crate::MaybeReg::Reg(register_reference, rhai_reg_info) => {
                Dynamic::from(WRegisterInfo((register_reference.clone(), rhai_reg_info)))
            }
            crate::MaybeReg::RegNotFound(span) => {
                r.push(
                    Report::build(ReportKind::Error, span.clone())
                        .with_message("Register does not exist")
                        .finish(),
                );
                return None;
            }
            crate::MaybeReg::NotAReg(_) => unreachable!(),
        },
        ResolvedValue::Ident(ident) => {
            let str = ImmutableString::from(&**ident);
            Dynamic::from(str)
        }
        ResolvedValue::Block(_block) => {
            todo!()
        }
    })
}

fn from_rhai(
    value: Dynamic,
    span: Span,
    branch_key: MacroBranchKey,
    r: &Reporter,
) -> Option<ResolvedValue> {
    if let Ok(int) = value.as_int() {
        let Ok(v) = u64::try_from(int) else {
            r.push(
                Report::build(ReportKind::Error, span)
                    .with_message(format!("Integer values must be positive. Found {int}"))
                    .finish(),
            );
            return None;
        };

        return Some(ResolvedValue::Int(Int::<U>::from(v)));
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

            return Some(ResolvedValue::Int(int.abs()));
        }
        Err(v) => v,
    };

    let value = match value.try_cast_result::<ImmutableString>() {
        Ok(str) => {
            return Some(ResolvedValue::Ident(span.with(ArcIntern::from(&*str))));
        }
        Err(v) => v,
    };

    let value = match value.try_cast_result::<WRegisterInfo>() {
        Ok(WRegisterInfo(str)) => {
            return Some(ResolvedValue::Register(str.0));
        }
        Err(v) => v,
    };

    try_decode_block(value, &span, branch_key, r).map(|v| {
        ResolvedValue::Block(Block {
            code: v
                .into_iter()
                .map(|v| v.map(|v| (v, None, Some(branch_key))))
                .collect_vec(),
        })
    })
}

/// Returns `None` if there are no instructions in the block (an empty array)
fn try_decode_block(
    value: Dynamic,
    span: &Span,
    branch_key: MacroBranchKey,
    r: &Reporter,
) -> Option<Option<WithSpan<Instruction>>> {
    let value = match value.try_cast_result::<Array>() {
        Ok(v) => v,
        Err(value) => {
            r.push(
                Report::build(ReportKind::Error, span.clone())
                    .with_message(format!(
                        "Unable to interpret `{value}` as a positive integer, identifier, register, or code block."
                    ))
                    .finish(),
            );
            return None;
        }
    };

    if value.is_empty() {
        return Some(None);
    }

    if value[0].is_string() {
        // This is an instruction
        let mut values = value.into_iter();
        let name = values.next().unwrap().into_string().unwrap();

        let args = values
            .map(|v| {
                from_rhai(v, span.clone(), branch_key, r)
                    .map(|v| span.clone().with(Value::Resolved(v)))
            })
            .collect::<Option<Vec<_>>>()?;

        Some(Some(span.clone().with(Instruction::Code(Code::Macro(
            MacroCall {
                name: span.clone().with(ArcIntern::from(name)),
                arguments: span.clone().with(args),
            },
        )))))
    } else {
        // This is a code block
        Some(Some(
            span.clone().with(Instruction::Block(Block {
                code: value
                    .into_iter()
                    .map(|v| try_decode_block(v, span, branch_key, r))
                    .collect::<Option<Vec<Option<WithSpan<Instruction>>>>>()?
                    .into_iter()
                    .flatten()
                    .map(|v| v.map(|v| (v, None, Some(branch_key))))
                    .collect_vec(),
            })),
        ))
    }
}

#[cfg(test)]
mod tests {
    use puzzle_theory::{
        numbers::{I, Int},
        
    };
    use qter_core::span::Span;
    use rhai::{Dynamic, Scope};

    use crate::{
        Reporter,
        parsing::tests::file,
        rhai::{ENGINE, RhaiCode, WInt},
    };

    use super::RhaiMacros;

    #[test]
    fn custom_numeric() {
        let mut rhai_vm = RhaiMacros::new();

        let r = Reporter::default();

        let code = r#"
            fn assert(x) {
                if !x {
                    throw "oopsie";
                }
            }
            
            fn fail() {
                assert(false);
            }

            fn test(zero, too_big, tenth_too_big) {
                assert(zero < big(10));
                assert(big(10) > zero);
                assert(zero + 10 <= big(10));
                assert(zero + 10 >= big(10));
                assert(too_big / 10 == tenth_too_big);
                assert(too_big % 9 == big(1));
                assert(too_big % 9 != 2);
                assert(10 / big(6) == big(1));
                assert(10 - big(4) == big(6));
                assert(-big(10) == big(-10));
            }
        "#;

        let file = file(code);

        rhai_vm
            .add_code(RhaiCode::new(Span::new(file, 0, code.len())), &r)
            .unwrap();

        ENGINE.with(|v| {
            let mut scope = Scope::new();
            assert!(
                v.call_fn::<Dynamic>(&mut scope, &rhai_vm.rhai_asts[0].1, "fail", ())
                    .is_err()
            );
        });

        let too_big0 = Int::<I>::from(u64::MAX - 5);
        let too_big = WInt(too_big0 * too_big0);
        let zero = WInt(Int::<I>::zero());
        let tenth_too_big = WInt(too_big0 * too_big0 / Int::<I>::from(10));

        ENGINE.with(|v| {
            let mut scope = Scope::new();
            let status = v.call_fn::<Dynamic>(
                &mut scope,
                &rhai_vm.rhai_asts[0].1,
                "test",
                (zero, too_big, tenth_too_big),
            );
            assert!(status.is_ok());
        });

        assert_eq!(r.count(), 0);
    }
}

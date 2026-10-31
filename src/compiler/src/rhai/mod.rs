use std::{
    any::Any,
    fmt::{Debug, Display},
};

use ariadne::{Color, Label, Report, ReportKind, Span as _};
use itertools::Itertools;
use puzzle_theory::numbers::{Int, I, U};
use qter_core::span::{Span, WithSpan};
use rhai::{Array, CustomType, Dynamic, Engine, ImmutableString, Position, Scope, AST};
use sarlacc::Intern;

use crate::{
    Block, ExpansionInfo, MacroBranchKey, RegisterInfo, RegisterModifiers, Reporter, ResolvedValue, Value, rhai::{decode::from_rhai, encode::into_rhai},
};

mod encode;
mod decode;

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
            .on_print(|v| v.0 .0.to_string())
            .with_get("mods", |v: &mut WRegisterInfo| {
                Some(register_mods(v.0 .0.mods))
            })
            .with_get("order", |v: &mut WRegisterInfo| {
                WInt(Int::<I>::from(v.0 .0.mods.modulus.unwrap_or(v.0 .1.order)))
            })
            .with_get("full_order", |v: &mut WRegisterInfo| {
                WInt(Int::<I>::from(v.0 .1.order))
            })
            .with_get("name", |v: &mut WRegisterInfo| {
                ImmutableString::from(&**v.0 .0.reg_name)
            });
    }
}

fn register_mods(RegisterModifiers { modulus, negative }: RegisterModifiers) -> Dynamic {
    let mut map = rhai::Map::new();

    map.insert(
        "modulus".into(),
        Dynamic::from(modulus.map(|v| WInt(v.into()))),
    );
    map.insert("negative".into(), negative.into());

    Dynamic::from_map(map)
}

#[derive(Clone, Copy, Debug)]
pub struct RhaiCode(Span);

impl RhaiCode {
    pub fn new(span: Span) -> RhaiCode {
        RhaiCode(span)
    }

    pub fn span(&self) -> Span {
        self.0
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
                    None => (rhai_code.span(), true),
                };

                let mut report = Report::build(ReportKind::Error, span).with_label(
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
        branch_key: MacroBranchKey,
        r: &Reporter,
    ) -> Option<Value> {
        let args = args
            .into_iter()
            .map(|v| into_rhai(v.into_inner(), info, branch_key, r))
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
                    Report::build(ReportKind::Error, rhai_span)
                        .with_label(Label::new(rhai_span).with_message(e.to_string()))
                        .with_label(Label::new(span).with_message("Rhai function invoked here"))
                        .finish(),
                );
                return None;
            }
            None => {
                let mut report = Report::build(ReportKind::Error, span).with_label(
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

        from_rhai(value, span, branch_key, r)
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Ty {
    Label,
    Call,
    Const,
    Define,
    RhaiCall,
}

impl Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Ty::Label => "label",
            Ty::Call => "call",
            Ty::Const => "const",
            Ty::Define => "define",
            Ty::RhaiCall => "rhai call",
        })
    }
}

impl From<Ty> for Dynamic {
    fn from(value: Ty) -> Self {
        value.to_string().into()
    }
}

#[cfg(test)]
mod tests {
    use puzzle_theory::numbers::{Int, I};
    use qter_core::span::Span;
    use rhai::{Dynamic, Scope};

    use crate::{
        parsing::tests::file,
        rhai::{RhaiCode, WInt, ENGINE},
        Reporter,
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
            assert!(v
                .call_fn::<Dynamic>(&mut scope, &rhai_vm.rhai_asts[0].1, "fail", ())
                .is_err());
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

use std::{fmt::Debug, sync::OnceLock};

use chumsky::error::Rich;
use internment::ArcIntern;
use itertools::Itertools;
use puzzle_theory::{
    numbers::{I, Int, U},
    span::{Span, WithSpan},
};
use rhai::{AST, Array, CustomType, Dynamic, Engine, ImmutableString, ParseError, Scope};

use crate::{
    Block, Code, ExpansionInfo, Instruction, MacroCall, RegisterInfo, ResolvedValue, Value,
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
            .with_get("order", |v: &mut WRegisterInfo| {
                WInt(Int::<I>::from(v.0.1.order))
            })
            .with_get("name", |v: &mut WRegisterInfo| {
                ImmutableString::from(&**v.0.0.reg_name)
            });
    }
}

#[derive(Clone, Debug)]
pub struct RhaiMacros {
    rhai_ast: AST,
}

impl RhaiMacros {
    pub fn new() -> RhaiMacros {
        RhaiMacros {
            rhai_ast: AST::empty(),
        }
    }

    pub fn add_code(&mut self, code: &str) -> Result<(), ParseError> {
        let compiled = ENGINE.with(|engine| engine.compile(code))?;

        self.rhai_ast.combine(compiled);

        Ok(())
    }

    pub fn do_rhai_call(
        &self,
        name: &str,
        args: Vec<WithSpan<ResolvedValue>>,
        span: Span,
        info: &ExpansionInfo,
    ) -> Result<ResolvedValue, Vec<Rich<'static, char, Span>>> {
        let args = args
            .into_iter()
            .map(|v| into_rhai(v.into_inner(), info))
            .collect_vec();

        let result = ENGINE.with(|engine| {
            let mut scope = Scope::new();
            engine.call_fn::<Dynamic>(&mut scope, &self.rhai_ast, name, args)
        });

        let value = match result {
            Ok(v) => v,
            Err(e) => return Err(vec![Rich::custom(span, e)]),
        };

        from_rhai(value, span.clone()).map_err(|v| vec![Rich::custom(span, v)])
    }
}

fn into_rhai(arg: ResolvedValue, info: &ExpansionInfo) -> Dynamic {
    match arg {
        ResolvedValue::Int(int) => Dynamic::from(WInt(int.into())),
        ResolvedValue::Ident {
            ref ident,
            as_reg: _,
        } => match arg.as_reg(info) {
            Some(Ok(reg)) => Dynamic::from(WRegisterInfo(reg.to_owned())),
            Some(Err(_)) | None => {
                let str = ImmutableString::from(&***ident);
                Dynamic::from(str)
            }
        },
        ResolvedValue::Block(block) => {
            todo!()
        }
    }
}

fn from_rhai(value: Dynamic, span: Span) -> Result<ResolvedValue, String> {
    if let Ok(int) = value.as_int() {
        let Ok(v) = u64::try_from(int) else {
            return Err(format!("Integer values must be positive. Found {int}"));
        };

        return Ok(ResolvedValue::Int(Int::<U>::from(v)));
    }

    let value = match value.try_cast_result::<WInt>() {
        Ok(WInt(int)) => {
            if int < Int::<I>::zero() {
                return Err(format!("Integer values must be positive. Found {int}"));
            }

            return Ok(ResolvedValue::Int(int.abs()));
        }
        Err(v) => v,
    };

    let value = match value.try_cast_result::<ImmutableString>() {
        Ok(str) => {
            return Ok(ResolvedValue::Ident {
                ident: span.with(ArcIntern::from(&*str)),
                as_reg: OnceLock::new(),
            });
        }
        Err(v) => v,
    };

    let value = match value.try_cast_result::<WRegisterInfo>() {
        Ok(WRegisterInfo(str)) => {
            let span = str.0.reg_name.span().clone();

            return Ok(ResolvedValue::Ident {
                ident: span.with(ArcIntern::from(str.0.to_string())),
                as_reg: OnceLock::new(),
            });
        }
        Err(v) => v,
    };

    try_decode_block(value, &span).map(|v| {
        ResolvedValue::Block(Block {
            code: v.into_iter().map(|v| v.map(|v| (v, None))).collect_vec(),
        })
    })
}

/// Returns `None` if there are no instructions in the block (an empty array)
fn try_decode_block(value: Dynamic, span: &Span) -> Result<Option<WithSpan<Instruction>>, String> {
    let value = match value.try_cast_result::<Array>() {
        Ok(v) => v,
        Err(value) => {
            return Err(format!(
                "Unable to interpret `{value}` as a positive integer, identifier, register, or code block."
            ));
        }
    };

    if value.is_empty() {
        return Ok(None);
    }

    if value[0].is_string() {
        // This is an instruction
        let mut values = value.into_iter();
        let name = values.next().unwrap().into_string().unwrap();

        let args = values
            .map(|v| from_rhai(v, span.clone()).map(|v| span.clone().with(Value::Resolved(v))))
            .try_collect::<_, Vec<_>, _>()?;

        Ok(Some(span.clone().with(Instruction::Code(Code::Macro(
            MacroCall {
                name: span.clone().with(ArcIntern::from(name)),
                arguments: span.clone().with(args),
            },
        )))))
    } else {
        // This is a code block
        Ok(Some(
            span.clone().with(Instruction::Block(Block {
                code: value
                    .into_iter()
                    .filter_map(|v| try_decode_block(v, span).transpose())
                    .map(|v| v.map(|v| v.map(|v| (v, None))))
                    .try_collect::<_, Vec<_>, _>()?,
            })),
        ))
    }
}

#[cfg(test)]
mod tests {
    use puzzle_theory::numbers::{I, Int};
    use rhai::{Dynamic, Scope};

    use crate::rhai::{ENGINE, WInt};

    use super::RhaiMacros;

    #[test]
    fn custom_numeric() {
        let mut rhai_vm = RhaiMacros::new();

        rhai_vm
            .add_code(
                r#"
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
        "#,
            )
            .unwrap();

        ENGINE.with(|v| {
            let mut scope = Scope::new();
            assert!(
                v.call_fn::<Dynamic>(&mut scope, &rhai_vm.rhai_ast, "fail", ())
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
                &rhai_vm.rhai_ast,
                "test",
                (zero, too_big, tenth_too_big),
            );
            assert!(status.is_ok());
        });
    }
}

use std::fmt::Debug;

use puzzle_theory::numbers::{I, Int};
use rhai::{AST, CustomType, Engine, ParseError, Scope};

use crate::RegisterInfo;

thread_local! {
    static ENGINE: Engine = {
        let mut engine = Engine::new();

        register_int(&mut engine);
        engine.build_type::<W<RegisterInfo>>();

        engine.register_fn("big", |v: i64| W(Int::<I>::from(v)));
        engine.set_max_expr_depths(256, 256);

        engine
    };
}

#[derive(Clone, Copy, Debug)]
struct W<T>(T);

impl CustomType for W<Int<I>> {
    fn build(mut builder: rhai::TypeBuilder<Self>) {
        builder.with_name("BigInt").on_print(|v| v.0.to_string());
    }
}

fn register_int(engine: &mut Engine) {
    engine
        .build_type::<W<Int<I>>>()
        .register_fn("-", |a: W<Int<I>>| W(-a.0));

    register_op(engine, "+", |a, b| W(a + b));
    register_op(engine, "-", |a, b| W(a - b));
    register_op(engine, "*", |a, b| W(a * b));
    register_op(engine, "/", |a, b| W(a / b));
    register_op(engine, "%", |a, b| W(Int::<I>::from(a % b)));
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
        .register_fn(op, move |a: W<Int<I>>, b: W<Int<I>>| f(a.0, b.0))
        .register_fn(op, move |a: i64, b: W<Int<I>>| f(Int::<I>::from(a), b.0))
        .register_fn(op, move |a: W<Int<I>>, b: i64| f(a.0, Int::<I>::from(b)));
}

impl CustomType for W<RegisterInfo> {
    fn build(mut builder: rhai::TypeBuilder<Self>) {
        builder
            .with_name("Register")
            .on_print(|v| v.0.0.to_string())
            .with_get("order", |v: &mut W<RegisterInfo>| v.0.1.order)
            .with_get("name", |v: &mut W<RegisterInfo>| {
                (**v.0.0.reg_name).to_owned()
            });
    }
}

#[derive(Clone)]
pub struct RhaiMacros {
    rhai_ast: AST,
    scope: Scope<'static>,
}

impl Debug for RhaiMacros {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LuaMacros").field("lua_vm", &"VM").finish()
    }
}

impl RhaiMacros {
    pub fn new() -> RhaiMacros {
        RhaiMacros {
            rhai_ast: AST::empty(),
            scope: Scope::new(),
        }
    }

    pub fn add_code(&mut self, code: &str) -> Result<(), ParseError> {
        let compiled = ENGINE.with(|engine| engine.compile(code))?;

        self.rhai_ast.combine(compiled);

        Ok(())
    }

    // fn do_lua_call(&self, span: Span, name: &str, args: Vec<WithSpan<ResolvedValue>>) -> Result<ResolvedValue, Vec<Rich<'static, char, Span>>> {
    //     self.lua_vm

    //     todo!()
    // }
}

#[cfg(test)]
mod tests {
    use puzzle_theory::numbers::{I, Int};
    use rhai::Dynamic;

    use crate::rhai::{ENGINE, W};

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
            assert!(
                v.call_fn::<Dynamic>(&mut rhai_vm.scope, &rhai_vm.rhai_ast, "fail", ())
                    .is_err()
            );
        });

        let too_big0 = Int::<I>::from(u64::MAX - 5);
        let too_big = W(too_big0 * too_big0);
        let zero = W(Int::<I>::zero());
        let tenth_too_big = W(too_big0 * too_big0 / Int::<I>::from(10));

        ENGINE.with(|v| {
            let status = v.call_fn::<Dynamic>(
                &mut rhai_vm.scope,
                &rhai_vm.rhai_ast,
                "test",
                (zero, too_big, tenth_too_big),
            );
            assert!(status.is_ok());
        });
    }
}

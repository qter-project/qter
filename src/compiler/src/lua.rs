use std::{cell::RefCell, fmt::Debug, rc::Rc, sync::Arc};

use chumsky::error::Rich;
use piccolo::Lua;
use puzzle_theory::{numbers::{I, Int}, span::{Span, WithSpan}};

use crate::{RegisterInfo, ResolvedValue};

#[derive(Clone)]
pub struct LuaMacros {
    lua_vm: Rc<RefCell<Lua>>,
}

impl Debug for LuaMacros {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LuaMacros").field("lua_vm", &"VM").finish()
    }
}

impl LuaMacros {
    pub fn new() -> LuaMacros {
        let mut lua_vm = Lua::empty();
        lua_vm.load_core();

        // lua_vm.enter(|ctx| {});

        // lua_vm.register_userdata_type(Self::init_int_methods)?;

        // lua_vm.register_userdata_type(Self::init_reg_methods)?;

        // let to_big =
        //     lua_vm.create_function(|_, v| Ok(AnyUserData::wrap(Self::value_to_int(v)?)))?;

        // lua_vm.globals().set("big", to_big)?;

        LuaMacros { lua_vm: Rc::new(RefCell::new(lua_vm)) }
    }

    pub fn add_code(&self, code: &str) {
        // self.lua_vm.load(code).exec()
        todo!()
    }

    // fn borrow_reg(v: Value) -> mlua::Result<UserDataRef<RegisterInfo>> {
    //     match v {
    //         Value::UserData(data) => data.borrow::<RegisterInfo>(),
    //         _ => Err(mlua::Error::runtime("The value isn't a register")),
    //     }
    // }

    // fn value_to_int(v: Value) -> mlua::Result<Int<I>> {
    //     match v {
    //         Value::Integer(int) => Ok(Int::from(int)),
    //         Value::UserData(data) => data.borrow::<Int<I>>().map(|v| *v),
    //         _ => Err(mlua::Error::runtime("The value isn't an integer")),
    //     }
    // }

    // fn init_int_methods(registry: &mut UserDataRegistry<Int<I>>) {
    //     registry.add_meta_function("__add", |_, (lhs, rhs): (Value, Value)| {
    //         Ok(AnyUserData::wrap(
    //             Self::value_to_int(lhs)? + Self::value_to_int(rhs)?,
    //         ))
    //     });

    //     registry.add_meta_function("__sub", |_, (lhs, rhs): (Value, Value)| {
    //         Ok(AnyUserData::wrap(
    //             Self::value_to_int(lhs)? - Self::value_to_int(rhs)?,
    //         ))
    //     });

    //     registry.add_meta_function("__mul", |_, (lhs, rhs): (Value, Value)| {
    //         Ok(AnyUserData::wrap(
    //             Self::value_to_int(lhs)? * Self::value_to_int(rhs)?,
    //         ))
    //     });

    //     registry.add_meta_function("__div", |_, (lhs, rhs): (Value, Value)| {
    //         Ok(AnyUserData::wrap(
    //             Self::value_to_int(lhs)? / Self::value_to_int(rhs)?,
    //         ))
    //     });

    //     registry.add_meta_function("__mod", |_, (lhs, rhs): (Value, Value)| {
    //         Ok(AnyUserData::wrap(Int::<I>::from(
    //             Self::value_to_int(lhs)? % Self::value_to_int(rhs)?,
    //         )))
    //     });

    //     registry.add_meta_function("__unm", |_, v: Value| {
    //         Ok(AnyUserData::wrap(-Self::value_to_int(v)?))
    //     });

    //     registry.add_meta_function("__eq", |_, (lhs, rhs): (Value, Value)| {
    //         Ok(Value::Boolean(
    //             Self::value_to_int(lhs)? == Self::value_to_int(rhs)?,
    //         ))
    //     });

    //     registry.add_meta_function("__lt", |_, (lhs, rhs): (Value, Value)| {
    //         Ok(Value::Boolean(
    //             Self::value_to_int(lhs)? < Self::value_to_int(rhs)?,
    //         ))
    //     });

    //     registry.add_meta_function("__le", |_, (lhs, rhs): (Value, Value)| {
    //         Ok(Value::Boolean(
    //             Self::value_to_int(lhs)? <= Self::value_to_int(rhs)?,
    //         ))
    //     });

    //     registry.add_meta_function("__tostring", |lua_vm, v: Value| {
    //         mlua::String::wrap(Self::value_to_int(v)?.to_string().as_bytes()).into_lua(lua_vm)
    //     });
    // }

    // fn init_reg_methods(registry: &mut UserDataRegistry<RegisterInfo>) {
    //     registry.add_function("order", |_, reg: Value| {
    //         let reg = Self::borrow_reg(reg)?;

    //         Ok(AnyUserData::wrap(reg.1.order))
    //     });

    //     registry.add_function("name", |lua_vm, reg: Value| {
    //         let reg = Self::borrow_reg(reg)?;

    //         mlua::String::wrap(reg.0.reg_name.as_bytes()).into_lua(lua_vm)
    //     });

    //     registry.add_meta_function("__tostring", |lua_vm, reg: Value| {
    //         let reg = Self::borrow_reg(reg)?;

    //         let name = match reg.0.modulus {
    //             Some(m) => format!("{}%{}", &**reg.0.reg_name, m).into_bytes(),
    //             None => reg.0.reg_name.as_bytes().to_owned(),
    //         };

    //         mlua::String::wrap(name).into_lua(lua_vm)
    //     });
    // }

    // fn do_lua_call(&self, span: Span, name: &str, args: Vec<WithSpan<ResolvedValue>>) -> Result<ResolvedValue, Vec<Rich<'static, char, Span>>> {
    //     self.lua_vm
        
    //     todo!()
    // }
}

// #[cfg(test)]
// mod tests {
//     use mlua::{AnyUserData, Function};
//     use puzzle_theory::numbers::{I, Int};

//     use super::LuaMacros;

//     #[test]
//     fn custom_numeric() {
//         let lua_vm = LuaMacros::new().unwrap();

//         lua_vm
//             .add_code(
//                 "
//             function fail()
//                 assert(false)
//             end

//             function test(zero, too_big, tenth_too_big)
//                 assert(zero < big(10))
//                 assert(zero + 10 <= big(10))
//                 assert(too_big / 10 == tenth_too_big)
//                 assert(too_big % 9 == big(1))
//                 assert(10 / big(6) == big(1))
//                 assert(10 - big(4) == big(6))
//                 assert(-big(10) == big(-10))
//             end
//         ",
//             )
//             .unwrap();

//         assert!(
//             lua_vm
//                 .lua_vm
//                 .globals()
//                 .get::<Function>("fail")
//                 .unwrap()
//                 .call::<()>(())
//                 .is_err()
//         );

//         let too_big = Int::<I>::from(u64::MAX - 5);
//         let too_big = too_big * too_big;

//         lua_vm
//             .lua_vm
//             .globals()
//             .get::<Function>("test")
//             .unwrap()
//             .call::<()>((
//                 AnyUserData::wrap(Int::<I>::zero()),
//                 AnyUserData::wrap(too_big),
//                 AnyUserData::wrap(too_big / Int::<I>::from(10)),
//             ))
//             .unwrap();
//     }
// }

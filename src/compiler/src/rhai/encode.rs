use ariadne::{Report, ReportKind};
use rhai::{Dynamic, ImmutableString};

use crate::{Block, Code, Define, DefineValue, ExpansionInfo, Instruction, MacroBranchKey, MacroCall, RegisterModifiers, Reporter, ResolvedValue, RhaiCall, Value, rhai::{Ty, WInt, WRegisterInfo, register_mods}};

pub fn into_rhai(
    arg: ResolvedValue,
    info: &ExpansionInfo,
    surrounding_branch_key: MacroBranchKey,
    r: &Reporter,
) -> Option<Dynamic> {
    Some(match arg {
        ResolvedValue::Int(int) => Dynamic::from(WInt(int.into())),
        ResolvedValue::Register(_) => match arg.as_reg(info) {
            crate::MaybeReg::Reg(register_reference, rhai_reg_info) => {
                Dynamic::from(WRegisterInfo((register_reference, rhai_reg_info)))
            }
            crate::MaybeReg::RegNotFound(span) => {
                r.push(
                    Report::build(ReportKind::Error, span)
                        .with_label(ariadne::Label::new(span).with_message("Register does not exist"))
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
        ResolvedValue::Block(block) => into_rhai_block(block, info, surrounding_branch_key, r)?,
    })
}

fn into_rhai_block(
    Block { code }: Block,
    info: &ExpansionInfo,
    surrounding_branch_key: MacroBranchKey,
    r: &Reporter,
) -> Option<Dynamic> {
    Some(Dynamic::from_array(
        code.into_iter()
            .map(|v| {
                assert!(v.1.is_none());

                let mut map = rhai::Map::new();

                match v.into_inner().0 {
                    Instruction::Label(label) => {
                        let crate::Label {
                            name,
                            maybe_block_id,
                            public,
                            branch_key,
                        } = label;
                        assert!(maybe_block_id.is_none());
                        assert_eq!(branch_key, surrounding_branch_key);
                        map.insert("ty".into(), Ty::Label.into());
                        map.insert("name".into(), (&*name).into());
                        map.insert("public".into(), public.into());
                    }
                    Instruction::Code(code) => match code {
                        Code::Primitive(_) => unreachable!(),
                        Code::Macro(MacroCall { name, arguments }) => {
                            map.insert("ty".into(), Ty::Call.into());
                            map.insert("name".into(), (&**name).into());
                            map.insert(
                                "args".into(),
                                Dynamic::from_array(
                                    arguments
                                        .into_inner()
                                        .into_iter()
                                        .map(|v| {
                                            into_rhai_unresolved(
                                                v.into_inner(),
                                                info,
                                                surrounding_branch_key,
                                                r,
                                            )
                                        })
                                        .collect::<Option<Vec<_>>>()?,
                                ),
                            );
                        }
                    },
                    Instruction::Constant(intern) => {
                        map.insert("ty".into(), Ty::Const.into());
                        map.insert("name".into(), (&*intern).into());
                        map.insert(
                            "mods".into(),
                            register_mods(RegisterModifiers {
                                modulus: None,
                                negative: false,
                            }),
                        );
                    }
                    Instruction::RhaiCall(rhai_call) => {
                        return into_rhai_rhai_call(rhai_call, info, surrounding_branch_key, r);
                    }
                    Instruction::Define(Define { name, value }) => {
                        map.insert("ty".into(), Ty::Define.into());
                        map.insert("name".into(), (&**name).into());
                        map.insert(
                            "value".into(),
                            match value {
                                DefineValue::Value(value) => into_rhai_unresolved(
                                    value.into_inner(),
                                    info,
                                    surrounding_branch_key,
                                    r,
                                )?,
                                DefineValue::RhaiCall(rhai_call) => into_rhai_rhai_call(
                                    rhai_call.into_inner(),
                                    info,
                                    surrounding_branch_key,
                                    r,
                                )?,
                            },
                        );
                    }
                    Instruction::Block(block) => {
                        return into_rhai_block(block, info, surrounding_branch_key, r);
                    }
                }

                Some(Dynamic::from_map(map))
            })
            .collect::<Option<Vec<_>>>()?,
    ))
}

fn into_rhai_rhai_call(
    RhaiCall {
        function_name,
        args,
    }: RhaiCall,
    info: &ExpansionInfo,
    surrounding_branch_key: MacroBranchKey,
    r: &Reporter,
) -> Option<Dynamic> {
    let mut map = rhai::Map::new();

    map.insert("ty".into(), Ty::RhaiCall.into());
    map.insert("name".into(), (&**function_name).into());
    map.insert(
        "args".into(),
        Dynamic::from_array(
            args.into_iter()
                .map(|arg| into_rhai_unresolved(arg.into_inner(), info, surrounding_branch_key, r))
                .collect::<Option<Vec<_>>>()?,
        ),
    );

    Some(Dynamic::from_map(map))
}

fn into_rhai_unresolved(
    arg: Value,
    info: &ExpansionInfo,
    surrounding_branch_key: MacroBranchKey,
    r: &Reporter,
) -> Option<Dynamic> {
    match arg {
        Value::Resolved(resolved_value) => {
            into_rhai(resolved_value, info, surrounding_branch_key, r)
        }
        Value::Constant { name, mods } => {
            let mut map = rhai::Map::new();

            map.insert("ty".into(), Ty::Const.into());
            map.insert("name".into(), (&*name).into());
            map.insert("mods".into(), register_mods(mods));

            Some(Dynamic::from_map(map))
        }
    }
}

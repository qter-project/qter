use crate::{
    Block, BlockInfo, BlockInfoTracker, Code, Define, DefineValue, ExpansionInfo, Label, LuaCall, MacroArgTy, MacroBranch, MacroPattern, MacroPatternComponent, ResolvedValue, Value, builtin_macros::builtin_macros, lua::LuaMacros
};
use std::{
    collections::HashMap,
    rc::Rc,
    sync::{Arc, LazyLock},
};

use chumsky::{
    extra::{Full, SimpleState},
    input::{Emitter, MapExtra},
    inspector::Inspector,
    prelude::*,
    recursive::Indirect,
};
use internment::ArcIntern;
use itertools::Itertools;
use puzzle_theory::{
    numbers::{Int, U},
    puzzle_geometry::parsing::puzzle_definition,
    span::{Extra, File, MaybeErr, Span, WithSpan},
};
use qter_core::architectures::{Architecture, with_presets};

use crate::{BlockID, Macro, ParsedSyntax, Puzzle, RegistersDecl};

use super::Instruction;

static PRELUDE: LazyLock<ParsedSyntax> = LazyLock::new(|| {
    let prelude = File::from(include_str!("../../qter_core/prelude.qat"));

    let mut parsed_prelude = match parse(
        &prelude,
        |_| {
            panic!(
                "Prelude should not import files (because it's easier not to implement; message henry if you need this feature)"
            )
        },
        true,
    ) {
        Ok(v) => v,
        Err(errs) => {
            for err in &errs {
                println!(
                    "{err}; {:?}; `{}`",
                    err.span().line_and_col(),
                    err.span().slice()
                );
            }

            panic!();
        }
    };

    let builtin_macros = builtin_macros(&prelude.inner());
    parsed_prelude
        .expansion_info
        .available_macros
        .extend(builtin_macros.keys().map(|source_and_macro_name| {
            (
                source_and_macro_name.to_owned(),
                source_and_macro_name.0.clone(),
            )
        }));
    parsed_prelude.expansion_info.macros.extend(builtin_macros);

    parsed_prelude
});

type ExtraAndSyntax = Full<
    Rich<'static, char, Span>,
    SimpleState<(Rc<dyn Fn(&str) -> Result<ArcIntern<str>, String>>, bool)>,
    (),
>;

pub fn parse(
    qat: &File,
    find_import: impl Fn(&str) -> Result<ArcIntern<str>, String> + 'static,
    is_prelude: bool,
) -> Result<ParsedSyntax, Vec<Rich<'static, char, Span>>> {
    thread_local! {
        static PARSER: Boxed<'static, 'static, File, MaybeErr<ParsedSyntax>, ExtraAndSyntax> = parser().boxed();
    }

    let mut parsed_syntax_and_extras = SimpleState((
        Rc::from(find_import) as Rc<dyn Fn(&str) -> Result<ArcIntern<str>, String>>,
        is_prelude,
    ));

    let parsed_syntax = PARSER
        .with(|parser| parser.parse_with_state(qat.clone(), &mut parsed_syntax_and_extras))
        .into_result()?;

    Ok(match parsed_syntax {
        MaybeErr::Some(v) => v,
        MaybeErr::None => {
            unreachable!("A Result::None would have been returned if there were errors")
        }
    })
}

type ExtraAndState<S> = Full<Rich<'static, char, Span>, S, ()>;

fn parser() -> impl Parser<'static, File, MaybeErr<ParsedSyntax>, ExtraAndSyntax> {
    group((
        shebang().or_not(),
        registers()
            .with_state(())
            .map_with(|regs, data: &mut MapExtra<'_, '_, File, ExtraAndSyntax>| {
                data.span().with(regs)
            })
            .or_not(),
        statement()
            .with_state(())
            .separated_by(nl())
            .allow_trailing()
            .collect::<Vec<_>>(),
        nlm(),
    ))
    .validate(|(_, regs, statements, ()), data, emitter| {
        let qat = data.span().source();

        let zero_span = Span::new(data.span().source(), 0, 0);

        let lua_macros = match LuaMacros::new() {
            Ok(v) => v,
            Err(e) => {
                emitter.emit(Rich::custom(
                    zero_span,
                    format!("Failed to initialize the Lua runtime. {e}"),
                ));
                return MaybeErr::None;
            }
        };

        let expansion_info = ExpansionInfo {
            registers: match regs {
                Some(decl) => match decl.spanspose() {
                    MaybeErr::Some(decl) => Some(decl),
                    MaybeErr::None => return MaybeErr::None,
                },
                None => None,
            },
            block_info: BlockInfoTracker {
                blocks: HashMap::new(),
                block_counter: 1,
            },
            macros: HashMap::new(),
            available_macros: HashMap::new(),
            lua_macros: HashMap::new(),
        };

        let code = Vec::new();

        let mut parsed_syntax = ParsedSyntax {
            expansion_info,
            code,
        };

        let is_prelude = data.state().0.1;
        if !is_prelude {
            merge_files(
                &mut parsed_syntax,
                &qat,
                (*PRELUDE).clone(),
                data.span(),
                emitter,
            );
        }

        for statement in statements.into_iter().filter_map(MaybeErr::option) {
            match statement {
                Statement::Macro { name, def } => {
                    if parsed_syntax
                        .expansion_info
                        .macros
                        .contains_key(&(ArcIntern::clone(&qat), ArcIntern::clone(&name)))
                    {
                        emitter.emit(Rich::custom(
                            name.span().clone(),
                            "This macro is already defined.",
                        ));
                        continue;
                    }

                    parsed_syntax
                        .expansion_info
                        .macros
                        .insert((ArcIntern::clone(&qat), ArcIntern::clone(&name)), def);
                    parsed_syntax
                        .expansion_info
                        .available_macros
                        .insert((ArcIntern::clone(&qat), name.into_inner()), qat.clone());
                }
                Statement::Instruction(instr) => {
                    parsed_syntax
                        .code
                        .push(instr.map(|instr| (instr, Some(BlockID(0)))));
                }
                Statement::LuaBlock(lua) => {
                    if let Err(e) = lua_macros.add_code(lua.slice()) {
                        emitter.emit(Rich::custom(data.span(), e.to_string()));
                    }
                }
                Statement::Import(filename) => {
                    let state_ref = &data.state().0;

                    let find_import = Rc::clone(&state_ref.0);
                    let is_prelude = state_ref.1;

                    let import = match (find_import)(filename.slice()) {
                        Ok(v) => v,
                        Err(e) => {
                            emitter.emit(Rich::custom(
                                filename,
                                format!("Unable to find import: {e}"),
                            ));

                            continue;
                        }
                    };

                    let importee =
                        match parse(&File::from(import), move |v| (find_import)(v), is_prelude) {
                            Ok(v) => v,
                            Err(errs) => {
                                for err in errs {
                                    emitter.emit(err);
                                }

                                continue;
                            }
                        };

                    merge_files(&mut parsed_syntax, &qat, importee, data.span(), emitter);
                }
            }
        }

        parsed_syntax
            .expansion_info
            .lua_macros
            .insert(data.span().source(), lua_macros);

        parsed_syntax.expansion_info.block_info.blocks.insert(
            BlockID(0),
            BlockInfo {
                parent_block: None,
                child_blocks: vec![],
                defines: HashMap::new(),
                labels: vec![],
            },
        );

        MaybeErr::Some(parsed_syntax)
    })
}

fn shebang<S: Inspector<'static, File> + 'static>()
-> impl Parser<'static, File, (), ExtraAndState<S>> {
    group((just('\n').not(), any()))
        .repeated()
        .delimited_by(just("#!"), just('\n'))
        .to(())
}

fn req_whitespace<S: Inspector<'static, File> + 'static>()
-> impl Parser<'static, File, (), ExtraAndState<S>> {
    choice((
        just(' ').to(()),
        just('\t').to(()),
        any()
            .repeated()
            .delimited_by(just("--[["), just("]]--"))
            .to(()),
    ))
    .repeated()
    .at_least(1)
}

fn whitespace<S: Inspector<'static, File> + 'static>()
-> impl Parser<'static, File, (), ExtraAndState<S>> {
    req_whitespace().or_not().to(())
}

fn line_comment<S: Inspector<'static, File> + 'static>()
-> impl Parser<'static, File, (), ExtraAndState<S>> {
    group((just('\n').not(), any()))
        .repeated()
        .delimited_by(just("--"), just('\n'))
        .to(())
}

fn nl<S: Inspector<'static, File> + 'static>() -> impl Parser<'static, File, (), ExtraAndState<S>> {
    group((
        whitespace(),
        choice((just('\n').to(()), line_comment()))
            .separated_by(whitespace())
            .at_least(1)
            .allow_trailing(),
    ))
    .to(())
}

fn nlm<S: Inspector<'static, File> + 'static>() -> impl Parser<'static, File, (), ExtraAndState<S>>
{
    group((
        whitespace(),
        choice((just('\n').to(()), line_comment()))
            .separated_by(whitespace())
            .allow_trailing(),
    ))
    .to(())
}

fn number<S: Inspector<'static, File> + 'static>()
-> impl Parser<'static, File, (), ExtraAndState<S>> {
    any()
        .filter(|c: &char| c.is_ascii_digit())
        .repeated()
        .at_least(1)
        .to(())
}

fn intu<S: Inspector<'static, File> + 'static>()
-> impl Parser<'static, File, MaybeErr<Int<U>>, ExtraAndState<S>> {
    number().validate(|(), data, emitter| match data.span().slice().parse() {
        Ok(v) => MaybeErr::Some(v),
        Err(e) => {
            emitter.emit(Rich::custom(
                data.span(),
                format!("Could not parse as an integer: {e}"),
            ));
            MaybeErr::None
        }
    })
}

fn simple_ident<S: Inspector<'static, File> + 'static>()
-> impl Parser<'static, File, WithSpan<ArcIntern<str>>, ExtraAndState<S>> {
    let special_char = choice((
        just('{').to(()),
        just('}').to(()),
        just('.').to(()),
        just(':').to(()),
        just('$').to(()),
        just("--").to(()),
        just(',').to(()),
        just("<-").to(()),
        just('←').to(()),
        just('\n').to(()),
        just('(').to(()),
        just(')').to(()),
        just('!').to(()),
        just('"').to(()),
        req_whitespace(),
    ));

    Parser::map(
        group((special_char.not(), any()))
            .repeated()
            .at_least(1)
            .to_span()
            .filter(|span: &Span| {
                span.slice().chars().any(|c| !c.is_ascii_digit()) && span.slice() != "lua"
            }),
        |v| WithSpan::new(ArcIntern::from(v.slice()), v),
    )
}

fn quoted_ident<S: Inspector<'static, File> + 'static>()
-> impl Parser<'static, File, WithSpan<ArcIntern<str>>, ExtraAndState<S>> {
    group((just('"').not(), any()))
        .repeated()
        .to_span()
        .delimited_by(just('"'), just('"'))
        .map(|v: Span| WithSpan::new(ArcIntern::from(v.slice()), v))
}

fn ident<S: Inspector<'static, File> + 'static>()
-> impl Parser<'static, File, WithSpan<ArcIntern<str>>, ExtraAndState<S>> {
    choice((simple_ident(), quoted_ident()))
}

fn tag_ident<S: Inspector<'static, File> + 'static>()
-> impl Parser<'static, File, (bool, WithSpan<ArcIntern<str>>), ExtraAndState<S>> {
    group((just('!').or_not().map(|v| v.is_some()), ident()))
}

fn constant<S: Inspector<'static, File> + 'static>()
-> impl Parser<'static, File, WithSpan<ArcIntern<str>>, ExtraAndState<S>> {
    group((just('$'), ident())).validate(|(_, v), data, _| data.span().with(v.into_inner()))
}

fn registers() -> impl Parser<'static, File, MaybeErr<RegistersDecl>, Extra> {
    group((
        just(".registers"),
        whitespace(),
        just("{"),
        register_decl()
            .separated_by(nl())
            .at_least(1)
            .allow_leading()
            .allow_trailing()
            .collect::<MaybeErr<Vec<_>>>(),
        just("}"),
    ))
    .delimited_by(nlm(), nlm())
    .map(|(_, (), _, puzzles, _)| puzzles.map(|puzzles| RegistersDecl { puzzles }))
}

fn register_decl() -> impl Parser<'static, File, MaybeErr<Puzzle>, Extra> {
    choice((register_decl_switchable(), register_decl_unswitchable()))
}

fn register_decl_unswitchable() -> impl Parser<'static, File, MaybeErr<Puzzle>, Extra> {
    group((
        ident()
            .separated_by(just(',').delimited_by(whitespace(), whitespace()))
            .at_least(1)
            .collect::<Vec<_>>(),
        choice((just("<-").to(()), just('←').to(()))).delimited_by(whitespace(), whitespace()),
        register_architecture(),
    ))
    .validate(|(mut names, (), archs), data, emitter| {
        archs
            .map(|archs| match archs {
                PuzzleUnnamed::Theoretical { order } => {
                    if names.len() == 1 {
                        MaybeErr::Some(Puzzle::Theoretical {
                            name: names.pop().unwrap(),
                            order,
                        })
                    } else {
                        emitter.emit(Rich::custom(
                            data.span(),
                            format!("Expected one name whereas {} were provided.", names.len()),
                        ));

                        MaybeErr::None
                    }
                }
                PuzzleUnnamed::Real {
                    architecture,
                    def_span,
                } => {
                    if architecture.registers().len() == names.len() {
                        MaybeErr::Some(Puzzle::Real {
                            architectures: vec![(names, architecture, def_span)],
                        })
                    } else {
                        emitter.emit(Rich::custom(
                            data.span(),
                            format!(
                                "Expected {} names whereas {} were provided.",
                                architecture.registers().len(),
                                names.len()
                            ),
                        ));

                        MaybeErr::None
                    }
                }
            })
            .flatten()
    })
}

#[derive(Clone, Debug)]
enum PuzzleUnnamed {
    Theoretical {
        order: WithSpan<Int<U>>,
    },
    Real {
        architecture: WithSpan<Arc<Architecture>>,
        def_span: Span,
    },
}

fn algorithm() -> impl Parser<'static, File, Vec<Span>, Extra> {
    ident()
        .to_span()
        .separated_by(req_whitespace())
        .at_least(1)
        .collect()
}

fn register_architecture() -> impl Parser<'static, File, MaybeErr<PuzzleUnnamed>, Extra> {
    choice((
        group((
            just("theoretical"),
            whitespace(),
            intu().map_with(|v, extra| v.map(|v| extra.span().with(v))),
        ))
        .map(|(_, (), order)| order.map(|order| PuzzleUnnamed::Theoretical { order })),
        group((
            puzzle_definition().map(with_presets),
            whitespace(),
            just("builtin"),
            whitespace(),
            choice((
                intu().map(|v| v.map(|v| vec![v])),
                intu()
                    .separated_by(just(",").delimited_by(nlm(), nlm()))
                    .at_least(1)
                    .allow_trailing()
                    .collect::<MaybeErr<Vec<_>>>()
                    .delimited_by(group((just("("), nlm())), group((nlm(), just(")")))),
            ))
            .map_with(|v, data| v.map(|v| data.span().with(v))),
        ))
        .validate(
            |(def, (), _, (), orders), data, emitter| orders.map(|orders| if let Some(arch) = def.get_preset(&orders) { MaybeErr::Some(PuzzleUnnamed::Real {
                architecture: data.span().with(arch),
                def_span: def.span().clone(),
            }) } else {
                emitter.emit(Rich::custom(
                                orders.span().clone(),
                                "There does not exist a preset architecture with the given orders.",
                            ));
                            MaybeErr::None
            },
        ).flatten()),
        group((
            puzzle_definition().map(with_presets),
            whitespace(),
            choice((
                algorithm().map(|v| vec![v]),
                algorithm()
                    .separated_by(just(",").delimited_by(nlm(), nlm()))
                    .allow_trailing()
                    .at_least(1)
                    .collect()
                    .delimited_by(group((just("("), nlm())), group((nlm(), just(")")))),
            ))
            .map_with(|v, data| data.span().with(v)),
            whitespace(),
        ))
        .validate(|(def, (), algs, ()), data, emitter| {
            match Architecture::new(Arc::clone(&def.perm_group), &algs) {
                Ok(arch) => MaybeErr::Some(PuzzleUnnamed::Real {
                    architecture: data.span().with(Arc::new(arch)),
                    def_span: def.span().clone(),
                }),
                Err(bad_generator) => {
                    emitter.emit(Rich::custom(bad_generator.clone(), format!("This generator does not exist in the given permutation group. The options are: {}", def.perm_group.generators().map(|(name, _)| name).join(&ArcIntern::from(", ")))));

                    MaybeErr::None
                },
            }
        }),
    ))
}

fn register_decl_switchable() -> impl Parser<'static, File, MaybeErr<Puzzle>, Extra> {
    register_decl_unswitchable()
        .validate(|v, data, emitter| {
            v.map(|v| match v {
                Puzzle::Theoretical { name: _, order: _ } => {
                    emitter.emit(Rich::custom(
                        data.span(),
                        "Theoretical architectures cannot be switchable.",
                    ));
                    MaybeErr::None
                }
                Puzzle::Real { architectures } => MaybeErr::Some(architectures),
            })
            .flatten()
        })
        .separated_by(nl())
        .allow_leading()
        .allow_trailing()
        .at_least(1)
        .collect::<MaybeErr<Vec<_>>>()
        .delimited_by(just('('), just(')'))
        .map(|v| {
            v.map(|v| Puzzle::Real {
                architectures: v
                    .into_iter()
                    .reduce(|mut a, b| {
                        a.extend_from_slice(&b);
                        a
                    })
                    .unwrap(),
            })
        })
}

type BlockParser = Recursive<Indirect<'static, 'static, File, MaybeErr<Block>, Extra>>;

enum Statement {
    Macro {
        name: WithSpan<ArcIntern<str>>,
        def: WithSpan<Macro>,
    },
    Instruction(WithSpan<Instruction>),
    LuaBlock(Span),
    Import(Span),
}

fn statement() -> impl Parser<'static, File, MaybeErr<Statement>, Extra> {
    let mut block_rec: BlockParser = Recursive::declare();
    block_rec.define(block(block_rec.clone()));

    choice((
        parse_macro(block_rec.clone()).map(|v| v.map(|(name, def)| Statement::Macro { name, def })),
        instruction(block_rec).map(|instr| instr.map(Statement::Instruction)),
        lua_block().map(|v| MaybeErr::Some(Statement::LuaBlock(v))),
        import().map(|v| v.map(Statement::Import)),
    ))
}

fn parse_macro(
    block_rec: BlockParser,
) -> impl Parser<'static, File, MaybeErr<(WithSpan<ArcIntern<str>>, WithSpan<Macro>)>, Extra> {
    group((
        just(".macro"),
        req_whitespace(),
        ident(),
        req_whitespace(),
        macro_branch(block_rec)
            .separated_by(nl())
            .allow_leading()
            .allow_trailing()
            .collect::<MaybeErr<Vec<_>>>()
            .delimited_by(just("{"), just("}")),
    ))
    .validate(
        |(_, (), name, (), branches),
         data: &mut MapExtra<'_, '_, File, Extra>,
         emitter| {
            let MaybeErr::Some(branches) = branches else {
                return MaybeErr::None;
            };

            let span = data.span();

            let mut conflict = false;

            for [branch1, branch2] in branches.iter().array_combinations() {
                if let Some(counterexample) = branch2.pattern.conflicts_with(&name, &branch1.pattern) {
                    emitter.emit(Rich::custom(branch2.span().clone(), format!(
                        "This macro branch conflicts with the macro branch with the pattern `{}`. A counterexample matching both is `{counterexample}`.",
                        branch1.pattern.span().slice(),
                    )));

                    conflict = true;
                }
            }

            if conflict {
                return MaybeErr::None;
            }

            let macro_def = span.with(Macro::UserDefined {
                branches,
            });

            MaybeErr::Some((name, macro_def))

        },
    )
}

fn macro_branch(
    block_rec: BlockParser,
) -> impl Parser<'static, File, MaybeErr<WithSpan<MacroBranch>>, Extra> {
    group((
        choice((
            ident().map(|v| MacroPatternComponent::Word(v.into_inner())),
            group((constant(), just(":"), macro_arg_ty()))
                .map(|(name, _, ty)| MacroPatternComponent::Argument { name, ty }),
        ))
        .map_with(|v, data| data.span().with(v))
        .separated_by(req_whitespace())
        .allow_leading()
        .allow_trailing()
        .collect::<Vec<_>>()
        .map_with(|v, data| data.span().with(MacroPattern(v)))
        .delimited_by(just('('), just(')')),
        whitespace(),
        just("=>"),
        whitespace(),
        choice((
            instruction(block_rec.clone()).map_with(|instr, data| {
                instr.map(|instr| Block {
                    code: vec![data.span().with((instr.value, None))],
                })
            }),
            block_rec,
        )),
    ))
    .map_with(|(pattern, (), _, (), block), data| {
        block.map(|block| {
            data.span().with(MacroBranch {
                pattern,
                code: block.code,
            })
        })
    })
}

fn macro_arg_ty() -> impl Parser<'static, File, WithSpan<MacroArgTy>, Extra> {
    choice((
        just("int").to(MacroArgTy::Int),
        just("reg").to(MacroArgTy::Reg),
        just("block").to(MacroArgTy::Block),
        just("ident").to(MacroArgTy::Ident),
    ))
    .map_with(
        |v, data: &mut MapExtra<'_, '_, File, Full<Rich<'_, char, Span>, (), ()>>| {
            data.span().with(v)
        },
    )
}

fn value(block_rec: BlockParser) -> impl Parser<'static, File, MaybeErr<WithSpan<Value>>, Extra> {
    choice((
        intu().map(|v| v.map(|v| Value::Resolved(ResolvedValue::Int(v)))),
        constant().map(|v| MaybeErr::Some(Value::Constant(v.value))),
        ident().map(|v| MaybeErr::Some(Value::Resolved(ResolvedValue::Ident(v.value)))),
        block_rec.map(|v| v.map(|v| Value::Resolved(ResolvedValue::Block(v)))),
    ))
    .map_with(|v, data| v.map(|v| data.span().with(v)))
}

fn instruction(
    block_rec: BlockParser,
) -> impl Parser<'static, File, MaybeErr<WithSpan<Instruction>>, Extra> {
    choice((
        label().map(MaybeErr::Some),
        code(block_rec.clone()),
        constant().map(|v| MaybeErr::Some(v.span().clone().with(Instruction::Constant(v.value)))),
        lua_call(block_rec.clone()).map(|v| v.map(|v| v.map(Instruction::LuaCall))),
        define(block_rec),
    ))
}

fn label() -> impl Parser<'static, File, WithSpan<Instruction>, Extra> {
    group((tag_ident(), whitespace(), just(':'))).map_with(|((public, name), (), _), data| {
        data.span().with(Instruction::Label(Label {
            name: name.value,
            public,
            maybe_block_id: None,
            available_in_blocks: None,
        }))
    })
}

fn code(
    block_rec: BlockParser,
) -> impl Parser<'static, File, MaybeErr<WithSpan<Instruction>>, Extra> {
    group((
        ident(),
        req_whitespace(),
        value(block_rec)
            .separated_by(req_whitespace())
            .allow_trailing()
            .collect::<MaybeErr<Vec<_>>>()
            .map_with(|v, data| v.map(|v| data.span().with(v))),
    ))
    .map_with(|(name, (), args), data| {
        args.map(|arguments| {
            data.span()
                .with(Instruction::Code(Code::Macro(crate::MacroCall {
                    name,
                    arguments,
                })))
        })
    })
}

fn lua_call(
    block_rec: BlockParser,
) -> impl Parser<'static, File, MaybeErr<WithSpan<LuaCall>>, Extra> {
    group((
        just("lua"),
        req_whitespace(),
        ident(),
        whitespace(),
        value(block_rec)
            .separated_by(just(',').delimited_by(nlm(), nlm()))
            .collect::<MaybeErr<Vec<_>>>()
            .delimited_by(group((just("("), nlm())), group((nlm(), just(")")))),
    ))
    .map_with(|(_, (), name, (), args), data| {
        args.map(|args| {
            data.span().with(LuaCall {
                function_name: name,
                args,
            })
        })
    })
}

fn define(
    block_rec: BlockParser,
) -> impl Parser<'static, File, MaybeErr<WithSpan<Instruction>>, Extra> {
    group((
        just(".define"),
        req_whitespace(),
        ident(),
        req_whitespace(),
        choice((
            lua_call(block_rec.clone()).map(|v| v.map(DefineValue::LuaCall)),
            value(block_rec).map(|v| v.map(DefineValue::Value)),
        )),
    ))
    .map_with(|(_, (), name, (), value), data| {
        value.map(|value| {
            data.span()
                .with(Instruction::Define(Define { name, value }))
        })
    })
}

fn lua_block() -> impl Parser<'static, File, Span, Extra> {
    group((
        just(".start-lua"),
        group((just("end-lua").not(), any())).repeated().to_span(),
        just("end-lua"),
    ))
    .map(|(_, span, _)| span)
}

fn import() -> impl Parser<'static, File, MaybeErr<Span>, Extra> {
    group((
        just(".import"),
        req_whitespace(),
        choice((
            group((simple_ident(), just(".qat")))
                .to_span()
                .map(MaybeErr::Some),
            quoted_ident().validate(|v, data, emitter| {
                if v.ends_with(".qat") {
                    MaybeErr::Some(v.span().clone())
                } else {
                    emitter.emit(Rich::custom(
                        data.span(),
                        "The file name must end in `.qat`.",
                    ));
                    MaybeErr::None
                }
            }),
        )),
    ))
    .map(|(_, (), span)| span)
}

fn block(block_rec: BlockParser) -> impl Parser<'static, File, MaybeErr<Block>, Extra> + Clone {
    Rc::new(
        instruction(block_rec)
            .map(|v| v.map(|v| v.span().clone().with((v.value, None))))
            .separated_by(nl())
            .allow_leading()
            .allow_trailing()
            .collect::<MaybeErr<Vec<_>>>()
            .map(|code| code.map(|code| Block { code }))
            .delimited_by(just('{'), just('}')),
    )
}

fn merge_files(
    importer: &mut ParsedSyntax,
    importer_contents: &ArcIntern<str>,
    mut importee: ParsedSyntax,
    span: Span,
    emitter: &mut Emitter<Rich<'static, char, Span>>,
) {
    match (
        &importer.expansion_info.registers,
        importee.expansion_info.registers,
    ) {
        (None, Some(regs)) => importer.expansion_info.registers = Some(regs),
        (Some(_), Some(_)) => emitter.emit(Rich::custom(
            span,
            "Cannot merge files that both contain registers declarations.",
        )),
        (_, None) => {}
    }

    // Block numbers shouldn't be defined deeper than the root in this stage
    let block_offset = importer.expansion_info.block_info.block_counter;

    let mut max_block = 0;

    for (block_id, block_info) in importee.expansion_info.block_info.blocks {
        max_block = max_block.max(block_id.0);

        importer
            .expansion_info
            .block_info
            .blocks
            .insert(BlockID(block_id.0 + block_offset), block_info);
    }

    importer
        .expansion_info
        .macros
        .extend(importee.expansion_info.macros);
    for (source_and_macro_name, macro_file) in importee.expansion_info.available_macros {
        // Imports should not shadow existing macros
        importer
            .expansion_info
            .available_macros
            .entry((
                ArcIntern::clone(importer_contents),
                ArcIntern::clone(&source_and_macro_name.1),
            ))
            .or_insert_with(|| ArcIntern::clone(&macro_file));

        importer
            .expansion_info
            .available_macros
            .insert(source_and_macro_name, macro_file);
    }
    importer
        .expansion_info
        .lua_macros
        .extend(importee.expansion_info.lua_macros);

    importee.code.iter_mut().for_each(|tagged_instruction| {
        if let Some(block_id) = &mut tagged_instruction.1 {
            block_id.0 += block_offset;
        }
    });
    importer.code.extend(importee.code);
}

#[cfg(test)]
mod tests {
    use chumsky::Parser;
    use internment::ArcIntern;
    use puzzle_theory::span::File;

    use super::{ident, number, parse, registers};

    #[test]
    fn test_number() {
        number::<()>().parse(File::from("123")).unwrap();
        number::<()>().parse(File::from("12398263596868928956891896286935689869218695689689297479561963469856981968423679569173479159")).unwrap();

        assert!(number::<()>().parse(File::from("")).has_errors());
        assert!(number::<()>().parse(File::from("3x3")).has_errors());
        assert!(number::<()>().parse(File::from("0.12")).has_errors());
        assert!(number::<()>().parse(File::from("-11")).has_errors());
        assert!(number::<()>().parse(File::from("-11")).has_errors());
    }

    #[test]
    fn test_ident() {
        ident::<()>().parse(File::from("a")).unwrap();
        ident::<()>().parse(File::from("A")).unwrap();
        ident::<()>().parse(File::from("3x3")).unwrap();
        ident::<()>().parse(File::from("thingy")).unwrap();
        ident::<()>().parse(File::from("pluah")).unwrap();
        ident::<()>().parse(File::from("->")).unwrap();
        ident::<()>().parse(File::from("\"345\"")).unwrap();
        ident::<()>().parse(File::from("\"lua\"")).unwrap();

        assert!(ident::<()>().parse(File::from("345")).has_errors());
        assert!(ident::<()>().parse(File::from("lua")).has_errors());
        assert!(ident::<()>().parse(File::from("thing<-thing")).has_errors());
        assert!(ident::<()>().parse(File::from("aa.aa")).has_errors());
        assert!(ident::<()>().parse(File::from("!aaaa")).has_errors());
        assert!(ident::<()>().parse(File::from("aaaa)")).has_errors());
    }

    #[test]
    fn test_registers() {
        let code = "
            .registers {
                a, b <- 3x3 builtin (90, 90)
                (
                    c, d ← 3x3 builtin (210, 24)
                    d, e, f ← 3x3 builtin (30, 30, 30)
                )
                f ← theoretical 90
                g, h ← 3x3 (U, D)
            }
        ";

        let errs = registers().parse(File::from(code)).into_errors();

        for err in &errs {
            println!("{err}; {:?}", err.span().line_and_col());
        }

        assert!(errs.is_empty());
    }

    #[test]
    fn bruh() {
        let code = "
            .registers {
                a, b ← 3x3 builtin ( 90 , 90 )
                (
                    c, d ← 3x3 builtin (210, 24)
                    d, e, f ← 3x3 builtin (30, 30, 30)
                )
                f ← theoretical 90
                g, h ← 3x3 (U , D    )
            }

            .macro bruh {
                ( lmao $a:reg) => add 1 $a
                (oofy $a:reg ) => {
                    bruh:
                    add 1 $a
                    goto bruh
                }
            }

            .start-lua
                function bruh()
                    print(\"skibidi\")
                end
            end-lua

            bruh :
            bruhy:
            add 1 a
            goto bruh

            lua bruh( 1,2 , 3)

            .define yeet lua bruh(1, 2, 3)
            .define pog 4

            .import pog.qat
            .import \"pog.qat\"
        ";

        match parse(
            &File::from(code),
            |name| {
                assert_eq!(name, "pog.qat");
                Ok(ArcIntern::from("add 1 a"))
            },
            false,
        ) {
            Ok(_) => {}
            Err(errs) => {
                for err in &errs {
                    println!(
                        "{err}; {:?}; `{}`",
                        err.span().line_and_col(),
                        err.span().slice()
                    );
                }

                panic!();
            }
        }
    }
}

use std::{
    collections::HashMap,
    mem,
    rc::Rc,
    sync::{Arc, atomic::AtomicUsize},
};

use ariadne::{Color, Label, Report, ReportKind};
use itertools::Itertools;
use puzzle_theory::{
    permutations::{Algorithm, Permutation, PermutationGroup},
    puzzle_geometry::PuzzleGeometry,
};
use qter_core::architectures::{Architecture, with_presets};
use qter_core::span::{File, Span, WithSpan};
use sarlacc::Intern;

use crate::{
    Block, BlockID, BlockInfo, BlockInfoTracker, Code, DefineUnresolved, DefineValue,
    ExpansionInfo, Instruction, Macro, MacroArgTy, MacroBranch, MacroBranchKey, MacroPattern,
    MacroPatternComponent, ParsedSyntax, Puzzle, RegistersDecl, ResolvedValue, RhaiCall, Value,
    parsing::{
        level_two::{Attempt, Token, TokenIter},
        tokenizer::{Encloser, Symbol},
    },
    rhai::RhaiMacros,
};

pub fn parse(
    iter: &mut TokenIter,
    find_import: &Rc<impl Fn(&str) -> Result<Intern<str>, String> + 'static>,
    is_prelude: bool,
) -> Option<ParsedSyntax> {
    let registers = match registers(iter) {
        Attempt::NotTaken(_) => None,
        Attempt::Taken(regs) => Some(regs?),
    };

    let expansion_info = ExpansionInfo {
        registers,
        block_info: BlockInfoTracker {
            blocks: HashMap::new(),
            block_counter: 1,
            reporter: iter.r(),
        },
        macros: HashMap::new(),
        available_macros: HashMap::new(),
        rhai_macros: HashMap::new(),
        branch_count: Arc::new(AtomicUsize::new(0)),
    };

    let code = Vec::new();

    let mut parsed_syntax = ParsedSyntax {
        expansion_info,
        code,
    };

    if !is_prelude {
        super::merge_files(
            &mut parsed_syntax,
            iter.file(),
            super::PRELUDE.with(|v| (*v).clone()),
            Span::new(iter.file(), 0, iter.file().inner().chars().count()),
            &iter.r(),
        );
    }

    parsed_syntax.expansion_info.block_info.blocks.insert(
        BlockID(0),
        BlockInfo {
            parent_block: None,
            child_blocks: vec![],
            defines: HashMap::new(),
            labels: vec![],
        },
    );

    let mut rhai_macros = RhaiMacros::new();

    let mut ok = true;

    loop {
        let marker = iter.marker();

        if let Attempt::Taken(instr) = instruction(iter) {
            let instr = instr?;
            let span = instr.span();
            parsed_syntax
                .code
                .push(span.with((instr.into_inner(), None, None)));

            continue;
        }

        let token = iter.next()?;
        let span = token.span();

        match &*token {
            Token::Directive(directive) => {
                if directive.as_ref() == "macro" {
                    let (name, def) =
                        macro_def(iter, parsed_syntax.expansion_info.fresh_branch_key())?;

                    if parsed_syntax
                        .expansion_info
                        .macros
                        .contains_key(&(iter.file(), *name))
                    {
                        iter.report(
                            Report::build(ReportKind::Error, name.span())
                                .with_message("This macro is already defined.")
                                .finish(),
                        );
                        continue;
                    }

                    parsed_syntax
                        .expansion_info
                        .macros
                        .insert((iter.file(), *name), def);
                    parsed_syntax
                        .expansion_info
                        .available_macros
                        .insert((iter.file(), name.into_inner()), iter.file());
                } else if directive.as_ref() == "import" {
                    let filename = iter.ident()?;

                    if !filename.ends_with(".qat") {
                        iter.report(
                            Report::build(ReportKind::Error, filename.span())
                                .with_message("The file extension must be `.qat`")
                                .finish(),
                        );
                        ok = false;
                        continue;
                    }

                    let import = match (find_import)(&filename.value) {
                        Ok(v) => v,
                        Err(e) => {
                            iter.report(
                                Report::build(ReportKind::Error, filename.span())
                                    .with_message(format!("Unable to find import: {e}"))
                                    .finish(),
                            );
                            ok = false;

                            continue;
                        }
                    };

                    let find_import = Rc::clone(find_import);

                    let Some(importee) = super::parse(
                        File::new(filename.value, import),
                        &find_import,
                        is_prelude,
                        &iter.r(),
                    ) else {
                        continue;
                    };

                    super::merge_files(
                        &mut parsed_syntax,
                        iter.file(),
                        importee.value,
                        iter.cash_in(marker),
                        &iter.r(),
                    );
                } else {
                    iter.report(
                        Report::build(ReportKind::Error, span)
                            .with_message("Unknown directive; expected .macro or .import")
                            .finish(),
                    );
                    return None;
                }
            }
            Token::RhaiCode(rhai_code) => {
                // if the rhai code doesn't parse, that doesn't stop us from continuing to parse the qat code
                let _ = rhai_macros.add_code(*rhai_code, &iter.r());
            }
            Token::EndOfEnclosure(encloser) => {
                assert!(encloser.is_none());

                parsed_syntax
                    .expansion_info
                    .rhai_macros
                    .insert(iter.file(), rhai_macros);

                return if ok { Some(parsed_syntax) } else { None };
            }
            _ => {
                return iter.unexpected(token, "an instruction, macro, import, or rhai block");
            }
        }
    }
}

fn registers(t: &mut TokenIter) -> Attempt<Option<WithSpan<RegistersDecl>>> {
    let marker = t.marker();

    t.attempt(|t, commit| {
        t.word(".registers")?;

        *commit = true;

        let decls = t.enclosure(Encloser::Brace)?.into_inner().parse(|t| {
            t.parse_list(
                |t, c| register_decl(t).c(c),
                |t, c| {
                    t.nl()?;
                    *c = true;
                    Some(())
                },
            )
        })?;

        Some(t.cash_in(marker).with(RegistersDecl {
            puzzles: decls.into_inner(),
        }))
    })
}

fn register_decl(t: &mut TokenIter) -> Attempt<Option<Puzzle>> {
    t.attempt(|t, commit| {
        let mut names = Vec::new();

        let start = t.marker();

        loop {
            let reg = t.reg()?;

            if !reg.is_plain() {
                t.report(Report::build(ReportKind::Error, reg.span()).with_label(Label::new(reg.span())
                    .with_message("Expected register name")).finish()
                );
            }

            names.push(reg.span().with(reg.value.reg_name.value));

            *commit = true;

            let token = t.next()?;
            match &*token {
                Token::Symbol(s) if *s == Symbol::Comma => {}
                Token::Symbol(s) if *s == Symbol::AssignArrow => break,
                _ => return t.unexpected(token, "a ',' followed by a register name or a '<-' followed by the architecture definition")
            }
        }

        let arch = register_architecture(t)?;

        let PuzzleUnnamed {
            architecture,
            def_span,
        } = arch;

        let span = architecture.span();
        let (arch, swizzle) = architecture.into_inner();

        if arch.registers().len() == names.len() {
            swizzle.apply(&mut names);

            Some(Puzzle {
                reg_names: names,
                arch: span.with(arch),
                puzzle_def: def_span
            })
        } else {
            t.report(
                Report::build(ReportKind::Error, t.cash_in(start))
                    .with_message(format!(
                        "Expected {} names whereas {} were provided.",
                        arch.registers().len(),
                        names.len()
                    ))
                    .finish(),
            );

            None
        }
    })
}

#[derive(Clone, Debug)]
struct PuzzleUnnamed {
    architecture: WithSpan<(Arc<Architecture>, Permutation)>,
    def_span: Span,
}

fn register_architecture(t: &mut TokenIter) -> Option<PuzzleUnnamed> {
    let start = t.marker();

    let puzzle_def = t.ident()?;

    let puzzle = match puzzle_def.parse::<PuzzleGeometry>() {
        Ok(v) => Some(v),
        Err(err) => {
            t.report(
                Report::build(ReportKind::Error, puzzle_def.span())
                    .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
                    .with_message(err.clone())
                    .with_label(
                        Label::new(puzzle_def.span())
                            .with_message(err)
                            .with_color(Color::Red),
                    )
                    .finish(),
            );
            None
        }
    };

    let def_span = puzzle_def.span();

    let group = puzzle?.permutation_group();

    arch(t, &group).map(|v| PuzzleUnnamed {
        architecture: t.cash_in(start).with(v),
        def_span,
    })
}

fn arch(
    t: &mut TokenIter,
    group: &Arc<PermutationGroup>,
) -> Option<(Arc<Architecture>, Permutation)> {
    let builtin = t.attempt(|t, commit| {
        t.word("builtin")?;
        *commit = true;

        let token = t.next()?;
        let orders = match &*token {
            Token::Number(num) => Box::from([*num]),
            Token::Enclosure(Encloser::Paren, token_enclosure) => (**token_enclosure)
                .clone()
                .parse(|t| {
                    let v = t.parse_list(
                        |t, commit| {
                            let num = t.number()?;
                            *commit = true;
                            Some(num.into_inner())
                        },
                        |t, commit| {
                            t.symbol(Symbol::Comma)?;
                            *commit = true;
                            Some(())
                        },
                    )?;

                    t.end_of_enclosure()?;

                    Some(v)
                })?
                .into_inner(),
            _ => {
                return t.unexpected(token, "an number or parenthezised list of numbers");
            }
        };

        with_presets(Arc::clone(group)).get_preset(&orders)
    });

    if let Attempt::Taken(v) = builtin {
        return v;
    }

    let algs = t.attempt(|t, commit| {
        let e = t.enclosure(Encloser::Paren)?;
        *commit = true;
        e.into_inner().parse(|t| {
            let v = t.parse_list(
                |t, commit| {
                    *commit = true;
                    parse_alg(t, group)
                },
                |t, commit| {
                    t.symbol(Symbol::Comma)?;
                    *commit = true;
                    Some(())
                },
            )?;

            t.end_of_enclosure()?;

            Some(v)
        })
    });

    if let Attempt::Taken(algs) = algs {
        let algs = algs?.into_inner();

        return Some((
            Arc::new(Architecture::new(Arc::clone(group), algs)),
            Permutation::identity(),
        ));
    }

    let alg = parse_alg(t, group)?;

    Some((
        Arc::new(Architecture::new(Arc::clone(group), Box::from([alg]))),
        Permutation::identity(),
    ))
}

fn parse_alg(t: &mut TokenIter, group: &Arc<PermutationGroup>) -> Option<Algorithm> {
    let mut spans = HashMap::<Intern<str>, Vec<Span>>::new();

    let move_seq = t.parse_list(
        |t, commit| {
            let token = t.next()?;
            let span = token.span();
            let ident = match token.into_inner() {
                Token::Ident(v) => v,
                Token::Register(v) => {
                    if !v.is_plain() {
                        return None;
                    }

                    v.reg_name.into_inner()
                }
                _ => return None,
            };
            spans.entry(ident).or_default().push(span);
            *commit = true;
            Some(ident)
        },
        |_, commit| {
            *commit = true;
            Some(())
        },
    )?;

    match Algorithm::new_from_move_seq(Arc::clone(group), move_seq.to_vec()) {
        Ok(v) => Some(v),
        Err(k) => {
            for span in spans.get(&k).unwrap() {
                t.report(
                    Report::build(ReportKind::Error, *span)
                        .with_message("Nonexistant move")
                        .with_help(format!(
                            "Valid options are {}",
                            group
                                .generators()
                                .sorted_by(|a, b| a.0.cmp(&b.0))
                                .format_with(", ", |v, f| f(&format_args!("`{}`", v.0)))
                        ))
                        .finish(),
                );
            }
            None
        }
    }
}

fn instruction(t: &mut TokenIter) -> Attempt<Option<WithSpan<Instruction>>> {
    let marker = t.marker();
    t.attempt(|t, commit| {
        *commit = true;
        let token = t.next()?;
        let span = token.span();
        match token.into_inner() {
            Token::Ident(name) => {
                if let Attempt::Taken(v) = t.attempt(|t, commit| {
                    if t.on_whitespace().is_some() {
                        return None;
                    }

                    t.symbol(Symbol::Colon)?;

                    *commit = true;

                    t.nl()?;

                    let (name, public) = match name.strip_prefix("!") {
                        Some(stripped) => (Intern::from_ref(stripped), true),
                        None => (name, false),
                    };

                    Some(Instruction::Label(crate::Label {
                        name,
                        public,
                        maybe_block_id: None,
                        branch_key: None,
                    }))
                }) {
                    v
                } else {
                    Some(if name.as_ref() == "rhai" {
                        Instruction::RhaiCall(rhai_call(t)?)
                    } else {
                        Instruction::Code(Code::Macro(crate::MacroCall {
                            name: span.with(name),
                            arguments: args(t)?,
                        }))
                    })
                }
            }
            Token::Directive(ident) if &*ident == "define" => Some(Instruction::Define(define(t)?)),
            Token::Constant(ident, mods) => {
                if !mods.is_plain() {
                    t.report(Report::build(ReportKind::Error, span).with_label(
                        Label::new(span).with_message(
                            format!("A constant used as an instruction cannot have register modifiers. Found a {}", mods.found()),
                        ),
                    ).finish());
                }

                Some(Instruction::Constant(ident))
            }
            Token::Enclosure(Encloser::Brace, enclosure) => Some(Instruction::Block(
                enclosure.into_inner().parse(block)?.value.value,
            )),
            _ => {
                *commit = false;
                None
            }
        }
        .map(|v| t.cash_in(marker).with(v))
    })
}

fn args(t: &mut TokenIter) -> Option<WithSpan<Vec<WithSpan<Value>>>> {
    let marker = t.marker();
    let mut args = Some(Vec::new());

    while let Attempt::NotTaken(_) = t.attempt(|t, commit| {
        t.nl()?;
        *commit = true;
        Some(())
    }) {
        let token = t.next()?;
        if let Some(v) = value(t, token) {
            if let Some(args) = &mut args {
                args.push(v);
            }
        } else {
            args = None;
        }
    }

    args.map(|args| t.cash_in(marker).with(args))
}

fn value(t: &TokenIter, token: WithSpan<Token>) -> Option<WithSpan<Value>> {
    let span = token.span();
    match &*token {
        Token::Ident(ident) => {
            Some(span.with(Value::Resolved(ResolvedValue::Ident(span.with(*ident)))))
        }
        Token::Register(r) => Some(span.with(Value::Resolved(ResolvedValue::Register(*r)))),
        Token::Constant(constant, mods) => Some(span.with(Value::Constant {
            name: *constant,
            mods: *mods,
        })),
        Token::Number(num) => Some(span.with(Value::Resolved(ResolvedValue::Int(*num)))),
        Token::Enclosure(Encloser::Brace, enclosure) => {
            (**enclosure).clone().parse(block).map(|v| {
                let block = v.value;

                block
                    .span()
                    .with(Value::Resolved(ResolvedValue::Block(block.value)))
            })
        }
        _ => {
            t.unexpected(token, "an argument for an instruction")?;
            None
        }
    }
}

fn block(t: &mut TokenIter) -> Option<WithSpan<Block>> {
    let mut code = Vec::new();

    loop {
        if let Attempt::Taken(v) = t.attempt(|t, commit| {
            let token = t.next()?;
            let span = token.span();
            match token.into_inner() {
                Token::EndOfEnclosure(_) => {
                    *commit = true;
                    Some(span.with(Block {
                        code: mem::take(&mut code),
                    }))
                }
                _ => None,
            }
        }) {
            return v;
        }

        code.push(match instruction(t) {
            Attempt::NotTaken(span) => {
                t.report(
                    Report::build(ReportKind::Error, span)
                        .with_message("Could not be parsed as an instruction")
                        .finish(),
                );
                return None;
            }
            Attempt::Taken(v) => {
                let v = v?;
                v.span().with((v.into_inner(), None, None))
            }
        });
    }
}

fn define(t: &mut TokenIter) -> Option<DefineUnresolved> {
    // Expects the `.define` to already be consumed

    let token = t.constant()?;
    let span = token.span();
    let (name, mods) = token.into_inner();

    if !mods.is_plain() {
        t.report(
            Report::build(ReportKind::Error, span)
                .with_label(Label::new(span).with_message(format!(
                    "Cannot use register modifiers in the name of a constant. Found a {}",
                    mods.found()
                )))
                .finish(),
        );
    }

    Some(
        match t.attempt(|t, commit| {
            let marker = t.marker();
            if t.word("rhai").is_some() {
                *commit = true;
                rhai_call(t).map(|v| t.cash_in(marker).with(v))
            } else {
                None
            }
        }) {
            Attempt::Taken(v) => DefineUnresolved {
                name: span.with(name),
                value: DefineValue::RhaiCall(v?),
            },
            Attempt::NotTaken(_) => {
                let token = t.next()?;

                return Some(DefineUnresolved {
                    name: span.with(name),
                    value: DefineValue::Value(value(t, token)?),
                });
            }
        },
    )
}

fn rhai_call(t: &mut TokenIter) -> Option<RhaiCall> {
    let name = t.ident()?;

    let args = t.enclosure(Encloser::Paren)?.into_inner().parse(|t| {
        t.parse_list(
            |t, commit| {
                let token = t.next()?;
                match &*token {
                    Token::EndOfEnclosure(_) => None,
                    _ => {
                        *commit = true;
                        value(t, token)
                    }
                }
            },
            |t, commit| {
                let token = t.next()?;
                match &*token {
                    Token::EndOfEnclosure(_) => None,
                    Token::Symbol(sym) if *sym == Symbol::Comma => {
                        *commit = true;
                        Some(())
                    }
                    _ => {
                        *commit = true;
                        t.unexpected(token, "a comma or closing parenthesis")?
                    }
                }
            },
        )
    })?;

    Some(RhaiCall {
        function_name: name,
        args: args.into_inner(),
    })
}

fn macro_def(
    t: &mut TokenIter,
    fresh_branch_key: impl Fn() -> MacroBranchKey,
) -> Option<(WithSpan<Intern<str>>, WithSpan<Macro>)> {
    let name = t.ident()?;

    let macro_def = t.enclosure(Encloser::Brace)?.into_inner().parse(|t| {
        let mut branches = Vec::<WithSpan<MacroBranch>>::new();

        while let Attempt::Taken(branch) = macro_branch(t, &fresh_branch_key) {
            let branch = branch?;

            for old_branch in &branches {
                if let Some(ce) = branch.pattern.conflicts_with(&name, &old_branch.pattern) {
                    t.report(
                        Report::build(ReportKind::Error, branch.span())
                            .with_message(format!(
                                "Macro branches are conflicting. `{ce}` would match both of them."
                            ))
                            .with_label(Label::new(old_branch.pattern.span()).with_message("First branch"))
                            .with_label(Label::new(branch.pattern.span()).with_message("Second branch"))
                            .finish(),
                    );
                }
            }

            branches.push(branch);
        }

        t.end_of_enclosure()?;

        Some(Macro::UserDefined { branches })
    })?;

    Some((name, macro_def))
}

fn macro_branch(
    t: &mut TokenIter,
    fresh_branch_key: impl Fn() -> MacroBranchKey,
) -> Attempt<Option<WithSpan<MacroBranch>>> {
    t.attempt(|t, commit| {
        let start = t.marker();

        let pattern = t.enclosure(Encloser::Paren)?.into_inner().parse(|t| {
            *commit = true;

            let mut pattern = Vec::new();

            while let Attempt::Taken(component) = macro_pattern_component(t) {
                pattern.push(component?);
            }

            t.end_of_enclosure()?;

            Some(MacroPattern(pattern))
        })?;

        t.symbol(Symbol::DefineArrow)?;

        let subst = match instruction(t) {
            Attempt::NotTaken(span) => {
                t.report(
                    Report::build(ReportKind::Error, span)
                        .with_message("Unable to parse as an instruction")
                        .finish(),
                );

                return None;
            }
            Attempt::Taken(v) => v?,
        };

        Some(
            t.cash_in(start).with(MacroBranch {
                pattern,
                code: subst
                    .span()
                    .with((subst.into_inner(), None, Some(fresh_branch_key()))),
            }),
        )
    })
}

fn macro_pattern_component(t: &mut TokenIter) -> Attempt<Option<WithSpan<MacroPatternComponent>>> {
    t.attempt(|t, commit| {
        let token = t.next()?;
        let span = token.span();
        match token.into_inner() {
            Token::Ident(word) => {
                *commit = true;
                Some(span.with(MacroPatternComponent::Word(word)))
            }
            Token::Constant(name, mods) => {
                *commit = true;

                if !mods.is_plain() {
                    t.report(
                        Report::build(ReportKind::Error, span)
                            .with_label(Label::new(span).with_message(format!(
                                "A macro argument name cannot have register modifiers. Found a {}",
                                mods.found()
                            )))
                            .finish(),
                    );
                }

                let marker = t.marker();

                if let Some(ws) = t.on_whitespace() {
                    t.report(
                        Report::build(ReportKind::Error, ws)
                            .with_message("Expected colon, found whitespace")
                            .finish(),
                    );
                    return None;
                }

                t.symbol(Symbol::Colon)?;

                let ty = t.one_of([
                    ("int", MacroArgTy::Int),
                    ("reg", MacroArgTy::Reg),
                    ("block", MacroArgTy::Block),
                    ("ident", MacroArgTy::Ident),
                ])?;

                Some(t.cash_in(marker).with(MacroPatternComponent::Argument {
                    name: span.with(name),
                    ty,
                }))
            }
            _ => None,
        }
    })
}

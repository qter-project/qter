use std::{
    collections::HashMap,
    mem,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
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
    reporter::{OrErrorProof, ReportToken},
    rhai::RhaiMacros,
};

fn new_branch_key() -> MacroBranchKey {
    static BRANCH_COUNT: AtomicU64 = AtomicU64::new(0);
    let num = BRANCH_COUNT.fetch_add(1, Ordering::Relaxed);
    MacroBranchKey(num)
}

pub fn parse(
    iter: &mut TokenIter,
    find_import: &Rc<impl Fn(&str) -> Result<Intern<str>, String> + 'static>,
    is_prelude: bool,
) -> OrErrorProof<ParsedSyntax> {
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
    };

    let code = Vec::new();

    let mut parsed_syntax = ParsedSyntax {
        expansion_info,
        code,
    };

    // Holds proof of an error that doesn't prevent us from continuing to parse
    let mut err: Option<ReportToken> = None;

    if !is_prelude
        && let Err(e) = super::merge_files(
            &mut parsed_syntax,
            iter.file(),
            super::PRELUDE.with(|v| (*v).clone()),
            Span::new(iter.file(), 0, iter.file().inner().chars().count()),
            &iter.r(),
        )
    {
        err = Some(e);
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

    let global_branch_key = new_branch_key();

    loop {
        let marker = iter.marker();

        if let Attempt::Taken(instr) = instruction(iter, global_branch_key) {
            let instr = instr?;
            let span = instr.span();
            parsed_syntax
                .code
                .push(span.with((instr.into_inner(), None, global_branch_key)));

            continue;
        }

        let token = iter.next()?;
        let span = token.span();

        match &*token {
            Token::Directive(directive) => {
                if directive.as_ref() == "macro" {
                    let (name, def) = macro_def(iter)?;

                    if parsed_syntax
                        .expansion_info
                        .macros
                        .contains_key(&(iter.file(), *name))
                    {
                        err = Some(iter.report(
                            Report::build(ReportKind::Error, name.span())
                                .with_label(Label::new(name.span()).with_message("This macro is already defined."))
                                .finish(),
                        ));
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
                        err = Some(iter.report(
                            Report::build(ReportKind::Error, filename.span())
                                .with_label(Label::new(filename.span()).with_message("The file extension must be `.qat`"))
                                .finish(),
                        ));
                        continue;
                    }

                    let import = match (find_import)(&filename.value) {
                        Ok(v) => v,
                        Err(e) => {
                            err = Some(iter.report(
                                Report::build(ReportKind::Error, filename.span())
                                    .with_label(Label::new(filename.span()).with_message(format!("Unable to find import: {e}")))
                                    .finish(),
                            ));

                            continue;
                        }
                    };

                    let find_import = Rc::clone(find_import);

                    let importee = match super::parse(
                        File::new(filename.value, import),
                        &find_import,
                        is_prelude,
                        &iter.r(),
                    ) {
                        Ok(v) => v,
                        Err(e) => {
                            err = Some(e);
                            continue;
                        }
                    };

                    if let Err(e) = super::merge_files(
                        &mut parsed_syntax,
                        iter.file(),
                        importee.value,
                        iter.cash_in(marker),
                        &iter.r(),
                    ) {
                        err = Some(e);
                    }
                } else {
                    return Err(iter.report(
                        Report::build(ReportKind::Error, span)
                            .with_label(Label::new(span).with_message("Unknown directive; expected .macro or .import"))
                            .finish(),
                    ));
                }
            }
            Token::RhaiCode(rhai_code) => {
                // if the rhai code doesn't parse, that doesn't stop us from continuing to parse the qat code
                if let Err(e) = rhai_macros.add_code(*rhai_code, &iter.r()) {
                    err = Some(e);
                }
            }
            Token::EndOfEnclosure(encloser) => {
                assert!(encloser.is_none());

                parsed_syntax
                    .expansion_info
                    .rhai_macros
                    .insert(iter.file(), rhai_macros);

                return match err {
                    None => Ok(parsed_syntax),
                    Some(e) => Err(e),
                };
            }
            _ => {
                return iter.unexpected(token, "an instruction, macro, import, or rhai block");
            }
        }
    }
}

fn registers(t: &mut TokenIter) -> Attempt<OrErrorProof<WithSpan<RegistersDecl>>> {
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
                    Ok(())
                },
            )
        })?;

        Ok(t.cash_in(marker).with(RegistersDecl {
            puzzles: decls.into_inner(),
        }))
    })
}

fn register_decl(t: &mut TokenIter) -> Attempt<OrErrorProof<Puzzle>> {
    t.attempt(|t, commit| {
        let mut names = Vec::new();

        let start = t.marker();

        loop {
            let reg = t.reg()?;

            if !reg.is_plain() {
                let _ = t.report(Report::build(ReportKind::Error, reg.span()).with_label(Label::new(reg.span())
                    .with_message("Expected register name")).finish()
                );
            }

            names.push(reg.span().with(reg.value.reg_name.value));

            *commit = true;

            let token = t.next()?;
            match &*token {
                Token::Symbol(s) if *s == Symbol::Comma => {}
                Token::Symbol(s) if *s == Symbol::AssignArrow => break,
                _ => return t.unexpected(token, "a ',' followed by a register name or a '<-' followed by the architecture definition").map_err(Some)
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

            Ok(Puzzle {
                reg_names: names,
                arch: span.with(arch),
                puzzle_def: def_span
            })
        } else {
            Err(Some(t.report(
                Report::build(ReportKind::Error, t.cash_in(start))
                    .with_label(Label::new(t.cash_in(start)).with_message(format!(
                        "Expected {} names whereas {} were provided.",
                        arch.registers().len(),
                        names.len()
                    )))
                    .finish(),
            )))
        }
    })
}

#[derive(Clone, Debug)]
struct PuzzleUnnamed {
    architecture: WithSpan<(Arc<Architecture>, Permutation)>,
    def_span: Span,
}

fn register_architecture(t: &mut TokenIter) -> OrErrorProof<PuzzleUnnamed> {
    let start = t.marker();

    let puzzle_def = t.ident()?;

    let puzzle = match puzzle_def.parse::<PuzzleGeometry>() {
        Ok(v) => Ok(v),
        Err(err) => Err(t.report(
                Report::build(ReportKind::Error, puzzle_def.span())
                    .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
                    .with_label(
                        Label::new(puzzle_def.span())
                            .with_message(err)
                            .with_color(Color::Red),
                    )
                    .finish(),
            )),
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
) -> OrErrorProof<(Arc<Architecture>, Permutation)> {
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
                            Ok(num.into_inner())
                        },
                        |t, commit| {
                            t.symbol(Symbol::Comma)?;
                            *commit = true;
                            Ok(())
                        },
                    )?;

                    t.end_of_enclosure()?;

                    Ok(v)
                })?
                .into_inner(),
            _ => {
                return t
                    .unexpected(token, "an number or parenthezised list of numbers")
                    .map_err(Some);
            }
        };

        match with_presets(Arc::clone(group)).get_preset(&orders) {
            Some(v) => Ok(v),
            None => Err(todo!()),
        }
    });

    if let Attempt::Taken(v) = builtin {
        return v;
    }

    let algs = t.attempt(|t, commit| {
        let e = t.enclosure(Encloser::Paren)?;
        *commit = true;
        e.into_inner()
            .parse(|t| {
                let v = t.parse_list(
                    |t, commit| {
                        *commit = true;
                        parse_alg(t, group).map_err(Some)
                    },
                    |t, commit| {
                        t.symbol(Symbol::Comma)?;
                        *commit = true;
                        Ok(())
                    },
                )?;

                t.end_of_enclosure()?;

                Ok(v)
            })
            .map_err(Some)
    });

    if let Attempt::Taken(algs) = algs {
        let algs = algs?.into_inner();

        return Ok((
            Arc::new(Architecture::new(Arc::clone(group), algs)),
            Permutation::identity(),
        ));
    }

    let alg = parse_alg(t, group)?;

    Ok((
        Arc::new(Architecture::new(Arc::clone(group), Box::from([alg]))),
        Permutation::identity(),
    ))
}

fn parse_alg(t: &mut TokenIter, group: &Arc<PermutationGroup>) -> OrErrorProof<Algorithm> {
    let mut spans = HashMap::<Intern<str>, Vec<Span>>::new();

    let move_seq = t.parse_list(
        |t, commit| {
            let token = t.next()?;
            let span = token.span();
            let ident = match token.into_inner() {
                Token::Ident(v) => v,
                Token::Register(v) => {
                    if !v.is_plain() {
                        return Err(None);
                    }

                    v.reg_name.into_inner()
                }
                _ => return Err(None),
            };
            spans.entry(ident).or_default().push(span);
            *commit = true;
            Ok(ident)
        },
        |_, commit| {
            *commit = true;
            Ok(())
        },
    )?;

    match Algorithm::new_from_move_seq(Arc::clone(group), move_seq.to_vec()) {
        Ok(v) => Ok(v),
        Err(k) => {
            let mut token = None;

            for span in spans.get(&k).unwrap() {
                token = Some(t.report(
                    Report::build(ReportKind::Error, *span)
                        .with_help(format!(
                            "Valid options are {}",
                            group
                                .generators()
                                .sorted_by(|a, b| a.0.cmp(&b.0))
                                .format_with(", ", |v, f| f(&format_args!("`{}`", v.0)))
                        ))
                        .with_label(Label::new(*span).with_message("Nonexistant move"))
                        .finish(),
                ));
            }

            Err(token.expect("the nonexistant move to have come from the parsed move sequence"))
        }
    }
}

fn instruction(
    t: &mut TokenIter,
    branch_key: MacroBranchKey,
) -> Attempt<OrErrorProof<WithSpan<Instruction>>> {
    let marker = t.marker();
    t.attempt(|t, commit| {
        *commit = true;
        let token = t.next()?;
        let span = token.span();
        match token.into_inner() {
            Token::Ident(name) => {
                if let Attempt::Taken(v) = t.attempt(|t, commit| {
                    if t.on_whitespace().is_some() {
                        return Err(None);
                    }

                    t.symbol(Symbol::Colon)?;

                    *commit = true;

                    t.nl()?;

                    let (name, public) = match name.strip_prefix("!") {
                        Some(stripped) => (Intern::from_ref(stripped), true),
                        None => (name, false),
                    };

                    Ok(Instruction::Label(crate::Label {
                        name,
                        public,
                        maybe_block_id: None,
                        branch_key,
                    }))
                }) {
                    v.map_err(Some)
                } else {
                    Ok(if name.as_ref() == "rhai" {
                        Instruction::RhaiCall(rhai_call(t, branch_key)?)
                    } else {
                        Instruction::Code(Code::Macro(crate::MacroCall {
                            name: span.with(name),
                            arguments: args(t, branch_key)?,
                        }))
                    })
                }
            }
            Token::Directive(ident) if &*ident == "define" => Ok(Instruction::Define(define(t, branch_key)?)),
            Token::Constant(ident, mods) => {
                if !mods.is_plain() {
                    let _ = t.report(Report::build(ReportKind::Error, span).with_label(
                        Label::new(span).with_message(
                            format!("A constant used as an instruction cannot have register modifiers. Found a {}", mods.found()),
                        ),
                    ).finish());
                }

                Ok(Instruction::Constant(ident))
            }
            Token::Enclosure(Encloser::Brace, enclosure) => Ok(Instruction::Block(
                enclosure.into_inner().parse(|t| block(t, branch_key))?.value.value,
            )),
            _ => {
                *commit = false;
                Err(None)
            }
        }
        .map(|v| t.cash_in(marker).with(v))
    })
}

fn args(t: &mut TokenIter, branch_key: MacroBranchKey) -> OrErrorProof<WithSpan<Vec<WithSpan<Value>>>> {
    let marker = t.marker();
    let mut args = Ok(Vec::new());

    while let Attempt::NotTaken(_) = t.attempt(|t, commit| {
        t.nl()?;
        *commit = true;
        Ok(())
    }) {
        let token = t.next()?;
        match value(t, token, branch_key) {
            Ok(v) => {
                if let Ok(args) = &mut args {
                    args.push(v);
                }
            }
            Err(e) => args = Err(e),
        }
    }

    args.map(|args| t.cash_in(marker).with(args))
}

fn value(
    t: &TokenIter,
    token: WithSpan<Token>,
    branch_key: MacroBranchKey,
) -> OrErrorProof<WithSpan<Value>> {
    let span = token.span();
    match &*token {
        Token::Ident(ident) => {
            Ok(span.with(Value::Resolved(ResolvedValue::Ident(span.with(*ident)))))
        }
        Token::Register(r) => Ok(span.with(Value::Resolved(ResolvedValue::Register(*r)))),
        Token::Constant(constant, mods) => Ok(span.with(Value::Constant {
            name: *constant,
            mods: *mods,
        })),
        Token::Number(num) => Ok(span.with(Value::Resolved(ResolvedValue::Int(*num)))),
        Token::Enclosure(Encloser::Brace, enclosure) => (**enclosure)
            .clone()
            .parse(|t| block(t, branch_key))
            .map(|v| {
                let block = v.value;

                block
                    .span()
                    .with(Value::Resolved(ResolvedValue::Block(block.value)))
            }),
        _ => t.unexpected(token, "an argument for an instruction"),
    }
}

fn block(t: &mut TokenIter, branch_key: MacroBranchKey) -> OrErrorProof<WithSpan<Block>> {
    let mut code = Vec::new();

    loop {
        if let Attempt::Taken(v) = t.attempt(|t, commit| {
            let token = t.next()?;
            let span = token.span();
            match token.into_inner() {
                Token::EndOfEnclosure(_) => {
                    *commit = true;
                    Ok(span.with(Block {
                        code: mem::take(&mut code),
                    }))
                }
                _ => Err(None),
            }
        }) {
            return v;
        }

        code.push(match instruction(t, branch_key) {
            Attempt::NotTaken(span) => {
                return Err(t.report(
                    Report::build(ReportKind::Error, span)
                        .with_label(Label::new(span).with_message("Could not be parsed as an instruction"))
                        .finish(),
                ));
            }
            Attempt::Taken(v) => {
                let v = v?;
                v.span().with((v.into_inner(), None, branch_key))
            }
        });
    }
}

fn define(t: &mut TokenIter, branch_key: MacroBranchKey) -> OrErrorProof<DefineUnresolved> {
    // Expects the `.define` to already be consumed

    let token = t.constant()?;
    let span = token.span();
    let (name, mods) = token.into_inner();

    if !mods.is_plain() {
        let _ = t.report(
            Report::build(ReportKind::Error, span)
                .with_label(Label::new(span).with_message(format!(
                    "Cannot use register modifiers in the name of a constant. Found a {}",
                    mods.found()
                )))
                .finish(),
        );
    }

    Ok(
        match t.attempt(|t, commit| {
            let marker = t.marker();
            if t.word("rhai").is_ok() {
                *commit = true;
                rhai_call(t, branch_key)
                    .map(|v| t.cash_in(marker).with(v))
                    .map_err(Some)
            } else {
                Err(None)
            }
        }) {
            Attempt::Taken(v) => DefineUnresolved {
                name: span.with(name),
                value: DefineValue::RhaiCall(v?),
            },
            Attempt::NotTaken(_) => {
                let token = t.next()?;

                return Ok(DefineUnresolved {
                    name: span.with(name),
                    value: DefineValue::Value(value(t, token, branch_key)?),
                });
            }
        },
    )
}

fn rhai_call(t: &mut TokenIter, branch_key: MacroBranchKey) -> OrErrorProof<RhaiCall> {
    let name = t.ident()?;

    let args = t.enclosure(Encloser::Paren)?.into_inner().parse(|t| {
        t.parse_list(
            |t, commit| {
                let token = t.next()?;
                match &*token {
                    Token::EndOfEnclosure(_) => Err(None),
                    _ => {
                        *commit = true;
                        value(t, token, branch_key).map_err(Some)
                    }
                }
            },
            |t, commit| {
                let token = t.next()?;
                match &*token {
                    Token::EndOfEnclosure(_) => Err(None),
                    Token::Symbol(sym) if *sym == Symbol::Comma => {
                        *commit = true;
                        Ok(())
                    }
                    _ => {
                        *commit = true;
                        t.unexpected(token, "a comma or closing parenthesis").map_err(Some)
                    }
                }
            },
        )
    })?;

    Ok(RhaiCall {
        function_name: name,
        args: args.into_inner(),
    })
}

fn macro_def(t: &mut TokenIter) -> OrErrorProof<(WithSpan<Intern<str>>, WithSpan<Macro>)> {
    let name = t.ident()?;

    let macro_def = t.enclosure(Encloser::Brace)?.into_inner().parse(|t| {
        let mut branches = Vec::<WithSpan<MacroBranch>>::new();

        while let Attempt::Taken(branch) = macro_branch(t) {
            let branch = branch?;

            for old_branch in &branches {
                if let Some(ce) = branch.pattern.conflicts_with(&name, &old_branch.pattern) {
                    let _ = t.report(
                        Report::build(ReportKind::Error, branch.span())
                            .with_message(format!(
                                "Macro branches are conflicting. `{ce}` would match both of them."
                            ))
                            .with_label(
                                Label::new(old_branch.pattern.span()).with_message("First branch"),
                            )
                            .with_label(
                                Label::new(branch.pattern.span()).with_message("Second branch"),
                            )
                            .finish(),
                    );
                }
            }

            branches.push(branch);
        }

        t.end_of_enclosure()?;

        Ok(Macro::UserDefined { branches })
    })?;

    Ok((name, macro_def))
}

fn macro_branch(t: &mut TokenIter) -> Attempt<OrErrorProof<WithSpan<MacroBranch>>> {
    t.attempt(|t, commit| {
        let start = t.marker();

        let pattern = t.enclosure(Encloser::Paren)?.into_inner().parse(|t| {
            *commit = true;

            let mut pattern = Vec::new();

            while let Attempt::Taken(component) = macro_pattern_component(t) {
                pattern.push(component?);
            }

            t.end_of_enclosure()?;

            Ok(MacroPattern(pattern))
        })?;

        t.symbol(Symbol::DefineArrow)?;

        let branch_key = new_branch_key();

        let subst = match instruction(t, branch_key) {
            Attempt::NotTaken(span) => {
                return Err(Some(t.report(
                    Report::build(ReportKind::Error, span)
                        .with_label(Label::new(span).with_message("Unable to parse as an instruction"))
                        .finish(),
                )));
            }
            Attempt::Taken(v) => v?,
        };

        Ok(t.cash_in(start).with(MacroBranch {
            pattern,
            code: subst.span().with((subst.into_inner(), None, branch_key)),
        }))
    })
}

fn macro_pattern_component(t: &mut TokenIter) -> Attempt<OrErrorProof<WithSpan<MacroPatternComponent>>> {
    t.attempt(|t, commit| {
        let token = t.next()?;
        let span = token.span();
        match token.into_inner() {
            Token::Ident(word) => {
                *commit = true;
                Ok(span.with(MacroPatternComponent::Word(word)))
            }
            Token::Constant(name, mods) => {
                *commit = true;

                if !mods.is_plain() {
                    let _ = t.report(
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
                    return Err(Some(t.report(
                        Report::build(ReportKind::Error, ws)
                            .with_label(Label::new(ws).with_message("Expected colon, found whitespace"))
                            .finish(),
                    )));
                }

                t.symbol(Symbol::Colon)?;

                let ty = t.one_of([
                    ("int", MacroArgTy::Int),
                    ("reg", MacroArgTy::Reg),
                    ("block", MacroArgTy::Block),
                    ("ident", MacroArgTy::Ident),
                ])?;

                Ok(t.cash_in(marker).with(MacroPatternComponent::Argument {
                    name: span.with(name),
                    ty,
                }))
            }
            _ => Err(None),
        }
    })
}

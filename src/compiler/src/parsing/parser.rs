use std::{
    collections::HashMap,
    mem,
    rc::Rc,
    sync::{Arc, OnceLock, atomic::AtomicUsize},
};

use ariadne::{Color, Label, Report, ReportKind};
use internment::ArcIntern;
use itertools::Itertools;
use puzzle_theory::{
    numbers::{Int, U},
    permutations::{Algorithm, Permutation, PermutationGroup},
    puzzle_geometry::PuzzleGeometry,
    span::{File, Span, WithSpan},
};
use qter_core::architectures::{Architecture, with_presets};
use rhai::ParseError;

use crate::{
    Block, BlockID, BlockInfo, BlockInfoTracker, Code, DefineUnresolved, DefineValue,
    ExpansionInfo, Instruction, Macro, MacroArgTy, MacroBranch, MacroBranchKey, MacroPattern,
    MacroPatternComponent, ParsedSyntax, Puzzle, RegistersDecl, Reporter, ResolvedValue, RhaiCall,
    Value,
    parsing::tokenizer::{Attempt, Encloser, Symbol, TokenIter, TokenNL, TokenW},
    rhai::RhaiMacros,
};

use super::tokenizer::Token;

pub fn parse(
    iter: &mut TokenIter,
    find_import: impl Fn(&str) -> Result<ArcIntern<str>, String> + 'static,
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
            Span::new(iter.file().clone(), 0, iter.file().inner().len()),
            iter.r(),
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

    let find_import = Rc::new(find_import);

    loop {
        let marker = iter.marker();

        if let Attempt::Taken(instr) = instruction(iter) {
            let instr = instr?;
            let span = instr.span().clone();
            parsed_syntax
                .code
                .push(span.with((instr.into_inner(), None, None)));

            continue;
        }

        let TokenW { token, reporter } = iter.next()?;
        match token {
            Token::Directive(ident) if &**ident == "macro" => {
                let (name, def) = macro_def(iter, parsed_syntax.expansion_info.fresh_branch_key())?;

                if parsed_syntax
                    .expansion_info
                    .macros
                    .contains_key(&(iter.file().clone(), ArcIntern::clone(&name)))
                {
                    iter.report(
                        Report::build(ReportKind::Error, name.span().clone())
                            .with_message("This macro is already defined.")
                            .finish(),
                    );
                    continue;
                }

                parsed_syntax
                    .expansion_info
                    .macros
                    .insert((iter.file().clone(), ArcIntern::clone(&name)), def);
                parsed_syntax.expansion_info.available_macros.insert(
                    (iter.file().clone(), name.into_inner()),
                    iter.file().clone(),
                );
            }
            Token::Directive(ident) if &**ident == "import" => {
                let filename = iter.next_nl()?.token("a file to import")?.ident()?;

                if !filename.ends_with(".qat") {
                    iter.report(
                        Report::build(ReportKind::Error, filename.span().clone())
                            .with_message("The file extension must be `.qat`")
                            .finish(),
                    );
                    continue;
                }

                let import = match (find_import)(&filename.value) {
                    Ok(v) => v,
                    Err(e) => {
                        iter.report(
                            Report::build(ReportKind::Error, filename.span().clone())
                                .with_message(format!("Unable to find import: {e}"))
                                .finish(),
                        );

                        continue;
                    }
                };

                let find_import = Rc::clone(&find_import);

                let Some(importee) = super::parse(
                    &File::new(filename.value, import),
                    move |v| (find_import)(v),
                    is_prelude,
                    iter.r(),
                ) else {
                    continue;
                };

                super::merge_files(
                    &mut parsed_syntax,
                    iter.file(),
                    importee.value,
                    iter.cash_in(marker),
                    iter.r(),
                );
            }
            Token::Directive(ident) if &**ident == "start-rhai" => {
                let (code, pos_to_span) = iter.take_rhai()?;

                if let Err(ParseError(err, pos)) = rhai_macros.add_code(code.slice()) {
                    let (span, default) = match pos_to_span(pos) {
                        Some(span) => (span, false),
                        None => (ident.span().clone(), true),
                    };

                    let mut report =
                        Report::build(ReportKind::Error, span).with_message(err.to_string());

                    if default {
                        report =
                            report.with_note("The Rhai compiler did not provide span information");
                    }

                    iter.report(report.finish());
                }
            }
            Token::EndOfEnclosure(encloser, _) => {
                assert!(encloser.is_none());

                parsed_syntax
                    .expansion_info
                    .rhai_macros
                    .insert(iter.file().clone(), rhai_macros);

                return Some(parsed_syntax);
            }
            t => {
                return TokenW { token: t, reporter }
                    .unexpected("an instruction, macro, import, or rhai block");
            }
        }
    }
}

fn registers(t: &mut TokenIter) -> Attempt<Option<WithSpan<RegistersDecl>>> {
    let marker = t.marker();

    t.attempt(|t, commit| {
        t.next()?.word(".registers")?;

        *commit = true;

        let decls = t.next()?.enclosure(Encloser::Brace)?.parse(|t| {
            let mut decls = Vec::new();

            while !t.is_empty() {
                decls.push(register_decl(t)?);
            }

            Some(decls)
        })?;

        Some(t.cash_in(marker).with(RegistersDecl {
            puzzles: decls.into_inner(),
        }))
    })
}

fn register_decl(t: &mut TokenIter) -> Option<Puzzle> {
    let mut names = Vec::new();

    let start = t.marker();

    loop {
        names.push(t.next()?.ident()?);

        let tokenw = t.next()?;
        match tokenw.token {
            Token::Symbol(s) if *s == Symbol::Comma => {}
            Token::Symbol(s) if *s == Symbol::AssignArrow => break,
            _ => return tokenw.unexpected("a ',' followed by a register name or a '<-' followed by the architecture definition")
        }
    }

    let arch = register_architecture(t)?;

    match arch {
        PuzzleUnnamed::Theoretical { order } => {
            if names.len() == 1 {
                Some(Puzzle::Theoretical {
                    name: names.pop().unwrap(),
                    order,
                })
            } else {
                t.report(
                    Report::build(ReportKind::Error, t.cash_in(start))
                        .with_message(format!(
                            "Expected one register name whereas {} were provided.",
                            names.len()
                        ))
                        .finish(),
                );

                None
            }
        }
        PuzzleUnnamed::Real {
            architecture,
            def_span,
        } => {
            let span = architecture.span().clone();
            let (arch, swizzle) = architecture.into_inner();

            if arch.registers().len() == names.len() {
                swizzle.apply(&mut names);

                Some(Puzzle::Real {
                    architectures: vec![(names, span.with(arch), def_span)],
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
        }
    }
}

#[derive(Clone, Debug)]
enum PuzzleUnnamed {
    Theoretical {
        order: WithSpan<Int<U>>,
    },
    Real {
        architecture: WithSpan<(Arc<Architecture>, Permutation)>,
        def_span: Span,
    },
}

fn register_architecture(t: &mut TokenIter) -> Option<PuzzleUnnamed> {
    let start = t.marker();

    let puzzle_def = t.next()?.ident()?;

    if &**puzzle_def == "theoretical" {
        let order = t.next()?.number()?;
        t.next_nl()?.nl()?;

        return Some(PuzzleUnnamed::Theoretical { order });
    }

    let puzzle = match puzzle_def.parse::<PuzzleGeometry>() {
        Ok(v) => Some(v),
        Err(errs) => {
            for err in errs {
                t.report(
                    Report::build(ReportKind::Error, err.span().clone())
                        .with_config(
                            ariadne::Config::new().with_index_type(ariadne::IndexType::Byte),
                        )
                        .with_message(err.to_string())
                        .with_label(
                            Label::new(err.span().clone())
                                .with_message(err.reason().to_string())
                                .with_color(Color::Red),
                        )
                        .finish(),
                );
            }
            None
        }
    };

    let def_span = puzzle_def.span().clone();

    let tokenw = t.next()?;
    let arch = match tokenw.token {
        Token::Ident(ident) => {
            if &**ident == "builtin" {
                let tokenw = t.next()?;
                let orders = match tokenw.token {
                    Token::Number(num) => vec![*num],
                    Token::Enclosure(Encloser::Paren, token_enclosure) => token_enclosure
                        .parse(|t| {
                            let mut out = Vec::new();

                            loop {
                                let tokenw = t.next()?;
                                let num = match tokenw.token {
                                    Token::Number(num) => *num,
                                    Token::EndOfEnclosure(_, _) => return Some(out),
                                    _ => return tokenw.unexpected("a number or ')'"),
                                };

                                out.push(num);

                                let tokenw = t.next()?;
                                match tokenw.token {
                                    Token::Symbol(sym) if *sym == Symbol::Comma => {}
                                    Token::EndOfEnclosure(_, _) => return Some(out),
                                    _ => return tokenw.unexpected("a comma or ')'"),
                                }
                            }
                        })?
                        .into_inner(),
                    _ => {
                        return tokenw.unexpected(
                            "an algorithm, parenthezised list of algorithms, or `builtin`",
                        );
                    }
                };

                Some(
                    t.cash_in(start)
                        .with(with_presets(puzzle?.permutation_group()).get_preset(&orders)?),
                )
            } else {
                let group = puzzle?.permutation_group();

                let mut alg = Some(Algorithm::identity(Arc::clone(&group)));

                try_append(&mut alg, ident, &group, t.r());

                loop {
                    let tokenw = t.next_nl()?;
                    match tokenw.token {
                        TokenNL::NewLine(_) => {
                            break alg.map(|alg| {
                                t.cash_in(start).with((
                                    Arc::new(Architecture::new(group, vec![alg].into())),
                                    Permutation::identity(),
                                ))
                            });
                        }
                        TokenNL::Token(Token::Ident(turn)) => {
                            try_append(&mut alg, turn, &group, t.r())
                        }
                        TokenNL::Token(token) => {
                            return TokenW {
                                token,
                                reporter: tokenw.reporter,
                            }
                            .unexpected("a move or a line break");
                        }
                    }
                }
            }
        }
        Token::Enclosure(Encloser::Paren, token_enclosure) => token_enclosure.parse(|t| {
            let group = puzzle?.permutation_group();

            let mut algs = Some(Vec::new());

            let mut alg = Some(Algorithm::identity(Arc::clone(&group)));

            loop {
                let tokenw = t.next()?;
                match tokenw.token {
                    Token::Ident(turn) => try_append(&mut alg, turn, &group, t.r()),
                    Token::Symbol(sym) if *sym == Symbol::Comma => {
                        if let (Some(algs), Some(alg)) = (&mut algs, alg) {
                            algs.push(alg);
                        }

                        alg = Some(Algorithm::identity(Arc::clone(&group)));
                    }
                    Token::EndOfEnclosure(_, _) => {
                        break algs.map(|algs| {
                            (
                                Arc::new(Architecture::new(group, algs.into())),
                                Permutation::identity(),
                            )
                        });
                    }
                    _ => return tokenw.unexpected("a move, a comma, or a ')'"),
                }
            }
        }),
        _ => tokenw.unexpected("an algorithm, parenthezised list of algorithms, or `builtin`"),
    };

    arch.map(|v| PuzzleUnnamed::Real {
        architecture: v,
        def_span,
    })
}

fn try_append(
    alg: &mut Option<Algorithm>,
    turn: WithSpan<ArcIntern<str>>,
    group: &Arc<PermutationGroup>,
    r: Reporter,
) {
    let span = turn.span().clone();

    match Algorithm::new_from_move_seq(Arc::clone(group), vec![turn.into_inner()]) {
        Ok(alg2) => {
            if let Some(alg) = alg {
                alg.compose_into(&alg2);
            }
        }
        Err(_) => {
            r.push(
                Report::build(ReportKind::Error, span)
                    .with_message("This move is not a member of the specified puzzle.")
                    .with_help(format!(
                        "Valid options are {}",
                        group
                            .generators()
                            .sorted_by(|a, b| a.0.cmp(&b.0))
                            .format_with(", ", |v, f| f(&format_args!("`{}`", v.0)))
                    ))
                    .finish(),
            );

            *alg = None;
        }
    }
}

fn instruction(t: &mut TokenIter) -> Attempt<Option<WithSpan<Instruction>>> {
    let marker = t.marker();
    t.attempt(|t, commit| {
        *commit = true;
        let tokenw = t.next()?;
        match tokenw.token {
            Token::Ident(name) => {
                if let Attempt::Taken(v) = t.attempt(|t, commit| {
                    if t.whitespace().is_some() {
                        return None;
                    }

                    t.next()?.symbol(Symbol::Colon)?;

                    *commit = true;

                    t.next_nl()?.nl()?;

                    let (name, public) = if name.value.starts_with('!') {
                        (ArcIntern::from(&name.value[1..]), true)
                    } else {
                        (name.value.clone(), false)
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
                    Some(Instruction::Code(Code::Macro(crate::MacroCall {
                        name,
                        arguments: args(t)?,
                    })))
                }
            }
            Token::Directive(ident) if &**ident == "define" => {
                Some(Instruction::Define(define(t)?))
            }
            Token::Constant(ident) => Some(Instruction::Constant(ident.value)),
            Token::Enclosure(Encloser::Brace, enclosure) => Some(Instruction::Block(
                enclosure.parse(|t| block(t))?.value.value,
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
    let mut args = Vec::new();

    loop {
        let v = t.attempt(|t, commit| {
            *commit = true;
            let tokenw = t.next_nl()?;
            match tokenw.token {
                TokenNL::NewLine(_) | TokenNL::Token(Token::EndOfEnclosure(_, _)) => {
                    *commit = false;
                }
                TokenNL::Token(token) => match value(token) {
                    Ok(v) => args.push(v?),
                    Err(token) => TokenW {
                        token,
                        reporter: tokenw.reporter,
                    }
                    .unexpected("an argument for an instruction")?,
                },
            };
            Some(())
        });

        match v {
            Attempt::Taken(v) => v?,
            Attempt::NotTaken(_) => return Some(t.cash_in(marker).with(args)),
        }
    }
}

fn value<'a>(t: Token<'a>) -> Result<Option<WithSpan<Value>>, Token<'a>> {
    Ok(match t {
        Token::Ident(ident) => Some(ident.span().clone().with(Value::Resolved(
            ResolvedValue::Ident {
                ident,
                as_reg: OnceLock::new(),
            },
        ))),
        Token::Constant(constant) => Some(
            constant
                .span()
                .clone()
                .with(Value::Constant(constant.into_inner())),
        ),
        Token::Number(num) => Some(
            num.span()
                .clone()
                .with(Value::Resolved(ResolvedValue::Int(*num))),
        ),
        Token::Enclosure(Encloser::Brace, enclosure) => enclosure.parse(|t| block(t)).map(|v| {
            let block = v.value;

            block
                .span()
                .clone()
                .with(Value::Resolved(ResolvedValue::Block(block.value)))
        }),
        _ => return Err(t),
    })
}

fn block(t: &mut TokenIter) -> Option<WithSpan<Block>> {
    let mut code = Vec::new();

    loop {
        if let Attempt::Taken(v) = t.attempt(|t, commit| {
            let tokenw = t.next()?;
            match tokenw.token {
                Token::EndOfEnclosure(_, span) => {
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
                v.span().clone().with((v.into_inner(), None, None))
            }
        });
    }
}

fn define(t: &mut TokenIter) -> Option<DefineUnresolved> {
    // Expects the `.define` to already be consumed

    let name = t
        .next_nl()?
        .token("the name for a `define` statement")?
        .ident();

    Some(
        match t.attempt(|t, commit| {
            let marker = t.marker();
            if let Token::Ident(v) = t.next()?.token
                && &**v == "rhai"
            {
                *commit = true;
                rhai_call(t).map(|v| t.cash_in(marker).with(v))
            } else {
                None
            }
        }) {
            Attempt::Taken(v) => DefineUnresolved {
                name: name?,
                value: DefineValue::RhaiCall(v?),
            },
            Attempt::NotTaken(_) => {
                let TokenW { token, reporter } =
                    t.next_nl()?.token("the value of a `define` statement")?;

                match value(token) {
                    Ok(Some(v)) => {
                        return Some(DefineUnresolved {
                            name: name?,
                            value: DefineValue::Value(v),
                        });
                    }
                    Ok(None) => return None,
                    Err(token) => TokenW { token, reporter }
                        .unexpected("a constant, identifier, number, or code block")?,
                }
            }
        },
    )
}

fn rhai_call(t: &mut TokenIter) -> Option<RhaiCall> {
    let name = t.next_nl()?.token("a rhai function to call")?.ident()?;

    let args = t
        .next_nl()?
        .token("an arguments list")?
        .enclosure(Encloser::Paren)?
        .parse(|t| {
            let mut values = Vec::new();

            loop {
                let TokenW { token, reporter } = t.next()?;
                match token {
                    Token::EndOfEnclosure(_, _) => return Some(values),
                    v => match value(v) {
                        Ok(Some(v)) => values.push(v),
                        Ok(None) => return None,
                        Err(t) => TokenW { token: t, reporter }
                            .unexpected("a constant, identifier, number, or code block")?,
                    },
                }

                let tokenw = t.next()?;
                match tokenw.token {
                    Token::EndOfEnclosure(_, _) => return Some(values),
                    Token::Symbol(sym) if *sym == Symbol::Comma => continue,
                    _ => tokenw.unexpected("a comma or closing parenthesis")?,
                }
            }
        })?;

    Some(RhaiCall {
        function_name: name,
        args: args.into_inner(),
    })
}

fn macro_def(
    t: &mut TokenIter,
    fresh_branch_key: impl Fn() -> MacroBranchKey,
) -> Option<(WithSpan<ArcIntern<str>>, WithSpan<Macro>)> {
    let name = t.next()?.ident()?;

    let macro_def = t.next()?.enclosure(Encloser::Brace)?.parse(|t| {
        let mut branches = Vec::new();

        while let Attempt::Taken(branch) = macro_branch(t, &fresh_branch_key) {
            branches.push(branch?)
        }

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

        let pattern = t.next()?.enclosure(Encloser::Paren)?.parse(|t| {
            *commit = true;

            let mut pattern = Vec::new();

            while let Attempt::Taken(component) = macro_pattern_component(t) {
                pattern.push(component?)
            }

            Some(MacroPattern(pattern))
        })?;

        t.next()?.symbol(Symbol::DefineArrow)?;

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

        Some(t.cash_in(start).with(
            MacroBranch {
                pattern,
                code: subst.span().clone().with((
                    subst.into_inner(),
                    None,
                    Some(fresh_branch_key()),
                )),
            },
        ))
    })
}

fn macro_pattern_component(t: &mut TokenIter) -> Attempt<Option<WithSpan<MacroPatternComponent>>> {
    t.attempt(|t, commit| {
        match t.next()?.token {
            Token::Ident(word) => {
                *commit = true;
                Some(
                    word.span()
                        .clone()
                        .with(MacroPatternComponent::Word(word.into_inner())),
                )
            }
            Token::Constant(name) => {
                *commit = true;

                let marker = t.marker();

                if let Some(ws) = t.whitespace() {
                    t.report(
                        Report::build(ReportKind::Error, ws)
                            .with_message("Expected colon, found whitespace")
                            .finish(),
                    );
                    return None;
                }

                t.next()?.symbol(Symbol::Colon)?;

                let ty = t
                    .next()?
                    .one_of([
                        ("int", MacroArgTy::Int),
                        ("reg", MacroArgTy::Reg),
                        ("block", MacroArgTy::Block),
                        ("ident", MacroArgTy::Ident),
                    ])
                    .map(|(ty, span)| span.with(ty))?;

                Some(
                    t.cash_in(marker)
                        .with(MacroPatternComponent::Argument { name, ty }),
                )
            }
            _ => None,
        }
    })
}

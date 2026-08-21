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
    MacroPatternComponent, ParsedSyntax, Puzzle, RegistersDecl, ResolvedValue, RhaiCall, Value,
    parsing::tokenizer::{Attempt, Encloser, Symbol, TokenIter},
    rhai::RhaiMacros,
};

use super::tokenizer::Token;

pub fn parse(
    iter: &mut TokenIter,
    find_import: &Rc<impl Fn(&str) -> Result<ArcIntern<str>, String> + 'static>,
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

        let token = iter.next()?;
        let span = token.span();

        match &*token {
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
                let filename = iter.ident()?;

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

                let find_import = Rc::clone(find_import);

                let Some(importee) = super::parse(
                    &File::new(filename.value, import),
                    &find_import,
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
                    &iter.r(),
                );
            }
            Token::RhaiCode(rhai_code) => {
                if let Err(ParseError(err, pos)) = rhai_macros.add_code(rhai_code.span().slice()) {
                    let (span, default) = match rhai_code.pos_to_span(pos) {
                        Some(span) => (span, false),
                        None => (span.clone(), true),
                    };

                    let mut report = Report::build(ReportKind::Error, span.clone())
                        .with_message(err.to_string())
                        .with_label(Label::new(span).with_color(Color::Red));

                    if default {
                        report =
                            report.with_note("The Rhai compiler did not provide span information");
                    }

                    iter.report(report.finish());
                }
            }
            Token::EndOfEnclosure(encloser) => {
                assert!(encloser.is_none());

                parsed_syntax
                    .expansion_info
                    .rhai_macros
                    .insert(iter.file().clone(), rhai_macros);

                return Some(parsed_syntax);
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
            names.push(t.ident()?);

            *commit = true;

            let token = t.next()?;
            match &*token {
                Token::Symbol(s) if *s == Symbol::Comma => {}
                Token::Symbol(s) if *s == Symbol::AssignArrow => break,
                _ => return t.unexpected(token, "a ',' followed by a register name or a '<-' followed by the architecture definition")
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
    })
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

    let puzzle_def = t.ident()?;

    if &**puzzle_def == "theoretical" {
        let order = t.number()?;

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

    let group = puzzle?.permutation_group();

    arch(t, &group).map(|v| PuzzleUnnamed::Real {
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
                    t.parse_list(
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
                    )
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
            t.parse_list(
                |t, commit| {
                    *commit = true;
                    parse_alg(t, group)
                },
                |t, commit| {
                    t.symbol(Symbol::Comma)?;
                    *commit = true;
                    Some(())
                },
            )
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
    let mut spans = HashMap::<ArcIntern<str>, Vec<Span>>::new();

    let move_seq = t.parse_list(
        |t, commit| {
            let ident = t.ident()?;
            spans
                .entry((*ident).clone())
                .or_default()
                .push(ident.span().clone());
            *commit = true;
            Some(ident.into_inner())
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
                    Report::build(ReportKind::Error, span.clone())
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
        let span = token.span().clone();
        match token.into_inner() {
            Token::Ident(name) => {
                if let Attempt::Taken(v) = t.attempt(|t, commit| {
                    if t.whitespace().is_some() {
                        return None;
                    }

                    t.symbol(Symbol::Colon)?;

                    *commit = true;

                    t.nl()?;

                    let (name, public) = match name.strip_prefix("!") {
                        Some(stripped) => (ArcIntern::from(stripped), true),
                        None => (name.clone(), false),
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
                    Some(if name == "rhai" {
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
            Token::Constant(ident) => Some(Instruction::Constant(ident)),
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
    let span = token.span().clone();
    match &*token {
        Token::Ident(ident) => Some(span.clone().with(Value::Resolved(ResolvedValue::Ident {
            ident: span.with(ident.clone()),
            as_reg: OnceLock::new(),
        }))),
        Token::Constant(constant) => Some(span.with(Value::Constant(constant.clone()))),
        Token::Number(num) => Some(span.with(Value::Resolved(ResolvedValue::Int(*num)))),
        Token::Enclosure(Encloser::Brace, enclosure) => {
            (**enclosure).clone().parse(block).map(|v| {
                let block = v.value;

                block
                    .span()
                    .clone()
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
            let span = token.span().clone();
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
                v.span().clone().with((v.into_inner(), None, None))
            }
        });
    }
}

fn define(t: &mut TokenIter) -> Option<DefineUnresolved> {
    // Expects the `.define` to already be consumed

    let name = t.ident();

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
                name: name?,
                value: DefineValue::RhaiCall(v?),
            },
            Attempt::NotTaken(_) => {
                let token = t.next()?;

                return Some(DefineUnresolved {
                    name: name?,
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
) -> Option<(WithSpan<ArcIntern<str>>, WithSpan<Macro>)> {
    let name = t.ident()?;

    let macro_def = t.enclosure(Encloser::Brace)?.into_inner().parse(|t| {
        let mut branches = Vec::new();

        while let Attempt::Taken(branch) = macro_branch(t, &fresh_branch_key) {
            branches.push(branch?);
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

        let pattern = t.enclosure(Encloser::Paren)?.into_inner().parse(|t| {
            *commit = true;

            let mut pattern = Vec::new();

            while let Attempt::Taken(component) = macro_pattern_component(t) {
                pattern.push(component?);
            }

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
        let token = t.next()?;
        let span = token.span().clone();
        match token.into_inner() {
            Token::Ident(word) => {
                *commit = true;
                Some(span.with(MacroPatternComponent::Word(word)))
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

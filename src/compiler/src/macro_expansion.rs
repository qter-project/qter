use std::{cell::OnceCell, collections::HashMap, mem};

use ariadne::{Report, ReportKind};
use internment::ArcIntern;
use itertools::Itertools;
use puzzle_theory::span::{Span, WithSpan};

use crate::{
    BlockID, Code, Define, ExpandedCode, ExpandedCodeComponent, ExpansionInfo, Instruction, Macro,
    MacroBranchKey, ParsedSyntax, RegistersDecl, Reporter, ResolvedValue, TaggedInstruction,
    resolve_just_these_defines, tag_with_key,
};

pub fn expand(mut parsed: ParsedSyntax, r: Reporter) -> Option<ExpandedCode> {
    let branch_key_fn = parsed.expansion_info.fresh_branch_key();

    for macro_ in &mut parsed.expansion_info.macros {
        let Macro::UserDefined { branches } = &mut macro_.1.value else {
            continue;
        };

        for branch in branches {
            let key = branch_key_fn();

            tag_with_key(&mut branch.code, Some(key));
        }
    }

    let mut limit = 100;

    let before = r.count();

    while let Some(span) =
        expand_block(BlockID(0), &mut parsed.expansion_info, &mut parsed.code, &r)
    {
        limit -= 1;

        if limit == 0 {
            r.push(
                Report::build(ReportKind::Error, span)
                    .with_message("Depth limit reached during macro expansion")
                    .finish(),
            );
            return None;
        }
    }

    if r.count() - before != 0 {
        return None;
    }

    Some(ExpandedCode {
        registers: match parsed.expansion_info.registers {
            Some(decl) => decl.into_inner(),
            None => RegistersDecl {
                puzzles: Vec::new(),
            },
        },
        block_info: parsed.expansion_info.block_info,
        expanded_code_components: parsed
            .code
            .into_iter()
            .map(|tagged_instruction| {
                let span = tagged_instruction.span().to_owned();
                // We can ignore branch keys since expansion should have inserted them into label references
                let (instruction, maybe_block_id, _) = tagged_instruction.into_inner();

                let expanded = match instruction {
                    Instruction::Label(label) => ExpandedCodeComponent::Label(label),
                    Instruction::Code(Code::Primitive(primitive)) => {
                        ExpandedCodeComponent::Instruction(
                            Box::new(primitive),
                            maybe_block_id.unwrap(),
                        )
                    }
                    illegal => unreachable!("{illegal:?}"),
                };

                WithSpan::new(expanded, span)
            })
            .collect_vec(),
    })
}

/// Returns whether any changes were made
fn expand_block(
    block_id: BlockID,
    expansion_info: &mut ExpansionInfo,
    code: &mut Vec<WithSpan<TaggedInstruction>>,
    r: &Reporter,
) -> Option<Span> {
    // Will be set if anything is ever changed
    let mut changed = OnceCell::<Span>::new();

    *code = (mem::take(code)
        .into_iter()
        .map(|mut tagged_instruction| {
            let maybe_block_id = &mut tagged_instruction.1;
            if maybe_block_id.is_none() {
                *maybe_block_id = Some(block_id);
                let _ = changed.set(tagged_instruction.span().clone());
            }

            tagged_instruction
        })
        .map(|tagged_instruction| {
            let span = tagged_instruction.span().to_owned();

            let (instruction, maybe_block_id, maybe_branch_key) = tagged_instruction.into_inner();
            let block_id = maybe_block_id.unwrap();

            let block_info = expansion_info.block_info.blocks.get_mut(&block_id).unwrap();

            match instruction {
                Instruction::Label(mut label) => {
                    if label.maybe_block_id.is_none() {
                        label.maybe_block_id = Some(block_id);
                        let _ = changed.set(span.clone());
                    }

                    block_info.labels.push(label.clone());

                    vec![WithSpan::new(
                        (Instruction::Label(label), maybe_block_id, maybe_branch_key),
                        span,
                    )]
                }
                Instruction::Define(define) => {
                    if block_info.defines.contains_key(&define.name) {
                        r.push(Report::build(ReportKind::Error, span).with_message("Cannot shadow a `.define` in the same scope!").finish());
                        return vec![]
                    }

                    let resolved = match expansion_info.resolve(define.value, block_id, r) {
                        Some(v) => v,
                        None => return vec![],
                    };

                    let new_define = Define {
                        name: define.name,
                        value: resolved,
                    };

                    expansion_info
                        .block_info
                        .blocks
                        .get_mut(&block_id)
                        .unwrap()
                        .defines
                        .insert(ArcIntern::clone(&new_define.name), new_define);
                    let _ = changed.set(span);

                    vec![]
                }
                Instruction::Code(code) => {
                    expand_code(block_id, expansion_info, code, span, &changed, maybe_branch_key, r)
                }
                Instruction::Constant(name) => {
                    match expansion_info.block_info.get_define(block_id, &name) {
                        Some(define) => match &*define.value {
                            ResolvedValue::Int(_) => {
                                r.push(Report::build(ReportKind::Error, span).with_message("Expected a code block, found an integer").finish());
                                vec![]
                            },
                            ResolvedValue::Ident {
                                ident: _,
                                as_reg: _,
                            } => {
                                r.push(Report::build(ReportKind::Error, span).with_message("Expected a code block, found an identifier").finish());
                                vec![]
                            },
                            ResolvedValue::Block(block) => {
                                let _ = changed.set(span);

                                let block = block.clone();

                                let (new_id, _) = expansion_info.block_info.new_block(block_id);

                                block
                                    .code
                                    .into_iter()
                                    .map(|mut v| {
                                        v.1 = Some(new_id);
                                        v
                                    })
                                    .collect_vec()
                            }
                        },
                        None => {
                            r.push(Report::build(ReportKind::Error, span).with_message(format!("`{name}` was not found in this scope")).finish());
                            vec![]
                        }
                    }
                }
                Instruction::RhaiCall(call) => {
                    let value = match call.perform(span.clone(), expansion_info, block_id, r) {
                        Some(v) => v,
                        None => return vec![],
                    };
                    let _ = changed.set(span.clone());

                    match value.into_inner() {
                        ResolvedValue::Int(_) => {
                            r.push(Report::build(ReportKind::Error, span).with_message("Expected the macro to return a code block; actually returned an integer").finish());
                            vec![]
                        },
                        ResolvedValue::Ident { ident: _, as_reg: _ } => {
                            r.push(Report::build(ReportKind::Error, span).with_message("Expected the macro to return a code block; actually returned an identifier").finish());
                            vec![]
                        },
                        ResolvedValue::Block(block) => {
                            let _ = changed.set(span);

                            block.code.clone()
                        },
                    }
                }
                Instruction::Block(block) => {
                    let (new_id, _) = expansion_info.block_info.new_block(block_id);
                    let _ = changed.set(span);

                    block
                        .code
                        .into_iter()
                        .map(|mut v| {
                            v.1 = Some(new_id);
                            v
                        })
                        .collect_vec()
                },
            }
        })
    ).flatten()
    .collect_vec()
        ;

    changed.take()
}

fn expand_code(
    block_id: BlockID,
    expansion_info: &mut ExpansionInfo,
    code: Code,
    span: Span,
    changed: &OnceCell<Span>,
    maybe_branch_key: Option<MacroBranchKey>,
    r: &Reporter,
) -> Vec<WithSpan<TaggedInstruction>> {
    let macro_call = match code {
        Code::Primitive(prim) => {
            return vec![span.with((
                Instruction::Code(Code::Primitive(prim)),
                Some(block_id),
                maybe_branch_key,
            ))];
        }
        Code::Macro(mac) => mac,
    };

    let _ = changed.set(span.clone());

    let Some(macro_access) = expansion_info.available_macros.get(&(
        macro_call.name.span().source().clone(),
        ArcIntern::clone(&*macro_call.name),
    )) else {
        r.push(
            Report::build(ReportKind::Error, macro_call.name.span().clone())
                .with_message("Macro was not found in this scope")
                .finish(),
        );
        return vec![];
    };

    let macro_def = expansion_info
        .macros
        .get(&(macro_access.clone(), ArcIntern::clone(&macro_call.name)))
        .unwrap();

    match &**macro_def {
        Macro::UserDefined { branches } => {
            let args_span = macro_call.arguments.span().clone();
            let Some(mut args) = macro_call
                .arguments
                .into_inner()
                .into_iter()
                .map(|v| {
                    let span = v.span().clone();
                    match expansion_info.block_info.resolve(block_id, v.into_inner()) {
                        Some(v) => Some(span.with(v)),
                        None => {
                            r.push(
                                Report::build(ReportKind::Error, macro_call.name.span().clone())
                                    .with_message("Constant was not found in this scope")
                                    .finish(),
                            );
                            None
                        }
                    }
                })
                .collect::<Option<Vec<_>>>()
            else {
                return vec![];
            };

            for branch in branches {
                let defines = match branch.pattern.matches(args, expansion_info) {
                    Ok(v) => v.into_iter().collect::<HashMap<_, _>>(),
                    Err(returned_args) => {
                        args = returned_args;
                        continue;
                    }
                };

                let (block_id, _) = expansion_info.block_info.new_block(block_id);

                let mut instr = branch.code.clone();
                instr.1 = Some(block_id);
                resolve_just_these_defines(&mut instr, &defines, r);

                return vec![instr];
            }

            r.push(
                Report::build(ReportKind::Error, args_span)
                    .with_message("These arguments did not match any of the patterns of this macro")
                    .finish(),
            );
            vec![]
        }
        Macro::Builtin(macro_fn) => {
            match macro_fn(expansion_info, macro_call.arguments, block_id, r) {
                Some(v) => vec![span.with((
                    Instruction::Code(Code::Primitive(v.insert_branch_key(maybe_branch_key))),
                    Some(block_id),
                    maybe_branch_key,
                ))],
                None => vec![],
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use std::rc::Rc;
use std::sync::Arc;

    use crate::Reporter;
    use crate::parsing::tests::file;
    use crate::{macro_expansion::expand, parsing::parse};

    #[test]
    fn bruh() {
        let code = "
            .registers {
                a, b ← 3x3 builtin (90, 90)
            }

            loop:
                add a 1
                print \"What da heck\" a
                solved-goto a loop

                add b 89
                solved-goto b over
                goto loop

            over:

                halt Poggers b
        ";

        let reporter = Reporter::default();

        let parsed = parse(
            &file(code),
            Rc::new(|_: &str| unreachable!()),
            false,
            Arc::clone(&reporter),
        )
        .unwrap();

        let expanded = expand(parsed.into_inner(), Arc::clone(&reporter)).unwrap();

        let reports = Arc::try_unwrap(reporter).unwrap();
        assert_eq!(reports.count(), 0);

        println!("{expanded:?}");
    }
}

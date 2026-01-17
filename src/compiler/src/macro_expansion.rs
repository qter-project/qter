use std::{cell::OnceCell, mem};

use chumsky::error::Rich;
use internment::ArcIntern;
use itertools::{Either, Itertools};
use puzzle_theory::span::{Span, WithSpan};

use crate::{
    BlockID, Code, Define, ExpandedCode, ExpandedCodeComponent, ExpansionInfo, Instruction, Macro,
    ParsedSyntax, RegistersDecl, ResolvedValue, TaggedInstruction,
};

pub fn expand(mut parsed: ParsedSyntax) -> Result<ExpandedCode, Vec<Rich<'static, char, Span>>> {
    let mut errs = Vec::new();

    let mut limit = 100;

    while let Some(span) = expand_block(
        BlockID(0),
        &mut parsed.expansion_info,
        &mut parsed.code,
        &mut errs,
    ) {
        limit -= 1;

        if limit == 0 {
            errs.push(Rich::custom(span, "Recursion limit reached during macro expansion"));
            return Err(errs);
        }
    }

    if !errs.is_empty() {
        return Err(errs);
    }

    Ok(ExpandedCode {
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
                let (instruction, maybe_block_id) = tagged_instruction.into_inner();

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
    errs: &mut Vec<Rich<'static, char, Span>>,
) -> Option<Span> {
    // Will be set if anything is ever changed
    let mut changed = OnceCell::<Span>::new();

    let (new_code, new_errs) = mem::take(code)
        .into_iter()
        .map(|mut tagged_instruction| {
            let maybe_block_id = &mut tagged_instruction.1;
            if maybe_block_id.is_none() {
                *maybe_block_id = Some(block_id);
                let _ = changed.set(tagged_instruction.span().clone());
            }

            tagged_instruction
        })
        .flat_map(|tagged_instruction| {
            let span = tagged_instruction.span().to_owned();

            let (instruction, maybe_block_id) = tagged_instruction.into_inner();
            let block_id = maybe_block_id.unwrap();

            let block_info = expansion_info.block_info.blocks.get_mut(&block_id).unwrap();

            match instruction {
                Instruction::Label(mut label) => {
                    if label.maybe_block_id.is_none() {
                        label.maybe_block_id = Some(block_id);
                        let _ = changed.set(span.clone());
                    }

                    block_info.labels.push(label.clone());

                    vec![Ok(WithSpan::new(
                        (Instruction::Label(label), maybe_block_id),
                        span,
                    ))]
                }
                Instruction::Define(define) => {
                    if block_info.defines.contains_key(&define.name) {
                        return vec![Err(Rich::custom(
                            define.name.span().clone(),
                            "Cannot shadow a `.define` in the same scope!",
                        ))];
                    }

                    let resolved = match expansion_info.resolve(define.value, block_id) {
                        Ok(v) => v,
                        Err(errs) => return errs.into_iter().map(Err).collect_vec(),
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
                    match expand_code(block_id, expansion_info, code, &changed) {
                        Ok(tagged_instructions) => tagged_instructions
                            .into_iter()
                            .map(|tagged_instruction| {
                                Ok(WithSpan::new(tagged_instruction, span.clone()))
                            })
                            .collect_vec(),
                        Err(e) => vec![Err(e)],
                    }
                }
                Instruction::Constant(name) => {
                    match expansion_info.block_info.get_define(block_id, &name) {
                        Some(define) => match &*define.value {
                            ResolvedValue::Int(_) => vec![Err(Rich::custom(
                                span,
                                "Expected a code block, found an integer",
                            ))],
                            ResolvedValue::Ident(_) => vec![Err(Rich::custom(
                                span,
                                "Expected a code block, found an identifier",
                            ))],
                            ResolvedValue::Block(block) => {
                                let _ = changed.set(span);

                                let block = block.clone();

                                let (new_id, _) =
                                    expansion_info.block_info.new_block(block_id);

                                block
                                    .code
                                    .into_iter()
                                    .map(|mut v| {
                                        v.1 = Some(new_id);
                                        Ok(v)
                                    })
                                    .collect_vec()
                            }
                        },
                        None => {
                            vec![Err(Rich::custom(
                                span,
                                format!("`{name}` was not found in this scope"),
                            ))]
                        }
                    }
                }
                Instruction::LuaCall(_) => todo!(),
            }
        })
        .partition_map::<Vec<_>, Vec<_>, _, _, _>(|res| match res {
            Ok(v) => Either::Left(v),
            Err(e) => Either::Right(e),
        });

    errs.extend_from_slice(&new_errs);
    *code = new_code;

    changed.take()
}

fn expand_code(
    block_id: BlockID,
    expansion_info: &mut ExpansionInfo,
    code: Code,
    changed: &OnceCell<Span>,
) -> Result<Vec<TaggedInstruction>, Rich<'static, char, Span>> {
    let macro_call = match code {
        Code::Primitive(prim) => {
            return Ok(vec![(
                Instruction::Code(Code::Primitive(prim)),
                Some(block_id),
            )]);
        }
        Code::Macro(mac) => mac,
    };

    let _ = changed.set(macro_call.name.span().clone());

    let Some(macro_access) = expansion_info.available_macros.get(&(
        macro_call.name.span().source().clone(),
        ArcIntern::clone(&*macro_call.name),
    )) else {
        return Err(Rich::custom(
            macro_call.name.span().clone(),
            "Macro was not found in this scope",
        ));
    };

    let macro_def = expansion_info
        .macros
        .get(&(
            ArcIntern::clone(macro_access),
            ArcIntern::clone(&macro_call.name),
        ))
        .unwrap();

    Ok(match &**macro_def {
        Macro::UserDefined {
            branches: _,
            after: _,
        } => todo!(),
        Macro::Builtin(macro_fn) => macro_fn(expansion_info, macro_call.arguments, block_id)?
            .into_iter()
            .map(|instruction| (instruction, Some(block_id)))
            .collect_vec(),
    })
}

#[cfg(test)]
mod tests {

    use puzzle_theory::span::File;

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

        let parsed = match parse(&File::from(code), |_| unreachable!(), false) {
            Ok(v) => v,
            Err(e) => panic!("{e:?}"),
        };

        let expanded = match expand(parsed) {
            Ok(v) => v,
            Err(e) => panic!("{e:?}"),
        };

        println!("{expanded:?}");
    }
}

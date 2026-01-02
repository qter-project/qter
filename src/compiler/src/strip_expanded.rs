use std::{collections::HashMap, sync::Arc};

use chumsky::error::Rich;
use internment::ArcIntern;
use itertools::{Either, Itertools};
use puzzle_theory::{numbers::{Int, U}, permutations::PermutationGroup, span::{Span, WithSpan}};
use qter_core::{
    ByPuzzleType, Facelets, Halt, Input, Instruction, Print, Program, PuzzleIdx,
    RegisterGenerator, RepeatUntil, SeparatesByPuzzleType, StateIdx, TheoreticalIdx, 
    architectures::{Architecture, CycleGeneratorSubcycle, new_from_effect},
};

use crate::{
    ExpandedCode, ExpandedCodeComponent, LabelReference, Primitive, Puzzle, RegisterReference,
    optimization::{OptimizingCodeComponent, OptimizingPrimitive, do_optimization},
};

pub(super) struct RegisterIdx;

impl SeparatesByPuzzleType for RegisterIdx {
    type Theoretical<'s> = ();

    type Puzzle<'s> = (usize, Arc<Architecture>, Option<Int<U>>);
}

pub struct GlobalRegs {
    register_table: HashMap<ArcIntern<str>, ByPuzzleType<'static, (StateIdx, RegisterIdx)>>,
    theoretical: Vec<WithSpan<Int<U>>>,
    puzzles: Vec<WithSpan<Arc<PermutationGroup>>>,
}

impl GlobalRegs {
    pub(super) fn get_reg(
        &self,
        reference: &RegisterReference,
    ) -> ByPuzzleType<'static, (StateIdx, RegisterIdx)> {
        let mut reg = self
            .register_table
            .get(&reference.reg_name)
            .unwrap()
            .to_owned();

        if let Some(mod_) = reference.modulus {
            match &mut reg {
                ByPuzzleType::Theoretical(_) => todo!(),
                ByPuzzleType::Puzzle((_, (_, _, modulus))) => *modulus = Some(mod_),
            }
        }

        reg
    }

    fn generator(
        &self,
        register: &RegisterReference,
    ) -> Result<ByPuzzleType<'static, (StateIdx, RegisterGenerator)>, Rich<'static, char, Span>>
    {
        let reg_info = self.get_reg(register);

        match reg_info {
            ByPuzzleType::Theoretical((theoretical, ())) => {
                Ok(ByPuzzleType::Theoretical((theoretical, ())))
            }
            ByPuzzleType::Puzzle((puzzle_idx, (idx, arch, modulus))) => Ok(ByPuzzleType::Puzzle((
                puzzle_idx,
                (
                    new_from_effect(&arch, vec![(idx, Int::<U>::one())]),
                    get_facelets(idx, &arch, modulus, register)?,
                ),
            ))),
        }
    }

    fn facelets(
        &self,
        register: &RegisterReference,
    ) -> Result<ByPuzzleType<'_, FaceletsInfo>, Rich<'static, char, Span>> {
        let reg_info = self.get_reg(register);

        match reg_info {
            ByPuzzleType::Theoretical((theoretical_idx, ())) => {
                Ok(ByPuzzleType::Theoretical(theoretical_idx))
            }
            ByPuzzleType::Puzzle((puzzle_idx, (idx, arch, modulus))) => Ok(ByPuzzleType::Puzzle((
                puzzle_idx,
                get_facelets(idx, &arch, modulus, register)?,
            ))),
        }
    }
}

fn get_facelets(
    idx: usize,
    arch: &Architecture,
    modulus: Option<Int<U>>,
    register: &RegisterReference,
) -> Result<Facelets, Rich<'static, char, Span>> {
    match modulus {
        Some(modulus) => {
            if let Some(v) = arch.registers()[idx].signature_facelets_mod(modulus) {
                Ok(v)
            } else {
                let cycles = arch.registers()[idx]
                    .unshared_cycles()
                    .iter()
                    .map(CycleGeneratorSubcycle::chromatic_order)
                    .sorted()
                    .dedup()
                    .collect_vec();

                Err(Rich::custom(
                    register.reg_name.span().clone(),
                    format!(
                        "Could not find a set of pieces for solved-goto that encode the given modulus. The available moduli are the LCM of any combination of the following piece subcycles: {}",
                        cycles.into_iter().join(", ")
                    ),
                ))
            }
        }
        None => Ok(arch.registers()[idx].signature_facelets()),
    }
}

struct FaceletsInfo;

impl SeparatesByPuzzleType for FaceletsInfo {
    type Theoretical<'s> = TheoreticalIdx;

    type Puzzle<'s> = (PuzzleIdx, Facelets);
}

pub fn strip_expanded(expanded: ExpandedCode) -> Result<Program, Vec<Rich<'static, char, Span>>> {
    let mut global_regs = GlobalRegs {
        register_table: HashMap::new(),
        theoretical: vec![],
        puzzles: vec![],
    };

    for puzzle in &expanded.registers.puzzles {
        match puzzle {
            Puzzle::Theoretical { name, order } => {
                global_regs.register_table.insert(
                    ArcIntern::clone(name),
                    ByPuzzleType::Theoretical((TheoreticalIdx(global_regs.theoretical.len()), ())),
                );

                global_regs.theoretical.push(order.to_owned());
            }
            Puzzle::Real { architectures } => {
                // TODO: Support for architecture switching
                // Just take the first architecture
                let (names, architecture, puzzle_span) = &architectures[0];
                for (i, name) in names.iter().enumerate() {
                    global_regs.register_table.insert(
                        ArcIntern::clone(name),
                        ByPuzzleType::Puzzle((
                            PuzzleIdx(global_regs.puzzles.len()),
                            (i, Arc::clone(architecture), None),
                        )),
                    );
                }

                global_regs.puzzles.push(WithSpan::new(
                    architecture.group_arc(),
                    puzzle_span.clone(),
                ));
            }
        }
    }

    let global_regs = Arc::new(global_regs);
    let global_regs_for_iter = Arc::clone(&global_regs);

    let instructions_iter = expanded.expanded_code_components.into_iter().map(move |v| {
        v.map(|v| match v {
            ExpandedCodeComponent::Instruction(primitive, block_id) => {
                OptimizingCodeComponent::Instruction(
                    Box::new(match *primitive {
                        Primitive::Add { amt, register } => {
                            match global_regs_for_iter.get_reg(&register) {
                                ByPuzzleType::Theoretical((theoretical, ())) => {
                                    OptimizingPrimitive::AddTheoretical { theoretical, amt }
                                }
                                ByPuzzleType::Puzzle((puzzle, (reg_idx, arch, modulus))) => {
                                    OptimizingPrimitive::AddPuzzle {
                                        puzzle,
                                        arch,
                                        amts: vec![(reg_idx, modulus, amt)],
                                    }
                                }
                            }
                        }
                        Primitive::Goto { label } => OptimizingPrimitive::Goto { label },
                        Primitive::SolvedGoto { label, register } => {
                            OptimizingPrimitive::SolvedGoto { label, register }
                        }
                        Primitive::Input { message, register } => {
                            OptimizingPrimitive::Input { message, register }
                        }
                        Primitive::Halt { message, register } => {
                            OptimizingPrimitive::Halt { message, register }
                        }
                        Primitive::Print { message, register } => {
                            OptimizingPrimitive::Print { message, register }
                        }
                    }),
                    block_id,
                )
            }
            ExpandedCodeComponent::Label(label) => OptimizingCodeComponent::Label(label),
        })
    });

    let optimized = do_optimization(instructions_iter, &global_regs);

    let mut program_counter = 0;

    let mut label_locations = HashMap::new();

    let instructions = optimized
        .into_iter()
        .filter_map(|component| {
            let span = component.span().to_owned();

            match component.into_inner() {
                OptimizingCodeComponent::Instruction(primitive, _) => {
                    program_counter += 1;
                    Some(primitive)
                }
                OptimizingCodeComponent::Label(label) => {
                    label_locations.insert(
                        LabelReference {
                            name: label.name,
                            block_id: label.maybe_block_id.unwrap(),
                        },
                        program_counter,
                    );
                    None
                }
            }
            .map(|v| WithSpan::new(v, span))
        })
        .collect_vec();

    let (instructions, errors) = instructions
        .into_iter()
        .map(|fully_simplified| {
            let span = fully_simplified.span().to_owned();

            let instruction = match *fully_simplified.into_inner() {
                OptimizingPrimitive::AddPuzzle { puzzle, arch, amts } => {
                    Instruction::PerformAlgorithm(ByPuzzleType::Puzzle((
                        puzzle,
                        new_from_effect(
                            &arch,
                            amts.into_iter()
                                .map(|(idx, _, amt)| (idx, amt.into_inner()))
                                .collect(),
                        ),
                    )))
                }
                OptimizingPrimitive::AddTheoretical { theoretical, amt } => {
                    Instruction::PerformAlgorithm(ByPuzzleType::Theoretical((theoretical, *amt)))
                }
                OptimizingPrimitive::Goto { label } => {
                    let Some(label) = expanded.block_info.label_scope(&label) else {
                        return Err(Rich::custom(
                            label.span().clone(),
                            "Could not find label in scope",
                        ));
                    };

                    Instruction::Goto {
                        instruction_idx: *label_locations.get(&label).unwrap(),
                    }
                }
                OptimizingPrimitive::SolvedGoto { register, label } => {
                    let Some(label) = expanded.block_info.label_scope(&label) else {
                        return Err(Rich::custom(
                            label.span().clone(),
                            "Could not find label in scope",
                        ));
                    };

                    let facelets = global_regs.facelets(&register)?;

                    let solved_goto = qter_core::SolvedGoto {
                        instruction_idx: *label_locations.get(&label).unwrap(),
                    };

                    Instruction::SolvedGoto(match facelets {
                        ByPuzzleType::Theoretical(theoretical_idx) => {
                            ByPuzzleType::Theoretical((solved_goto, theoretical_idx))
                        }
                        ByPuzzleType::Puzzle((puzzle_idx, facelets)) => {
                            ByPuzzleType::Puzzle((solved_goto, puzzle_idx, facelets))
                        }
                    })
                }
                OptimizingPrimitive::RepeatUntil {
                    puzzle,
                    arch,
                    amts,
                    register,
                } => Instruction::RepeatUntil(ByPuzzleType::Puzzle(RepeatUntil {
                    puzzle_idx: puzzle,
                    facelets: match global_regs.facelets(&register)? {
                        ByPuzzleType::Theoretical(_) => unreachable!(),
                        ByPuzzleType::Puzzle((idx, facelets)) => {
                            assert_eq!(idx, puzzle);
                            facelets
                        }
                    },
                    alg: new_from_effect(
                        &arch,
                        amts.into_iter()
                            .map(|(idx, _, amt)| (idx, amt.into_inner()))
                            .collect(),
                    ),
                })),
                OptimizingPrimitive::Solve { puzzle } => Instruction::Solve(match puzzle {
                    ByPuzzleType::Theoretical(idx) => ByPuzzleType::Theoretical(idx),
                    ByPuzzleType::Puzzle(idx) => ByPuzzleType::Puzzle(idx),
                }),
                OptimizingPrimitive::Input { message, register } => {
                    let input = Input {
                        message: message.into_inner(),
                    };

                    Instruction::Input(match global_regs.generator(&register)? {
                        ByPuzzleType::Theoretical((theoretical, ())) => {
                            ByPuzzleType::Theoretical((input, theoretical))
                        }
                        ByPuzzleType::Puzzle((puzzle_idx, (generator, solved_goto_facelets))) => {
                            ByPuzzleType::Puzzle((
                                input,
                                puzzle_idx,
                                generator,
                                solved_goto_facelets,
                            ))
                        }
                    })
                }
                OptimizingPrimitive::Halt { message, register } => {
                    let halt = Halt {
                        message: message.into_inner(),
                    };
                    Instruction::Halt(match register {
                        Some(register) => match global_regs.generator(&register)? {
                            ByPuzzleType::Theoretical((theoretical_idx, ())) => {
                                ByPuzzleType::Theoretical((halt, Some(theoretical_idx)))
                            }
                            ByPuzzleType::Puzzle((
                                puzzle_idx,
                                (generator, solved_goto_facelets),
                            )) => ByPuzzleType::Puzzle((
                                halt,
                                Some((puzzle_idx, generator, solved_goto_facelets)),
                            )),
                        },
                        None => ByPuzzleType::Puzzle((halt, None)),
                    })
                }
                OptimizingPrimitive::Print { message, register } => {
                    let print = Print {
                        message: message.into_inner(),
                    };
                    Instruction::Print(match register {
                        Some(register) => match global_regs.generator(&register)? {
                            ByPuzzleType::Theoretical((theoretical_idx, ())) => {
                                ByPuzzleType::Theoretical((print, Some(theoretical_idx)))
                            }
                            ByPuzzleType::Puzzle((
                                puzzle_idx,
                                (generator, solved_goto_facelets),
                            )) => ByPuzzleType::Puzzle((
                                print,
                                Some((puzzle_idx, generator, solved_goto_facelets)),
                            )),
                        },
                        None => ByPuzzleType::Puzzle((print, None)),
                    })
                }
            };

            Ok(WithSpan::new(instruction, span))
        })
        .partition_map::<Vec<_>, Vec<_>, _, _, _>(|res| match res {
            Ok(v) => Either::Left(v),
            Err(e) => Either::Right(e),
        });

    if !errors.is_empty() {
        return Err(errors);
    }

    let global_regs = Arc::into_inner(global_regs).unwrap();

    Ok(Program {
        theoretical: global_regs.theoretical,
        puzzles: global_regs.puzzles,
        instructions,
    })
}

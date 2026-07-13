use std::{collections::HashMap, sync::Arc};

use ariadne::{Report, ReportKind};
use internment::ArcIntern;
use itertools::Itertools;
use puzzle_theory::{
    numbers::{Int, U},
    permutations::PermutationGroup,
    span::WithSpan,
};
use qter_core::{
    ByPuzzleType, Facelets, Halt, Input, Instruction, Print, Program, PuzzleIdx, RegisterGenerator,
    RepeatUntil, SeparatesByPuzzleType, StateIdx, TheoreticalIdx,
    architectures::{Architecture, CycleGeneratorSubcycle, new_from_effect},
};

use crate::{
    ExpandedCode, ExpandedCodeComponent, LabelReference, Primitive, Puzzle, RegisterReference,
    Reporter,
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
        r: &Reporter,
    ) -> Option<ByPuzzleType<'static, (StateIdx, RegisterGenerator)>> {
        let reg_info = self.get_reg(register);

        match reg_info {
            ByPuzzleType::Theoretical((theoretical, ())) => {
                Some(ByPuzzleType::Theoretical((theoretical, ())))
            }
            ByPuzzleType::Puzzle((puzzle_idx, (idx, arch, modulus))) => {
                Some(ByPuzzleType::Puzzle((
                    puzzle_idx,
                    (
                        new_from_effect(&arch, vec![(idx, Int::<U>::one())]),
                        get_facelets(idx, &arch, modulus, register, r)?,
                    ),
                )))
            }
        }
    }

    fn facelets(
        &self,
        register: &RegisterReference,
        r: &Reporter,
    ) -> Option<ByPuzzleType<'_, FaceletsInfo>> {
        let reg_info = self.get_reg(register);

        match reg_info {
            ByPuzzleType::Theoretical((theoretical_idx, ())) => {
                Some(ByPuzzleType::Theoretical(theoretical_idx))
            }
            ByPuzzleType::Puzzle((puzzle_idx, (idx, arch, modulus))) => Some(ByPuzzleType::Puzzle(
                (puzzle_idx, get_facelets(idx, &arch, modulus, register, r)?),
            )),
        }
    }
}

fn get_facelets(
    idx: usize,
    arch: &Architecture,
    modulus: Option<Int<U>>,
    register: &RegisterReference,
    r: &Reporter,
) -> Option<Facelets> {
    match modulus {
        Some(modulus) => {
            if let Some(v) = arch.registers()[idx].signature_facelets_mod(modulus) {
                Some(v)
            } else {
                let cycles = arch.registers()[idx]
                    .unshared_cycles()
                    .iter()
                    .map(CycleGeneratorSubcycle::chromatic_order)
                    .sorted()
                    .dedup()
                    .collect_vec();

                r.push(
                    Report::build(ReportKind::Error, register.reg_name.span().clone())
                        .with_message(format!(
                            "Could not find a set of pieces for solved-goto that encode the given modulus. The available moduli are the LCM of any combination of the following piece subcycles: {}",
                            cycles.into_iter().join(", ")
                        ))
                        .finish(),
                );
                None
            }
        }
        None => Some(arch.registers()[idx].signature_facelets()),
    }
}

struct FaceletsInfo;

impl SeparatesByPuzzleType for FaceletsInfo {
    type Theoretical<'s> = TheoreticalIdx;

    type Puzzle<'s> = (PuzzleIdx, Facelets);
}

pub fn strip_expanded(expanded: ExpandedCode, r: &Reporter) -> Option<Program> {
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

                global_regs
                    .puzzles
                    .push(WithSpan::new(architecture.group_arc(), puzzle_span.clone()));
            }
        }
    }

    let global_regs = Arc::new(global_regs);
    let global_regs_for_iter = Arc::clone(&global_regs);

    let before = r.count();

    let instructions_mapped = expanded
        .expanded_code_components
        .into_iter()
        .filter_map(move |v| {
            let span = v.span().clone();
            let instr = match v.into_inner() {
                ExpandedCodeComponent::Instruction(primitive, block_id) => {
                    OptimizingCodeComponent::Instruction(
                        Box::new(match *primitive {
                            Primitive::Add { amt, register } => {
                                match global_regs_for_iter.get_reg(&register) {
                                    ByPuzzleType::Theoretical((theoretical, ())) => {
                                        OptimizingPrimitive::AddTheoretical { theoretical, amt }
                                    }
                                    ByPuzzleType::Puzzle((puzzle, (reg_idx, arch, _))) => {
                                        OptimizingPrimitive::AddPuzzle {
                                            puzzle,
                                            arch,
                                            amts: vec![(reg_idx, amt)],
                                        }
                                    }
                                }
                            }
                            Primitive::Goto { label } => {
                                let span = label.span().clone();
                                let Some(label) = expanded.block_info.label_scope(&label) else {
                                    r.push(
                                        Report::build(ReportKind::Error, label.span().clone())
                                            .with_message("Could not find label in scope")
                                            .finish(),
                                    );
                                    return None;
                                };

                                OptimizingPrimitive::Goto {
                                    label: span.with(label),
                                }
                            }
                            Primitive::SolvedGoto { label, register } => {
                                let span = label.span().clone();
                                let Some(label) = expanded.block_info.label_scope(&label) else {
                                    r.push(
                                        Report::build(ReportKind::Error, label.span().clone())
                                            .with_message("Could not find label in scope")
                                            .finish(),
                                    );
                                    return None;
                                };

                                OptimizingPrimitive::SolvedGoto {
                                    label: span.with(label),
                                    register,
                                }
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
            };

            Some(span.with(instr))
        })
        .collect_vec();

    if r.count() - before != 0 {
        return None;
    }

    let optimized = do_optimization(instructions_mapped.into_iter(), &global_regs);

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
                            branch_key: label.branch_key,
                        },
                        program_counter,
                    );
                    None
                }
            }
            .map(|v| WithSpan::new(v, span))
        })
        .collect_vec();

    let before = r.count();

    let instructions = instructions
        .into_iter()
        .filter_map(|fully_simplified| {
            let span = fully_simplified.span().to_owned();

            let instruction = match *fully_simplified.into_inner() {
                OptimizingPrimitive::AddPuzzle { puzzle, arch, amts } => {
                    Instruction::PerformAlgorithm(ByPuzzleType::Puzzle((
                        puzzle,
                        new_from_effect(
                            &arch,
                            amts.into_iter()
                                .map(|(idx, amt)| (idx, amt.into_inner()))
                                .collect(),
                        ),
                    )))
                }
                OptimizingPrimitive::AddTheoretical { theoretical, amt } => {
                    Instruction::PerformAlgorithm(ByPuzzleType::Theoretical((theoretical, *amt)))
                }
                OptimizingPrimitive::Goto { label } => Instruction::Goto {
                    instruction_idx: *label_locations.get(&label).unwrap(),
                },
                OptimizingPrimitive::SolvedGoto { register, label } => {
                    let facelets = global_regs.facelets(&register, r)?;

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
                    facelets: match global_regs.facelets(&register, r)? {
                        ByPuzzleType::Theoretical(_) => unreachable!(),
                        ByPuzzleType::Puzzle((idx, facelets)) => {
                            assert_eq!(idx, puzzle);
                            facelets
                        }
                    },
                    alg: new_from_effect(
                        &arch,
                        amts.into_iter()
                            .map(|(idx, amt)| (idx, amt.into_inner()))
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

                    Instruction::Input(match global_regs.generator(&register, r)? {
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
                        Some(register) => match global_regs.generator(&register, r)? {
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
                        Some(register) => match global_regs.generator(&register, r)? {
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

            Some(WithSpan::new(instruction, span))
        })
        .collect_vec();

    if r.count() - before != 0 {
        return None;
    }

    let global_regs = Arc::into_inner(global_regs).unwrap();

    Some(Program {
        theoretical: global_regs.theoretical,
        puzzles: global_regs.puzzles,
        instructions,
    })
}

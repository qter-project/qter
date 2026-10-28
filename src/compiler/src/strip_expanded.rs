use std::{collections::HashMap, sync::Arc};

use ariadne::{Label, Report, ReportKind};
use itertools::Itertools;
use puzzle_theory::{
    numbers::{I, Int, U},
    permutations::PermutationGroup,
};
use qter_core::{Decoding, span::WithSpan};
use qter_core::{
    Facelets, Instruction, Program, PuzzleIdx,
    architectures::{Architecture, CycleGeneratorSubcycle, new_from_effect},
};
use sarlacc::Intern;

use crate::{
    ExpandedCode, ExpandedCodeComponent, LabelReference, Primitive, Puzzle, RegisterModifiers,
    RegisterReference, Reporter,
    optimization::{OptimizingCodeComponent, OptimizingPrimitive, do_optimization},
};

#[derive(Clone)]
pub(super) struct RegisterIdx {
    pub puzzle_idx: PuzzleIdx,
    pub reg_idx: usize,
    pub arch: Arc<Architecture>,
    pub mods: RegisterModifiers,
}

pub struct GlobalRegs {
    register_table: HashMap<Intern<str>, RegisterIdx>,
    puzzles: Vec<WithSpan<Arc<PermutationGroup>>>,
}

impl GlobalRegs {
    pub(super) fn get_reg(&self, reference: RegisterReference) -> RegisterIdx {
        let mut reg = self
            .register_table
            .get(&reference.reg_name)
            .unwrap()
            .to_owned();

        reg.mods = reference.mods;

        reg
    }

    fn decoding(&self, register: RegisterReference, r: &Reporter) -> Option<Decoding> {
        let RegisterIdx {
            puzzle_idx,
            reg_idx,
            arch,
            mods,
        } = self.get_reg(register);

        let mut alg = new_from_effect(&arch, vec![(reg_idx, Int::<U>::one())]);

        if mods.negative {
            alg.exponentiate(-Int::<I>::one());
        }

        Some(Decoding {
            puzzle_idx,
            algorithm: alg,
            facelets: get_facelets(reg_idx, &arch, mods.modulus, register, r)?,
        })
    }

    fn facelets(&self, register: RegisterReference, r: &Reporter) -> Option<(PuzzleIdx, Facelets)> {
        let RegisterIdx {
            puzzle_idx,
            reg_idx,
            arch,
            mods,
        } = self.get_reg(register);

        Some((
            puzzle_idx,
            get_facelets(reg_idx, &arch, mods.modulus, register, r)?,
        ))
    }
}

fn get_facelets(
    idx: usize,
    arch: &Architecture,
    modulus: Option<Int<U>>,
    register: RegisterReference,
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

                let span = register.reg_name.span();

                r.push(
                    Report::build(ReportKind::Error, span)
                        .with_label(Label::new(span)
                        .with_message(format!(
                            "Could not find a set of pieces for solved-goto that give divisibility by {modulus}. The available moduli are the LCMs of all combinations of: {}",
                            cycles.into_iter().join(", ")
                        ))
                        )
                        .finish(),
                );
                None
            }
        }
        None => Some(arch.registers()[idx].signature_facelets()),
    }
}

pub fn strip_expanded(expanded: ExpandedCode, r: &Reporter) -> Option<Program> {
    let mut global_regs = GlobalRegs {
        register_table: HashMap::new(),
        puzzles: vec![],
    };

    for Puzzle {
        reg_names,
        arch,
        puzzle_def,
    } in &expanded.registers.puzzles
    {
        for (i, name) in reg_names.iter().enumerate() {
            global_regs.register_table.insert(
                **name,
                RegisterIdx {
                    puzzle_idx: PuzzleIdx(global_regs.puzzles.len()),
                    reg_idx: i,
                    arch: Arc::clone(arch),
                    mods: RegisterModifiers {
                        modulus: None,
                        negative: false,
                    },
                },
            );
        }

        global_regs
            .puzzles
            .push(WithSpan::new(arch.group_arc(), *puzzle_def));
    }

    let global_regs = Arc::new(global_regs);
    let global_regs_for_iter = Arc::clone(&global_regs);

    let before = r.count();

    let instructions_mapped = expanded
        .expanded_code_components
        .into_iter()
        .filter_map(move |v| {
            let span = v.span();
            let instr = match v.into_inner() {
                ExpandedCodeComponent::Instruction(primitive, block_id) => {
                    OptimizingCodeComponent::Instruction(
                        Box::new(match *primitive {
                            Primitive::Add { amt, register } => {
                                let RegisterIdx {
                                    puzzle_idx,
                                    reg_idx,
                                    arch,
                                    mods,
                                } = global_regs_for_iter.get_reg(register);

                                OptimizingPrimitive::AddPuzzle {
                                    puzzle: puzzle_idx,
                                    amts: vec![(
                                        reg_idx,
                                        if mods.negative {
                                            amt.span().with(
                                                arch.registers()[reg_idx].order()
                                                    - amt.into_inner(),
                                            )
                                        } else {
                                            amt
                                        },
                                    )],
                                    arch,
                                }
                            }
                            Primitive::Goto { label } => {
                                let span = label.span();
                                let Some(label) = expanded.block_info.label_scope(&label) else {
                                    r.push(
                                        Report::build(ReportKind::Error, label.span())
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
                                let span = label.span();
                                let Some(label) = expanded.block_info.label_scope(&label) else {
                                    r.push(
                                        Report::build(ReportKind::Error, label.span())
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
            let span = component.span();

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
            let span = fully_simplified.span();

            let instruction = match *fully_simplified.into_inner() {
                OptimizingPrimitive::AddPuzzle { puzzle, arch, amts } => {
                    Instruction::PerformAlgorithm {
                        puzzle,
                        alg: new_from_effect(
                            &arch,
                            amts.into_iter()
                                .map(|(idx, amt)| (idx, amt.into_inner()))
                                .collect(),
                        ),
                    }
                }
                OptimizingPrimitive::Goto { label } => Instruction::Goto {
                    instruction_idx: *label_locations.get(&label).unwrap(),
                },
                OptimizingPrimitive::SolvedGoto { register, label } => {
                    let (puzzle_idx, facelets) = global_regs.facelets(register, r)?;

                    Instruction::SolvedGoto {
                        instruction_idx: *label_locations.get(&label).unwrap(),
                        puzzle_idx,
                        facelets,
                    }
                }
                OptimizingPrimitive::RepeatUntil {
                    puzzle,
                    arch,
                    amts,
                    register,
                } => Instruction::RepeatUntil {
                    puzzle_idx: puzzle,
                    facelets: {
                        let (idx, facelets) = global_regs.facelets(register, r)?;
                        assert_eq!(idx, puzzle);
                        facelets
                    },
                    alg: new_from_effect(
                        &arch,
                        amts.into_iter()
                            .map(|(idx, amt)| (idx, amt.into_inner()))
                            .collect(),
                    ),
                },
                OptimizingPrimitive::Solve { puzzle } => Instruction::Solve(puzzle),
                OptimizingPrimitive::Input { message, register } => {
                    let decoding = global_regs.decoding(register, r)?;

                    Instruction::Input {
                        message: message.into_inner(),
                        decoding,
                    }
                }
                OptimizingPrimitive::Halt { message, register } => {
                    let decoding = match register {
                        Some(v) => Some(global_regs.decoding(v, r)?),
                        None => None,
                    };

                    Instruction::Halt {
                        message: message.into_inner(),
                        maybe_reg: decoding,
                    }
                },
                OptimizingPrimitive::Print { message, register } => {
                    let decoding = match register {
                        Some(v) => Some(global_regs.decoding(v, r)?),
                        None => None,
                    };

                    Instruction::Print {
                        message: message.into_inner(),
                        maybe_reg: decoding,
                    }
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
        puzzles: global_regs.puzzles,
        instructions,
    })
}

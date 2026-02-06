use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};

use itertools::Itertools;
use puzzle_theory::{
    numbers::{Int, U, lcm, lcm_iter},
    span::WithSpan,
};
use qter_core::{
    ByPuzzleType, PuzzleIdx, TheoreticalIdx,
    architectures::{Architecture, CycleGeneratorSubcycle},
};

use crate::{
    BlockID,
    optimization::{
        OptimizingPrimitive,
        combinators::{PeepholeRewriter, Rewriter},
        extend_from_start,
    },
    primitive_match,
    strip_expanded::GlobalRegs,
};

use super::OptimizingCodeComponent;

/// Any non-label instructions that come immedately after an unconditional goto or halt are unreachable and can be removed
#[derive(Default)]
pub struct RemoveUnreachableCode {
    diverging: Option<WithSpan<OptimizingCodeComponent>>,
}

impl Rewriter for RemoveUnreachableCode {
    type Component = WithSpan<OptimizingCodeComponent>;
    type GlobalData = GlobalRegs;

    fn rewrite(
        &mut self,
        component: WithSpan<OptimizingCodeComponent>,
        _: &GlobalRegs,
    ) -> Vec<WithSpan<OptimizingCodeComponent>> {
        match self.diverging.take() {
            Some(goto) => {
                if matches!(&*component, OptimizingCodeComponent::Label(_)) {
                    return vec![goto, component];
                }

                // Otherwise throw out the instruction
                self.diverging = Some(goto);

                Vec::new()
            }
            None => {
                primitive_match!((OptimizingPrimitive::Goto { .. } | OptimizingPrimitive::Halt { .. }) = Some(&component); else { return vec![component]; });

                self.diverging = Some(component);

                Vec::new()
            }
        }
    }

    fn eof(self, _: &GlobalRegs) -> Vec<WithSpan<OptimizingCodeComponent>> {
        match self.diverging {
            Some(goto) => vec![goto],
            None => Vec::new(),
        }
    }
}

#[derive(Default)]
pub struct RemoveUselessJumps;

impl PeepholeRewriter for RemoveUselessJumps {
    type Component = WithSpan<OptimizingCodeComponent>;
    type GlobalData = GlobalRegs;

    const MAX_WINDOW_SIZE: usize = 2;

    fn try_match(window: &mut VecDeque<WithSpan<OptimizingCodeComponent>>, _: &GlobalRegs) {
        let Some(OptimizingCodeComponent::Label(label)) = window.get(1).map(|v| &**v) else {
            return;
        };

        primitive_match!(
            (OptimizingPrimitive::SolvedGoto {
                label: jumps_to,
                ..
            } | OptimizingPrimitive::Goto { label: jumps_to }) = window.front()
        );

        if jumps_to.name == label.name && jumps_to.block_id == label.maybe_block_id.unwrap() {
            window.pop_front().unwrap();
        }
    }
}

#[derive(Default)]
pub struct CoalesceAdds {
    block_id: Option<BlockID>,
    theoreticals: Vec<WithSpan<(TheoreticalIdx, WithSpan<Int<U>>)>>,
    puzzles: Vec<WithSpan<(PuzzleIdx, Arc<Architecture>, Vec<(usize, WithSpan<Int<U>>)>)>>,
}

impl CoalesceAdds {
    fn dump_state(&mut self) -> Vec<WithSpan<OptimizingCodeComponent>> {
        self.theoreticals
            .drain(..)
            .map(|v| {
                v.map(|(theoretical, amt)| {
                    OptimizingCodeComponent::Instruction(
                        Box::new(OptimizingPrimitive::AddTheoretical { theoretical, amt }),
                        self.block_id.unwrap(),
                    )
                })
            })
            .chain(self.puzzles.drain(..).map(|v| {
                v.map(|(puzzle, arch, amts)| {
                    OptimizingCodeComponent::Instruction(
                        Box::new(OptimizingPrimitive::AddPuzzle { puzzle, arch, amts }),
                        self.block_id.unwrap(),
                    )
                })
            }))
            .collect()
    }

    fn merge_effects(
        effect1: &mut Vec<(usize, WithSpan<Int<U>>)>,
        effect2: &[(usize, WithSpan<Int<U>>)],
    ) {
        'next_effect: for new_effect in effect2 {
            for effect in &mut *effect1 {
                if effect.0 == new_effect.0 {
                    *effect.1 += *new_effect.1;
                    continue 'next_effect;
                }
            }

            effect1.push(new_effect.to_owned());
        }
    }
}

impl Rewriter for CoalesceAdds {
    type Component = WithSpan<OptimizingCodeComponent>;
    type GlobalData = GlobalRegs;

    fn rewrite(
        &mut self,
        component: WithSpan<OptimizingCodeComponent>,
        _: &GlobalRegs,
    ) -> Vec<WithSpan<OptimizingCodeComponent>> {
        let span = component.span().clone();

        match component.into_inner() {
            OptimizingCodeComponent::Instruction(instr, block_id) => match *instr {
                OptimizingPrimitive::AddTheoretical {
                    theoretical: theoretical_idx,
                    amt,
                } => {
                    self.block_id = Some(block_id);

                    for theoretical in &mut self.theoreticals {
                        if theoretical.0 == theoretical_idx {
                            *theoretical.1 += *amt;
                            return Vec::new();
                        }
                    }

                    self.theoreticals.push(span.with((theoretical_idx, amt)));

                    Vec::new()
                }
                OptimizingPrimitive::AddPuzzle {
                    puzzle: puzzle_idx,
                    arch,
                    amts,
                } => {
                    self.block_id = Some(block_id);

                    for puzzle in &mut self.puzzles {
                        if puzzle.0 == puzzle_idx {
                            CoalesceAdds::merge_effects(&mut puzzle.2, &amts);

                            return Vec::new();
                        }
                    }

                    self.puzzles.push(span.with((puzzle_idx, arch, amts)));

                    Vec::new()
                }
                primitive => {
                    let mut instrs = self.dump_state();
                    instrs.push(span.with(OptimizingCodeComponent::Instruction(
                        Box::new(primitive),
                        block_id,
                    )));
                    instrs
                }
            },
            OptimizingCodeComponent::Label(label) => {
                let mut instrs = self.dump_state();
                instrs.push(span.with(OptimizingCodeComponent::Label(label)));
                instrs
            }
        }
    }

    fn eof(mut self, _: &GlobalRegs) -> Vec<WithSpan<OptimizingCodeComponent>> {
        self.dump_state()
    }
}

/*
Transforms
```
spot1:
    solved-goto <positions> wherever
    <algorithm>
    goto spot1
```
into
```
spot1:
    repeat until <positions> solved <algorithm>
    goto wherever
```
*/
pub struct RepeatUntil1;

impl PeepholeRewriter for RepeatUntil1 {
    type Component = WithSpan<OptimizingCodeComponent>;
    type GlobalData = GlobalRegs;

    const MAX_WINDOW_SIZE: usize = 5;

    fn try_match(
        window: &mut VecDeque<WithSpan<OptimizingCodeComponent>>,
        global_regs: &GlobalRegs,
    ) {
        let Some(OptimizingCodeComponent::Label(spot1)) = window.front().map(|v| &**v) else {
            return;
        };

        primitive_match!(
            OptimizingPrimitive::SolvedGoto {
                label: spot2,
                register,
            } = window.get(1)
        );

        primitive_match!(OptimizingPrimitive::AddPuzzle { puzzle, arch, amts } = window.get(2));

        if match global_regs.get_reg(register) {
            qter_core::ByPuzzleType::Theoretical(_) => true,
            qter_core::ByPuzzleType::Puzzle((idx, _)) => idx != *puzzle,
        } {
            return;
        }

        primitive_match!(OptimizingPrimitive::Goto { label } = window.get(3));

        if label.name != spot1.name || label.block_id != spot1.maybe_block_id.unwrap() {
            return;
        }

        let repeat_until = OptimizingCodeComponent::Instruction(
            Box::new(OptimizingPrimitive::RepeatUntil {
                puzzle: *puzzle,
                arch: Arc::clone(arch),
                amts: amts.to_owned(),
                register: register.to_owned(),
            }),
            spot2.block_id,
        );

        let goto = OptimizingCodeComponent::Instruction(
            Box::new(OptimizingPrimitive::Goto {
                label: spot2.to_owned(),
            }),
            spot2.block_id,
        );

        let mut values = Vec::new();
        values.push(window.pop_front().unwrap());

        let span = window
            .drain(0..3)
            .map(|v| v.span().clone())
            .reduce(|a, v| a.merge(&v))
            .unwrap();

        values.push(span.clone().with(repeat_until));
        values.push(span.with(goto));

        extend_from_start(window, values);
    }
}

/*
Transforms
```
spot1:
    <algorithm>
<optional label>:
    solved-goto <positions> wherever
    goto spot1
```
into
```
spot1:
    <algorithm>
<optional label>:
    repeat until <positions> solved <algorithm>
    goto wherever
```
*/
pub struct RepeatUntil2;

impl PeepholeRewriter for RepeatUntil2 {
    type Component = WithSpan<OptimizingCodeComponent>;
    type GlobalData = GlobalRegs;

    const MAX_WINDOW_SIZE: usize = 6;

    fn try_match(
        window: &mut VecDeque<WithSpan<OptimizingCodeComponent>>,
        global_regs: &GlobalRegs,
    ) {
        let Some(OptimizingCodeComponent::Label(spot1)) = window.front().map(|v| &**v) else {
            return;
        };

        primitive_match!(OptimizingPrimitive::AddPuzzle { puzzle, arch, amts } = window.get(1));

        let optional_label = usize::from(matches!(
            window.get(2).map(|v| &**v),
            Some(OptimizingCodeComponent::Label(_))
        ));

        primitive_match!(
            OptimizingPrimitive::SolvedGoto {
                label: spot3,
                register,
            } = window.get(2 + optional_label)
        );

        if match global_regs.get_reg(register) {
            qter_core::ByPuzzleType::Theoretical(_) => true,
            qter_core::ByPuzzleType::Puzzle((idx, _)) => idx != *puzzle,
        } {
            return;
        }

        primitive_match!(
            OptimizingPrimitive::Goto { label: maybe_spot1 } = window.get(3 + optional_label)
        );

        if spot1.name != maybe_spot1.name || spot1.maybe_block_id.unwrap() != maybe_spot1.block_id {
            return;
        }

        let repeat_until = OptimizingCodeComponent::Instruction(
            Box::new(OptimizingPrimitive::RepeatUntil {
                puzzle: *puzzle,
                arch: Arc::clone(arch),
                amts: amts.to_owned(),
                register: register.to_owned(),
            }),
            spot3.block_id,
        );

        let goto = OptimizingCodeComponent::Instruction(
            Box::new(OptimizingPrimitive::Goto {
                label: spot3.clone(),
            }),
            spot3.block_id,
        );

        let mut out = Vec::new();

        out.extend(window.drain(0..2 + optional_label));

        let span = window
            .drain(0..2)
            .map(|v| v.span().clone())
            .reduce(|a, v| a.merge(&v))
            .unwrap();

        out.push(span.clone().with(repeat_until));
        out.push(span.with(goto));

        extend_from_start(window, out);
    }
}

/*
Transforms
```
spot1:
    <algorithm>
<optional label>:
    solved-goto <positions> wherever
    <optional algorithm>
    goto spot1
```
into
```
spot1:
    <algorithm>
<optional label>:
    repeat until <positions> solved <optional algorithm> <algorithm>
    goto wherever
```
*/
pub struct RepeatUntil3;

impl PeepholeRewriter for RepeatUntil3 {
    type Component = WithSpan<OptimizingCodeComponent>;
    type GlobalData = GlobalRegs;

    const MAX_WINDOW_SIZE: usize = 7;

    fn try_match(
        window: &mut VecDeque<WithSpan<OptimizingCodeComponent>>,
        global_regs: &GlobalRegs,
    ) {
        let Some(OptimizingCodeComponent::Label(spot1)) = window.front().map(|v| &**v) else {
            return;
        };

        primitive_match!(OptimizingPrimitive::AddPuzzle { puzzle, arch, amts } = window.get(1));

        let optional_label = usize::from(matches!(
            window.get(2).map(|v| &**v),
            Some(OptimizingCodeComponent::Label(_))
        ));

        primitive_match!(
            OptimizingPrimitive::SolvedGoto {
                label: spot2,
                register,
            } = window.get(2 + optional_label)
        );

        if match global_regs.get_reg(register) {
            qter_core::ByPuzzleType::Theoretical(_) => true,
            qter_core::ByPuzzleType::Puzzle((idx, _)) => idx != *puzzle,
        } {
            return;
        }

        let maybe_algorithm = match window.get(3 + optional_label).map(|v| &**v) {
            Some(OptimizingCodeComponent::Instruction(optimizing_primitive, _)) => {
                match &**optimizing_primitive {
                    OptimizingPrimitive::AddPuzzle {
                        puzzle: new_puzzle,
                        arch,
                        amts,
                    } => {
                        if puzzle != new_puzzle {
                            return;
                        }

                        Some((new_puzzle, arch, amts))
                    }
                    _ => None,
                }
            }
            Some(OptimizingCodeComponent::Label(_)) => None,
            None => return,
        };

        let is_alg = usize::from(maybe_algorithm.is_some());

        primitive_match!(
            OptimizingPrimitive::Goto { label: maybe_spot1 } =
                window.get(3 + optional_label + is_alg)
        );

        if maybe_spot1.name != spot1.name || maybe_spot1.block_id != spot1.maybe_block_id.unwrap() {
            return;
        }

        let mut amts = amts.to_owned();

        if let Some((_, _, effect)) = maybe_algorithm {
            CoalesceAdds::merge_effects(&mut amts, effect);
        }

        let repeat_until = OptimizingCodeComponent::Instruction(
            Box::new(OptimizingPrimitive::RepeatUntil {
                puzzle: *puzzle,
                arch: Arc::clone(arch),
                amts,
                register: register.to_owned(),
            }),
            maybe_spot1.block_id,
        );

        let goto = OptimizingCodeComponent::Instruction(
            Box::new(OptimizingPrimitive::Goto {
                label: spot2.clone(),
            }),
            maybe_spot1.block_id,
        );

        let mut out = Vec::new();

        out.extend(window.drain(0..2 + optional_label));

        let span = window
            .drain(0..2 + is_alg)
            .map(|v| v.span().clone())
            .reduce(|a, v| a.merge(&v))
            .unwrap();

        out.push(span.clone().with(repeat_until));
        out.push(span.with(goto));

        extend_from_start(window, out);
    }
}

/// Splits up a repeat until into one for each piece being checked. Effectively transforms
///
/// ```
/// .registers {
///     // Note that `B` is a composition of an 8 cycle and a 6 cycle
///     A, B ← 3x3 builtin (210, 24)
/// }
///
/// while not-solved B {
///     dec A
///     dec B
/// }
/// ```
///
/// into
///
/// ```
/// while not-solved B%6 {
///     dec A
///     dec B
/// }
///
/// while not-solved B {
///     sub A 6
///     sub B 6
/// }
/// ```
pub struct VectorizeRepeatUntil;

impl PeepholeRewriter for VectorizeRepeatUntil {
    type Component = WithSpan<OptimizingCodeComponent>;
    type GlobalData = GlobalRegs;

    const MAX_WINDOW_SIZE: usize = 1;

    fn try_match(window: &mut VecDeque<Self::Component>, global_regs: &Self::GlobalData) {
        let Some(OptimizingCodeComponent::Instruction(instr, _)) =
            window.front_mut().map(|v| &mut **v)
        else {
            return;
        };

        let OptimizingPrimitive::RepeatUntil {
            puzzle: _,
            arch: _,
            amts,
            register,
        } = &mut **instr
        else {
            return;
        };

        let ByPuzzleType::Puzzle((_, (reg_idx, arch, modulus))) = global_regs.get_reg(register)
        else {
            return;
        };

        let Some(amt) = amts
            .iter()
            .find_map(|(idx, amt)| (*idx == reg_idx).then_some(**amt))
        else {
            return;
        };

        let reg_order = arch.registers()[reg_idx].order();
        let modulus = modulus.unwrap_or(reg_order);

        let cycles = arch.registers()[reg_idx].unshared_cycles();

        if let Some((cycle_order, new_amt)) = cycles
            .iter()
            .map(CycleGeneratorSubcycle::chromatic_order)
            .filter(|v| modulus != *v && (modulus % *v).is_zero() && !(amt % *v).is_zero())
            .map(|v| (v, lcm(v, amt)))
            .min_by_key(|v| v.1)
        {
            register.modulus = Some(cycle_order);

            let mut next = window.front().unwrap().clone();

            let OptimizingCodeComponent::Instruction(instr, _) = &mut *next else {
                unreachable!();
            };

            let OptimizingPrimitive::RepeatUntil {
                puzzle: _,
                arch: _,
                amts,
                register,
            } = &mut **instr
            else {
                unreachable!();
            };

            let scale_amt = new_amt / amt;

            register.modulus = Some(lcm_iter(
                cycles
                    .iter()
                    .map(CycleGeneratorSubcycle::chromatic_order)
                    .filter(|v| (modulus % *v).is_zero() && !(cycle_order % *v).is_zero()),
            ));

            for amt in amts {
                *amt.1 *= scale_amt;
                *amt.1 %= arch.registers()[amt.0].order();
            }

            window.push_back(next);
        }
    }
}

#[derive(Default)]
pub struct TransformSolve {
    instrs: VecDeque<(WithSpan<OptimizingCodeComponent>, usize, Int<U>)>,
    puzzle_idx: Option<PuzzleIdx>,
    guaranteed_zeroed: HashMap<usize, Int<U>>,
}

impl TransformSolve {
    fn dump(&mut self) -> Vec<WithSpan<OptimizingCodeComponent>> {
        self.guaranteed_zeroed = HashMap::new();
        self.instrs
            .drain(..)
            .map(|(instr, _, _)| instr)
            .collect_vec()
    }

    fn dump_with(
        &mut self,
        instr: WithSpan<OptimizingCodeComponent>,
    ) -> Vec<WithSpan<OptimizingCodeComponent>> {
        let mut instrs = self.dump();
        instrs.push(instr);
        instrs
    }
}

impl Rewriter for TransformSolve {
    type Component = WithSpan<OptimizingCodeComponent>;
    type GlobalData = GlobalRegs;

    fn rewrite(
        &mut self,
        component: WithSpan<OptimizingCodeComponent>,
        global_regs: &GlobalRegs,
    ) -> Vec<WithSpan<OptimizingCodeComponent>> {
        let OptimizingCodeComponent::Instruction(instr, block_id) = &*component else {
            return self.dump_with(component);
        };

        let OptimizingPrimitive::RepeatUntil {
            puzzle,
            arch: _,
            amts,
            register,
        } = &**instr
        else {
            return self.dump_with(component);
        };

        let mut dumped = Vec::new();

        if self.puzzle_idx.is_some() && self.puzzle_idx != Some(*puzzle) {
            dumped.extend(self.dump());
        }

        self.puzzle_idx = Some(*puzzle);

        let ByPuzzleType::Puzzle((puzzle_idx, (reg_idx, arch, modulus))) =
            global_regs.get_reg(register)
        else {
            dumped.extend(self.dump_with(component));
            return dumped;
        };

        assert_eq!(*puzzle, puzzle_idx);

        let mut broken = HashSet::new();

        for amt in amts {
            if amt.0 != reg_idx {
                broken.insert(amt.0);
            }
        }

        if let Some((i, _)) = self
            .instrs
            .iter()
            .enumerate()
            .rev()
            .find(|v| broken.contains(&v.1.1))
        {
            dumped.extend(self.instrs.drain(0..i).map(|v| v.0));
        }

        for thingy in broken {
            self.guaranteed_zeroed.remove(&thingy);
        }

        // If we have a modulus, then it is possible for the whole register not to be zeroed in the end
        let modulus = modulus.unwrap_or_else(|| arch.registers()[reg_idx].order());

        let zeroed_mod = self.guaranteed_zeroed.entry(reg_idx).or_insert(modulus);
        *zeroed_mod = lcm(*zeroed_mod, modulus);

        if self.guaranteed_zeroed.len() == arch.registers().len()
            && self
                .guaranteed_zeroed
                .iter()
                .all(|(idx, modulus)| arch.registers()[*idx].order() == *modulus)
        {
            let span = self
                .instrs
                .drain(..)
                .map(|v| v.0.span().clone())
                .reduce(|a, v| a.merge(&v))
                .unwrap();

            self.guaranteed_zeroed = HashMap::new();
            dumped.push(span.with(OptimizingCodeComponent::Instruction(
                Box::new(OptimizingPrimitive::Solve {
                    puzzle: ByPuzzleType::Puzzle(self.puzzle_idx.unwrap()),
                }),
                *block_id,
            )));
        } else {
            self.instrs.push_back((component, reg_idx, modulus));
        }

        dumped
    }

    fn eof(mut self, _: &GlobalRegs) -> Vec<WithSpan<OptimizingCodeComponent>> {
        self.dump()
    }
}

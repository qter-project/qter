use puzzle_theory::numbers::{Int, U, lcm};
use qter_core::{
    ByPuzzleType, Halt, Input, PerformAlgorithm, Print, RepeatUntil, SeparatesByPuzzleType, Solve,
    SolvedGoto, architectures::chromatic_orders_by_facelets,
};

use crate::{
    ActionPerformed, ExecutionState, InterpreterState, PausedState, PuzzleAndRegister, PuzzleState,
    SucceededSolvedGoto,
};

pub async fn do_instr<'a, Instr: PuzzleInstructionImpl, P: PuzzleState>(
    instr: &'a ByPuzzleType<'static, Instr>,
    state: &mut InterpreterState<P>,
) -> Result<ActionPerformed<'a>, P::Error> {
    match instr {
        ByPuzzleType::Theoretical(instr) => Ok(Instr::perform_theoretical(instr, state)),
        ByPuzzleType::Puzzle(instr) => Instr::perform_puzzle(instr, state).await,
    }
}

pub trait PuzzleInstructionImpl: SeparatesByPuzzleType {
    fn perform_theoretical<'a, P: PuzzleState>(
        instr: &'a Self::Theoretical<'static>,
        state: &mut InterpreterState<P>,
    ) -> ActionPerformed<'a>;

    async fn perform_puzzle<'a, P: PuzzleState>(
        instr: &'a Self::Puzzle<'static>,
        state: &mut InterpreterState<P>,
    ) -> Result<ActionPerformed<'a>, P::Error>;
}

impl PuzzleInstructionImpl for SolvedGoto {
    fn perform_theoretical<'a, P: PuzzleState>(
        instr: &'a Self::Theoretical<'static>,
        state: &mut InterpreterState<P>,
    ) -> ActionPerformed<'a> {
        if Int::is_zero(&state.puzzle_states.theoretical_state(instr.1).value()) {
            state.program_counter = instr.0.instruction_idx;

            ActionPerformed::SucceededSolvedGoto(ByPuzzleType::Theoretical((
                SucceededSolvedGoto {
                    jumped_to: instr.0.instruction_idx,
                },
                instr.1,
            )))
        } else {
            state.program_counter += 1;

            ActionPerformed::FailedSolvedGoto(ByPuzzleType::Theoretical(instr.1))
        }
    }

    async fn perform_puzzle<'a, P: PuzzleState>(
        instr: &'a Self::Puzzle<'static>,
        state: &mut InterpreterState<P>,
    ) -> Result<ActionPerformed<'a>, P::Error> {
        let puzzle = state.puzzle_states.puzzle_state_mut(instr.1);

        if puzzle.facelets_solved(instr.2.facelets()).await? {
            state.program_counter = instr.0.instruction_idx;

            Ok(ActionPerformed::SucceededSolvedGoto(ByPuzzleType::Puzzle(
                (
                    SucceededSolvedGoto {
                        jumped_to: instr.0.instruction_idx,
                    },
                    instr.1,
                    &instr.2,
                ),
            )))
        } else {
            state.program_counter += 1;

            Ok(ActionPerformed::FailedSolvedGoto(ByPuzzleType::Puzzle((
                instr.1, &instr.2,
            ))))
        }
    }
}

fn input_impl<'a, P: PuzzleState>(
    order: Int<U>,
    message: &'a str,
    data: ByPuzzleType<'static, PuzzleAndRegister>,
    state: &mut InterpreterState<P>,
) -> ActionPerformed<'a> {
    let max_input = order - Int::<U>::one();
    state.execution_state = ExecutionState::Paused(PausedState::Input { max_input, data });
    state
        .messages
        .push_back(format!("{message} (max input {max_input})"));

    ActionPerformed::Paused
}

impl PuzzleInstructionImpl for Input {
    fn perform_theoretical<'a, P: PuzzleState>(
        instr: &'a Self::Theoretical<'static>,
        state: &mut InterpreterState<P>,
    ) -> ActionPerformed<'a> {
        let order = state.puzzle_states.theoretical_state(instr.1).order();
        input_impl(
            order,
            &instr.0.message,
            ByPuzzleType::Theoretical(instr.1),
            state,
        )
    }

    async fn perform_puzzle<'a, P: PuzzleState>(
        instr: &'a Self::Puzzle<'static>,
        state: &mut InterpreterState<P>,
    ) -> Result<ActionPerformed<'a>, P::Error> {
        let order = instr
            .3
            .facelets()
            .iter()
            .map(|facelet| chromatic_orders_by_facelets(&instr.2)[*facelet])
            .fold(Int::<U>::one(), lcm);

        Ok(input_impl(
            order,
            &instr.0.message,
            // TODO: we should avoid the clone
            ByPuzzleType::Puzzle((instr.1, instr.2.clone(), instr.3.clone())),
            state,
        ))
    }
}

fn perform_halt<'a, P: PuzzleState>(
    maybe_decoded: Option<(Int<U>, ByPuzzleType<'static, PuzzleAndRegister>)>,
    instr: &'a Halt,
    state: &mut InterpreterState<P>,
) -> ActionPerformed<'a> {
    let full_message = if let Some((decoded, puzzle_idx_and_register)) = maybe_decoded {
        state.execution_state = ExecutionState::Paused(PausedState::Halt {
            maybe_puzzle_idx_and_register: Some(puzzle_idx_and_register),
        });

        format!("{} {decoded}", instr.message)
    } else {
        state.execution_state = ExecutionState::Paused(PausedState::Halt {
            maybe_puzzle_idx_and_register: None,
        });

        instr.message.clone()
    };
    state.messages.push_back(full_message);

    ActionPerformed::Paused
}

impl PuzzleInstructionImpl for Halt {
    fn perform_theoretical<'a, P: PuzzleState>(
        instr: &'a Self::Theoretical<'static>,
        state: &mut InterpreterState<P>,
    ) -> ActionPerformed<'a> {
        perform_halt(
            match instr.1 {
                Some(idx) => Some((
                    state.puzzle_states.theoretical_state(idx).value(),
                    ByPuzzleType::Theoretical(idx),
                )),
                None => None,
            },
            &instr.0,
            state,
        )
    }

    async fn perform_puzzle<'a, P: PuzzleState>(
        instr: &'a Self::Puzzle<'static>,
        state: &mut InterpreterState<P>,
    ) -> Result<ActionPerformed<'a>, P::Error> {
        Ok(perform_halt(
            match &instr.1 {
                Some((idx, algorithm, facelets)) => {
                    let puzzle = state.puzzle_states.puzzle_state_mut(*idx);
                    match puzzle.halt(facelets.facelets(), algorithm).await? {
                        Some(v) => Some((
                            v,
                            ByPuzzleType::Puzzle((*idx, algorithm.to_owned(), facelets.to_owned())),
                        )),
                        None => {
                            return Ok(state.panic("The register specified is not decodable!"));
                        }
                    }
                }
                None => None,
            },
            &instr.0,
            state,
        ))
    }
}

fn perform_print<'a, P: PuzzleState>(
    maybe_decoded: Option<Int<U>>,
    instr: &'a Print,
    state: &mut InterpreterState<P>,
) -> ActionPerformed<'a> {
    state.execution_state = ExecutionState::Running;

    let full_message = match maybe_decoded {
        Some(decoded) => {
            format!("{} {decoded}", instr.message)
        }
        None => instr.message.clone(),
    };
    state.messages.push_back(full_message);
    state.program_counter += 1;

    ActionPerformed::None
}

impl PuzzleInstructionImpl for Print {
    fn perform_theoretical<'a, P: PuzzleState>(
        instr: &'a Self::Theoretical<'static>,
        state: &mut InterpreterState<P>,
    ) -> ActionPerformed<'a> {
        perform_print(
            match instr.1 {
                Some(idx) => Some(state.puzzle_states.theoretical_state(idx).value()),
                None => None,
            },
            &instr.0,
            state,
        )
    }

    async fn perform_puzzle<'a, P: PuzzleState>(
        instr: &'a Self::Puzzle<'static>,
        state: &mut InterpreterState<P>,
    ) -> Result<ActionPerformed<'a>, P::Error> {
        Ok(perform_print(
            match &instr.1 {
                Some((idx, algorithm, facelets)) => {
                    let puzzle = state.puzzle_states.puzzle_state_mut(*idx);
                    match puzzle.print(facelets.facelets(), algorithm).await? {
                        Some(v) => Some(v),
                        None => {
                            return Ok(state.panic("The register specified is not decodable!"));
                        }
                    }
                }
                None => None,
            },
            &instr.0,
            state,
        ))
    }
}

impl PuzzleInstructionImpl for PerformAlgorithm {
    fn perform_theoretical<'a, P: PuzzleState>(
        instr: &'a Self::Theoretical<'static>,
        state: &mut InterpreterState<P>,
    ) -> ActionPerformed<'a> {
        state.execution_state = ExecutionState::Running;

        state
            .puzzle_states
            .theoretical_state_mut(instr.0)
            .add_to(instr.1);

        state.program_counter += 1;

        ActionPerformed::Added(ByPuzzleType::Theoretical((instr.0, instr.1)))
    }

    async fn perform_puzzle<'a, P: PuzzleState>(
        instr: &'a Self::Puzzle<'static>,
        state: &mut InterpreterState<P>,
    ) -> Result<ActionPerformed<'a>, P::Error> {
        state.execution_state = ExecutionState::Running;
        state
            .puzzle_states
            .puzzle_state_mut(instr.0)
            .compose_into(&instr.1)
            .await?;

        state.program_counter += 1;

        Ok(ActionPerformed::Added(ByPuzzleType::Puzzle((
            instr.0, &instr.1,
        ))))
    }
}

impl PuzzleInstructionImpl for Solve {
    fn perform_theoretical<'a, P: PuzzleState>(
        instr: &'a Self::Theoretical<'static>,
        state: &mut InterpreterState<P>,
    ) -> ActionPerformed<'a> {
        state.puzzle_states.theoretical_state_mut(*instr).zero_out();

        state.program_counter += 1;

        ActionPerformed::Solved(ByPuzzleType::Theoretical(*instr))
    }

    async fn perform_puzzle<'a, P: PuzzleState>(
        instr: &'a Self::Puzzle<'static>,
        state: &mut InterpreterState<P>,
    ) -> Result<ActionPerformed<'a>, P::Error> {
        state.puzzle_states.puzzle_state_mut(*instr).solve().await?;

        state.program_counter += 1;

        Ok(ActionPerformed::Solved(ByPuzzleType::Puzzle(*instr)))
    }
}

impl PuzzleInstructionImpl for RepeatUntil {
    fn perform_theoretical<'a, P: PuzzleState>(
        _: &'a Self::Theoretical<'static>,
        _: &mut InterpreterState<P>,
    ) -> ActionPerformed<'a> {
        unreachable!()
    }

    async fn perform_puzzle<'a, P: PuzzleState>(
        instr: &'a Self::Puzzle<'static>,
        state: &mut InterpreterState<P>,
    ) -> Result<ActionPerformed<'a>, P::Error> {
        state
            .puzzle_states
            .puzzle_state_mut(instr.puzzle_idx)
            .repeat_until(instr.facelets.facelets(), &instr.alg)
            .await?;

        state.program_counter += 1;

        Ok(ActionPerformed::RepeatedUntil {
            puzzle_idx: instr.puzzle_idx,
            facelets: &instr.facelets,
            alg: &instr.alg,
        })
    }
}

use puzzle_theory::{
    numbers::{Int, U, lcm},
    permutations::Algorithm,
};
use qter_core::{Decoding, Facelets, PuzzleIdx, architectures::chromatic_orders_by_facelets};

use crate::{ActionPerformed, ExecutionState, InterpreterState, PausedState, PuzzleState};

pub async fn perform_solved_goto<'a, P: PuzzleState>(
    instruction_idx: usize,
    puzzle_idx: PuzzleIdx,
    facelets: &'a Facelets,
    state: &mut InterpreterState<P>,
) -> Result<ActionPerformed<'a>, P::Error> {
    let puzzle = state.puzzle_states.puzzle_state_mut(puzzle_idx);

    if puzzle.facelets_solved(facelets.facelets()).await? {
        state.program_counter = instruction_idx;

        Ok(ActionPerformed::SucceededSolvedGoto {
            jumped_to: instruction_idx,
            puzzle: puzzle_idx,
            facelets,
        })
    } else {
        state.program_counter += 1;

        Ok(ActionPerformed::FailedSolvedGoto {
            puzzle: puzzle_idx,
            facelets,
        })
    }
}

pub async fn perform_input<'a, P: PuzzleState>(
    message: &'a str,
    decoding: &'a Decoding,
    state: &mut InterpreterState<P>,
) -> Result<ActionPerformed<'a>, P::Error> {
    let order = decoding
        .facelets
        .facelets()
        .iter()
        .map(|facelet| chromatic_orders_by_facelets(&decoding.algorithm)[*facelet])
        .fold(Int::<U>::one(), lcm);

    let max_input = order - Int::<U>::one();
    state.execution_state = ExecutionState::Paused(PausedState::Input {
        max_input,
        data: decoding.clone(),
    });
    state
        .messages
        .push_back(format!("{message} (max input {max_input})"));
    Ok(ActionPerformed::Paused)
}

pub async fn perform_halt<'a, P: PuzzleState>(
    message: &'a str,
    maybe_reg: Option<&'a Decoding>,
    state: &mut InterpreterState<P>,
) -> Result<ActionPerformed<'a>, P::Error> {
    let full_message = if let Some(decoding) = maybe_reg {
        let puzzle = state.puzzle_states.puzzle_state_mut(decoding.puzzle_idx);
        match puzzle
            .halt(decoding.facelets.facelets(), &decoding.algorithm)
            .await?
        {
            Some(decoded) => {
                state.execution_state = ExecutionState::Paused(PausedState::Halt {
                    maybe_puzzle_idx_and_register: Some(decoding.clone()),
                });

                format!("{message} {decoded}")
            }
            None => {
                return Ok(state.panic("The register specified is not decodable!"));
            }
        }
    } else {
        state.execution_state = ExecutionState::Paused(PausedState::Halt {
            maybe_puzzle_idx_and_register: None,
        });

        message.to_string()
    };

    state.messages.push_back(full_message);

    Ok(ActionPerformed::Paused)
}

pub async fn perform_print<'a, P: PuzzleState>(
    message: &'a str,
    maybe_reg: Option<&'a Decoding>,
    state: &mut InterpreterState<P>,
) -> Result<ActionPerformed<'a>, P::Error> {
    let full_message = match &maybe_reg {
        Some(Decoding {
            puzzle_idx,
            facelets,
            algorithm,
        }) => {
            let puzzle = state.puzzle_states.puzzle_state_mut(*puzzle_idx);
            match puzzle.print(facelets.facelets(), algorithm).await? {
                Some(decoded) => format!("{message} {decoded}"),
                None => {
                    return Ok(state.panic("The register specified is not decodable!"));
                }
            }
        }
        None => message.to_string(),
    };

    state.execution_state = ExecutionState::Running;

    state.messages.push_back(full_message);
    state.program_counter += 1;

    Ok(ActionPerformed::None)
}

pub async fn perform_alg<'a, P: PuzzleState>(
    puzzle: PuzzleIdx,
    alg: &'a Algorithm,
    state: &mut InterpreterState<P>,
) -> Result<ActionPerformed<'a>, P::Error> {
    state.execution_state = ExecutionState::Running;
    state
        .puzzle_states
        .puzzle_state_mut(puzzle)
        .compose_into(alg)
        .await?;

    state.program_counter += 1;

    Ok(ActionPerformed::Added { puzzle, alg })
}

pub async fn perform_solve<'a, P: PuzzleState>(
    instr: PuzzleIdx,
    state: &mut InterpreterState<P>,
) -> Result<ActionPerformed<'a>, P::Error> {
    state.puzzle_states.puzzle_state_mut(instr).solve().await?;

    state.program_counter += 1;

    Ok(ActionPerformed::Solved(instr))
}

pub async fn perform_repeat_until<'a, P: PuzzleState>(
    puzzle_idx: PuzzleIdx,
    facelets: &'a Facelets,
    alg: &'a Algorithm,
    state: &mut InterpreterState<P>,
) -> Result<ActionPerformed<'a>, P::Error> {
    state
        .puzzle_states
        .puzzle_state_mut(puzzle_idx)
        .repeat_until(facelets.facelets(), alg)
        .await?;

    state.program_counter += 1;

    Ok(ActionPerformed::RepeatedUntil {
        puzzle_idx,
        facelets,
        alg,
    })
}

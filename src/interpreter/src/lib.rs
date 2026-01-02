#![warn(clippy::pedantic)]
#![allow(clippy::too_many_lines)]

mod instructions;
pub mod puzzle_states;

use std::{collections::VecDeque, mem, sync::Arc};

use instructions::do_instr;
use puzzle_states::{PuzzleState, PuzzleStates};
use puzzle_theory::{numbers::{I, Int, U}, permutations::Algorithm};
use qter_core::{
    ByPuzzleType, Facelets,  Instruction,  Program, PuzzleIdx, SeparatesByPuzzleType,
    StateIdx, TheoreticalIdx, 
};

pub struct PuzzleAndRegister;

impl SeparatesByPuzzleType for PuzzleAndRegister {
    type Theoretical<'s> = TheoreticalIdx;

    type Puzzle<'s> = (PuzzleIdx, Algorithm, Facelets);
}

/// If the interpreter is paused, this represents the reason why.
#[derive(Debug)]
pub enum PausedState {
    Halt {
        maybe_puzzle_idx_and_register: Option<ByPuzzleType<'static, PuzzleAndRegister>>,
    },
    Input {
        max_input: Int<U>,
        data: ByPuzzleType<'static, PuzzleAndRegister>,
    },
    Panicked,
}

/// Whether the interpreter can be stepped forward or is paused for some reason
pub enum ExecutionState {
    Running,
    Paused(PausedState),
}

pub struct InterpreterState<P: PuzzleState> {
    puzzle_states: PuzzleStates<P>,
    program_counter: usize,
    messages: VecDeque<String>,
    execution_state: ExecutionState,
}

/// An interpreter for a qter program
pub struct Interpreter<P: PuzzleState> {
    state: InterpreterState<P>,
    program: Arc<Program>,
}

pub struct FaceletsByType;

impl SeparatesByPuzzleType for FaceletsByType {
    type Theoretical<'s> = ();

    type Puzzle<'s> = &'s Facelets;
}

pub struct FailedSolvedGoto;

impl SeparatesByPuzzleType for FailedSolvedGoto {
    type Theoretical<'s> = TheoreticalIdx;

    type Puzzle<'s> = (PuzzleIdx, &'s Facelets);
}

pub struct SucceededSolvedGoto {
    jumped_to: usize,
}

impl SeparatesByPuzzleType for SucceededSolvedGoto {
    type Theoretical<'s> = (Self, TheoreticalIdx);

    type Puzzle<'s> = (Self, PuzzleIdx, &'s Facelets);
}

pub struct Added;

impl SeparatesByPuzzleType for Added {
    type Theoretical<'s> = (TheoreticalIdx, Int<U>);

    type Puzzle<'s> = (PuzzleIdx, &'s Algorithm);
}

/// The action performed by the instruction that was just executed
pub enum ActionPerformed<'s> {
    None,
    Paused,
    Goto {
        instruction_idx: usize,
    },
    FailedSolvedGoto(ByPuzzleType<'s, FailedSolvedGoto>),
    SucceededSolvedGoto(ByPuzzleType<'s, SucceededSolvedGoto>),
    Added(ByPuzzleType<'s, Added>),
    Solved(ByPuzzleType<'static, StateIdx>),
    RepeatedUntil {
        puzzle_idx: PuzzleIdx,
        facelets: &'s Facelets,
        alg: &'s Algorithm,
    },
    Panicked,
}

impl<P: PuzzleState> InterpreterState<P> {
    /// Return the instruction index to be executed next
    #[must_use]
    pub fn program_counter(&self) -> usize {
        self.program_counter
    }

    /// Get the current execution state of the interpreter
    #[must_use]
    pub fn execution_state(&self) -> &ExecutionState {
        &self.execution_state
    }

    /// Get the message queue of the interpreter
    pub fn messages(&mut self) -> &mut VecDeque<String> {
        &mut self.messages
    }

    fn panic<'x>(&mut self, message: &str) -> ActionPerformed<'x> {
        self.execution_state = ExecutionState::Paused(PausedState::Panicked);
        self.messages.push_back(format!("Panicked: {message}"));
        ActionPerformed::Panicked
    }
}

impl<P: PuzzleState> Interpreter<P> {
    /// Get the program currently being executed
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// Get the current state of the interpreter
    #[must_use]
    pub fn state(&self) -> &InterpreterState<P> {
        &self.state
    }

    /// Get the current state of the interpreter mutably
    #[must_use]
    pub fn state_mut(&mut self) -> &mut InterpreterState<P> {
        &mut self.state
    }

    /// Create a new interpreter from a program and initial states for registers
    ///
    /// If an initial state isn't specified, it defaults to zero.
    #[must_use]
    pub fn new(program: Arc<Program>, args: P::InitializationArgs) -> Self where P::InitializationArgs: Clone {
        let state = InterpreterState {
            puzzle_states: PuzzleStates::new(&program, args),
            program_counter: 0,
            messages: VecDeque::new(),
            execution_state: ExecutionState::Running,
        };

        Interpreter { state, program }
    }

    /// Create a new interpreter from a program and initial states for registers, while assuming that the program only contains one puzzle.
    ///
    /// If an initial state isn't specified, it defaults to zero.
    #[must_use]
    pub fn new_only_one_puzzle(program: Arc<Program>, args: P::InitializationArgs) -> Self {
        let state = InterpreterState {
            puzzle_states: PuzzleStates::new_only_one_puzzle(&program, args),
            program_counter: 0,
            messages: VecDeque::new(),
            execution_state: ExecutionState::Running,
        };

        Interpreter { state, program }
    }

    /// Execute one instruction
    pub fn step(&mut self) -> ActionPerformed<'_> {
        if let ExecutionState::Paused(_) = self.state.execution_state() {
            return ActionPerformed::Paused;
        }
        let Some(instruction) = self.program.instructions.get(self.state.program_counter) else {
            return self.state.panic(
                "Execution fell through the end of the program without reaching a halt instruction!"
            );
        };

        match &**instruction {
            &Instruction::Goto { instruction_idx } => {
                self.state.program_counter = instruction_idx;
                self.state.execution_state = ExecutionState::Running;

                ActionPerformed::Goto { instruction_idx }
            }
            Instruction::SolvedGoto(instr) => do_instr(instr, &mut self.state),
            Instruction::Input(instr) => do_instr(instr, &mut self.state),
            Instruction::Halt(instr) => do_instr(instr, &mut self.state),
            Instruction::Print(instr) => do_instr(instr, &mut self.state),
            Instruction::PerformAlgorithm(instr) => do_instr(instr, &mut self.state),
            Instruction::Solve(instr) => do_instr(instr, &mut self.state),
            Instruction::RepeatUntil(instr) => do_instr(instr, &mut self.state),
        }
    }

    /// Execute instructions until an input or halt instruction is reached
    ///
    /// Returns details of the paused state reached
    ///
    /// # Panics
    ///
    /// Panics if the interpreter is not in a paused state
    pub fn step_until_halt(&mut self) -> &PausedState {
        loop {
            // println!("{}", self.state.program_counter);
            if let ActionPerformed::Paused | ActionPerformed::Panicked = self.step() {
                break;
            }
        }
        match self.state.execution_state() {
            ExecutionState::Paused(v) => v,
            ExecutionState::Running => panic!("Cannot be halted while running"),
        }
    }

    /// Give an input to the interpreter, returning the puzzle index and the algorithm performed `value` times if applicable
    ///
    /// # Errors
    ///
    /// Returns an error if the input is out of bounds
    ///
    /// # Panics
    ///
    /// Panics if the interpreter is not executing an `input` instruction
    pub fn give_input(&mut self, value: Int<I>) -> Result<ByPuzzleType<'static, InputRet>, String> {
        let &ExecutionState::Paused(PausedState::Input { max_input, data: _ }) =
            &self.state.execution_state
        else {
            panic!("The interpreter isn't in an input state");
        };

        if value > max_input {
            return Err(format!("Your input must not be greater than {max_input}."));
        }
        if value < -max_input {
            return Err(format!("Your input must not be less than {}.", -max_input));
        }

        // The code is weird to appease the borrow checker

        let ExecutionState::Paused(PausedState::Input { max_input: _, data }) =
            mem::replace(&mut self.state.execution_state, ExecutionState::Running)
        else {
            unreachable!("Checked before")
        };

        let ret = match data {
            ByPuzzleType::Theoretical(idx) => {
                self.state
                    .puzzle_states
                    .theoretical_state_mut(idx)
                    .add_to_i(value);

                ByPuzzleType::Theoretical(idx)
            }
            ByPuzzleType::Puzzle((idx, mut algorithm, _)) => {
                let puzzle = self.state.puzzle_states.puzzle_state_mut(idx);
                algorithm.exponentiate(value);

                puzzle.compose_into(&algorithm);

                ByPuzzleType::Puzzle((idx, algorithm))
            }
        };

        self.state.execution_state = ExecutionState::Running;
        self.state.program_counter += 1;

        Ok(ret)
    }
}

pub struct InputRet;

impl SeparatesByPuzzleType for InputRet {
    type Theoretical<'s> = TheoreticalIdx;

    type Puzzle<'s> = (PuzzleIdx, Algorithm);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Interpreter, PausedState, puzzle_states::SimulatedPuzzle};
    use compiler::{compile, q_emitter::emit_q};
    use internment::ArcIntern;
    use puzzle_theory::{puzzle_geometry::parsing::puzzle, span::File};
    use qter_core::architectures::{new_from_effect, with_presets};
    use std::sync::Arc;
    use pretty_assertions::{assert_eq, assert_ne, assert_str_eq};

    #[test]
    fn facelets_solved() {
        let perm_group = with_presets(puzzle("3x3"));

        let mut cube: SimulatedPuzzle =
            SimulatedPuzzle::initialize(Arc::clone(&perm_group.perm_group), ());

        // Remember that the decoder will subtract the smallest facelet found in the definition to make it zero based
        assert!(cube.facelets_solved(&[0, 8, 16, 24]));

        perm_group
            .perm_group
            .compose_generators_into(&mut cube.state, [ArcIntern::from("U")].iter())
            .unwrap();

        assert!(cube.facelets_solved(&[0, 12, 15, 7, 40]));

        assert!(!cube.facelets_solved(&[1, 12, 15, 7, 24]));
    }

    #[test]
    fn complicated_solved_decode_test() {
        let perm_group = with_presets(puzzle("3x3"));

        let arch = perm_group
            .get_preset(&[Int::from(210_u64), Int::from(24_u64)])
            .unwrap();

        let a_facelets = arch.registers()[0].signature_facelets();
        let b_facelets = arch.registers()[1].signature_facelets();

        let a_permutation = new_from_effect(&arch, vec![(0, Int::one())]);
        let b_permutation = new_from_effect(&arch, vec![(1, Int::one())]);

        let mut cube: SimulatedPuzzle =
            SimulatedPuzzle::initialize(Arc::clone(&perm_group.perm_group), ());

        for i in 1..=23 {
            cube.state.compose_into(b_permutation.permutation());
            assert_eq!(
                cube.print(b_facelets.facelets(), &b_permutation).unwrap(),
                Int::from(i)
            );
            assert!(!cube.facelets_solved(b_facelets.facelets()));
        }

        cube.state.compose_into(b_permutation.permutation());
        assert!(cube.facelets_solved(b_facelets.facelets()));
        assert_eq!(
            cube.print(b_facelets.facelets(), &b_permutation).unwrap(),
            Int::<U>::zero()
        );

        for i in 0..24 {
            println!("{i}");
            for j in 0..210 {
                assert_eq!(
                    cube.print(b_facelets.facelets(), &b_permutation).unwrap(),
                    Int::from(i)
                );
                assert_eq!(
                    cube.print(a_facelets.facelets(), &a_permutation).unwrap(),
                    Int::from(j)
                );

                cube.state.compose_into(a_permutation.permutation());
            }

            cube.state.compose_into(b_permutation.permutation());
        }
    }

    #[test]
    fn modulus() {
        let code = "
            .registers {
                B, A ← 3x3 builtin (24, 210)
            }

                input \"Number to modulus:\" A
            loop:
                print \"A is now\" A
                add B 13
            decrement:
                solved-goto B loop
                solved-goto A fix
                add A 209
                add B 23
                goto decrement
            fix:
                solved-goto B finalize
                add A 209
                add B 23
                goto fix
            finalize:
                add A 13
                halt \"The modulus is\" A
        ";

        let program = match compile(&File::from(code), |_| unreachable!()) {
            Ok(v) => v,
            Err(e) => panic!("{e:?}"),
        };

        let mut interpreter: Interpreter<SimulatedPuzzle> = Interpreter::new(Arc::new(program), ());

        assert!(match interpreter.step_until_halt() {
            PausedState::Input {
                max_input,
                data: ByPuzzleType::Puzzle(_),
            } => *max_input == Int::from(209),
            _ => false,
        });

        assert!(interpreter.give_input(Int::from(133_u64)).is_ok());

        assert!(matches!(
            interpreter.step_until_halt(),
            PausedState::Halt {
                maybe_puzzle_idx_and_register: Some(ByPuzzleType::Puzzle((PuzzleIdx(0), _, _))),
            }
        ));

        let expected_output = [
            "Number to modulus: (max input 209)",
            "A is now 133",
            "A is now 120",
            "A is now 107",
            "A is now 94",
            "A is now 81",
            "A is now 68",
            "A is now 55",
            "A is now 42",
            "A is now 29",
            "A is now 16",
            "A is now 3",
            "The modulus is 3",
        ];

        assert_eq!(
            expected_output.len(),
            interpreter.state_mut().messages().len(),
            "{:?}",
            interpreter.state_mut().messages()
        );

        for (message, expected) in interpreter
            .state()
            .messages
            .iter()
            .zip(expected_output.iter())
        {
            assert_eq!(message, expected);
        }
    }

    #[test]
    fn modulus_2() {
        let code = "
            .registers {
                A, B ← 3x3 builtin (90, 90)
            }

                input \"Number to modulus:\" A
            loop:
                print \"A is now\" A
                solved-goto A%9 finalize
                add B 1
                add A 89
                goto loop
            finalize:
                halt \"The modulus is\" B
        ";

        let program = match compile(&File::from(code), |_| unreachable!()) {
            Ok(v) => v,
            Err(e) => panic!("{e:?}"),
        };

        let mut interpreter: Interpreter<SimulatedPuzzle> = Interpreter::new(Arc::new(program), ());

        let halted_state = interpreter.step_until_halt();
        assert!(
            match halted_state {
                PausedState::Input {
                    max_input,
                    data: ByPuzzleType::Puzzle(_),
                } => *max_input == Int::from(89),
                _ => false,
            },
            "{halted_state:?}"
        );

        assert!(interpreter.give_input(Int::from(77_u64)).is_ok());

        assert!(matches!(
            interpreter.step_until_halt(),
            PausedState::Halt {
                maybe_puzzle_idx_and_register: Some(ByPuzzleType::Puzzle((PuzzleIdx(0), _, _))),
            }
        ));

        let expected_output = [
            "Number to modulus: (max input 89)",
            "A is now 77",
            "A is now 76",
            "A is now 75",
            "A is now 74",
            "A is now 73",
            "A is now 72",
            "The modulus is 5",
        ];

        assert_eq!(
            expected_output.len(),
            interpreter.state_mut().messages().len(),
            "{:?}",
            interpreter.state_mut().messages()
        );

        for (message, expected) in interpreter
            .state()
            .messages
            .iter()
            .zip(expected_output.iter())
        {
            assert_eq!(message, expected);
        }
    }

    #[test]
    fn fib() {
        // TODO: a test directory of qat files?
        let code = "
            .registers {
                D, C, B, A ← 3x3 builtin (9, 10, 18, 30)
            }

                input \"Which Fibonacci number to calculate:\" D
                solved-goto D do_if_1
                goto after_if_1
            do_if_1:
                halt \"The number is: 0\"
            after_if_1:
                add B 1
            continue_1:
                add D 8
                solved-goto D do_if_2
                goto after_if_2
            do_if_2:
                halt \"The number is\" B
            after_if_2:
            continue_2:
                solved-goto B break_2
                add B 17
                add A 1
                add C 1
                goto continue_2
            break_2:
                add D 8
                solved-goto D do_if_3
                goto after_if_3
            do_if_3:
                halt \"The number is\" A
            after_if_3:
            continue_3:
                solved-goto A break_3
                add A 29
                add C 1
                add B 1
                goto continue_3
            break_3:
                add D 8
                solved-goto D do_if_4
                goto after_if_4
            do_if_4:
                halt \"The number is\" C
            after_if_4:
            continue_4:
                solved-goto C break_4
                add C 9
                add B 1
                add A 1
                goto continue_4
            break_4:
                goto continue_1
        ";

        let program = match compile(&File::from(code), |_| unreachable!()) {
            Ok(v) => v,
            Err(e) => panic!("{e:?}"),
        };

        let q = emit_q(&program).unwrap();

        assert_str_eq!(q, r#"Puzzles
A: 3x3

0  | input "Which Fibonacci number to calculate:"
           B2 U2 L F' R B L2 D2 B R' F L
           max-input 8
1  | solved-goto UBL 3
2  | goto 4
3  | halt "The number is: 0"
4  | D L' F L2 B L' F' L B' D' L'
5  | L' F' R B' D2 L2 B' R' F L' U2 B2
6  | solved-goto UBL 8
7  | goto 9
8  | halt "The number is"
          L D B L' F L B' L2 F' L D'
          counting-until UFL DL
9  | repeat until UFL DL solved
            L U' B R' L B' L' U'
            L U R2 B R2 D2 R2 D'
10 | L' F' R B' D2 L2 B' R' F L' U2 B2
11 | solved-goto UBL 13
12 | goto 14
13 | halt "The number is"
          F2 L2 U2 D' R U' B L' B L' U'
          counting-until FR DFR
14 | repeat until FR DFR solved
            D' B' U2 B D' F' D L' D2
            F' R' D2 F2 R F2 R2 U' R'
15 | L' F' R B' D2 L2 B' R' F L' U2 B2
16 | solved-goto UBL 18
17 | goto 19
18 | halt "The number is"
          U L' R' F' U' F' L' F2 L U R
          counting-until UB
19 | repeat until UB solved
            B R2 D' R B D F2 U2 D'
            F' L2 F D2 F B2 D' L' U'
20 | goto 5
"#);

        let mut interpreter: Interpreter<SimulatedPuzzle> = Interpreter::new(Arc::new(program), ());

        assert!(match interpreter.step_until_halt() {
            PausedState::Input {
                max_input,
                data: ByPuzzleType::Puzzle(_),
            } => *max_input == Int::from(8),
            _ => false,
        });

        assert!(interpreter.give_input(Int::from(8_u64)).is_ok());

        assert!(matches!(
            interpreter.step_until_halt(),
            PausedState::Halt {
                maybe_puzzle_idx_and_register: Some(ByPuzzleType::Puzzle((PuzzleIdx(0), _, _))),
            }
        ));

        let expected_output = [
            "Which Fibonacci number to calculate: (max input 8)",
            "The number is 21",
        ];

        assert_eq!(
            expected_output.len(),
            interpreter.state_mut().messages().len(),
            "{:?}",
            interpreter.state_mut().messages()
        );

        for (message, expected) in interpreter
            .state()
            .messages
            .iter()
            .zip(expected_output.iter())
        {
            assert_eq!(message, expected);
        }
    }

    #[test]
    fn add_coalesce() {
        let code = "
            .registers {
                A, B <- 3x3 builtin (90, 90)
                C, D <- 3x3 builtin (90, 90)
                E    <- theoretical 90
                F    <- theoretical 90
            }

            -- These should be coalesced into just four instructions
            add A 1
            add E 1
            add C 1
            add B 1
            add F 1
            add D 1
            add A 1
            add E 1
            add C 1
            add B 1
            add F 1
            add D 1

            print \"A\" A
            print \"B\" B
            print \"C\" C
            print \"D\" D
            print \"E\" E
            print \"F\" F

            halt \"Done\"
        ";

        let program = match compile(&File::from(code), |_| unreachable!()) {
            Ok(v) => v,
            Err(e) => panic!("{e:?}"),
        };

        assert_eq!(program.instructions.len(), 4 + 6 + 1);

        let mut interpreter: Interpreter<SimulatedPuzzle> = Interpreter::new(Arc::new(program), ());

        let expected_output = ["A 2", "B 2", "C 2", "D 2", "E 2", "F 2", "Done"];

        assert!(matches!(
            interpreter.step_until_halt(),
            PausedState::Halt {
                maybe_puzzle_idx_and_register: None,
            }
        ));

        assert_eq!(
            expected_output.len(),
            interpreter.state_mut().messages().len(),
            "{:?}",
            interpreter.state_mut().messages()
        );

        for (message, expected) in interpreter
            .state()
            .messages
            .iter()
            .zip(expected_output.iter())
        {
            assert_eq!(message, expected);
        }
    }

    #[test]
    fn repeat_until() {
        let code = "
            .registers {
                A, B <- 3x3 builtin (90, 90)
            }

            add A 1

            -- Two repeat untils
            spot1:
                solved-goto A spot2
                add A 89
                add B 2
                goto spot1
            spot2:
                solved-goto B spot3
                add B 89
                add A 2
                goto spot2
            spot3:

            -- Two repeat untils
                goto spot5
            spot4:
                add A 89
                add B 2
            spot5:
                solved-goto A spot6
                goto spot4
            spot6:
                goto spot8
            spot7:
                add A 2
                add B 89
            spot8:
                solved-goto B spot9
                goto spot7
            spot9:

            -- One repeat until

            goto spot11

            spot10:
                add B 1
            spot11:
                solved-goto A spot12
                add A 89
                add B 1
                goto spot10
            spot12:

            -- One algorithm + one repeat until

            spot13:
                add B 89
                add A 2
                solved-goto B spot14
                goto spot13
            spot14:
                
                halt \"A=\" A
        ";

        let program = match compile(&File::from(code), |_| unreachable!()) {
            Ok(v) => v,
            Err(e) => panic!("{e:?}"),
        };

        // println!("{:#?}", program);
        assert_eq!(program.instructions.len(), 1 + 2 + 2 + 1 + 2 + 1);

        let mut interpreter: Interpreter<SimulatedPuzzle> = Interpreter::new(Arc::new(program), ());

        let expected_output = ["A= 64"];

        assert!(matches!(
            interpreter.step_until_halt(),
            PausedState::Halt {
                maybe_puzzle_idx_and_register: Some(ByPuzzleType::Puzzle((PuzzleIdx(0), _, _))),
            }
        ));

        assert_eq!(
            expected_output.len(),
            interpreter.state_mut().messages().len(),
            "{:?}",
            interpreter.state_mut().messages()
        );

        for (message, expected) in interpreter
            .state()
            .messages
            .iter()
            .zip(expected_output.iter())
        {
            assert_eq!(message, expected);
        }
    }

    #[test]
    fn repeat_until_two_cubes() {
        let code = "
            .registers {
                A <- 3x3 builtin (1260)
                B <- 3x3 builtin (1260)
            }

            -- Should not be converted to a repeat until
            -- 3 instructions
            spot1:
                solved-goto B spot2
                add A 89
                goto spot1
            spot2:

            -- 4 instructions
                goto spot5
            spot4:
                add A 89
            spot5:
                solved-goto B spot6
                goto spot4
            spot6:

            -- 4 instructions
            spot10:
                add A 89
            spot11:
                solved-goto B spot12
                add B 1
                goto spot10
            spot12:
                
                halt \"A=\" A
        ";

        let program = match compile(&File::from(code), |_| unreachable!()) {
            Ok(v) => v,
            Err(e) => panic!("{e:?}"),
        };

        // println!("{:#?}", program);
        assert_eq!(program.instructions.len(), 3 + 4 + 4 + 1);

        let mut interpreter: Interpreter<SimulatedPuzzle> = Interpreter::new(Arc::new(program), ());

        let expected_output = ["A= 89"];

        assert!(matches!(
            interpreter.step_until_halt(),
            PausedState::Halt {
                maybe_puzzle_idx_and_register: Some(ByPuzzleType::Puzzle((PuzzleIdx(0), _, _))),
            }
        ));

        assert_eq!(
            expected_output.len(),
            interpreter.state_mut().messages().len(),
            "{:?}",
            interpreter.state_mut().messages()
        );

        for (message, expected) in interpreter
            .state()
            .messages
            .iter()
            .zip(expected_output.iter())
        {
            assert_eq!(message, expected);
        }
    }

    #[test]
    fn dead_code() {
        let code = "
            .registers {
                A, B <- 3x3 builtin (90, 90)
            }

                add A 1

                -- Dead code
                goto spot1
            never_jumped_to:
                add A 80
                add B 30
            spot1:

                solved-goto A spot2
            spot2:

                halt \"A=\" A

                -- More dead code
                add A 20
        ";

        let program = match compile(&File::from(code), |_| unreachable!()) {
            Ok(v) => v,
            Err(e) => panic!("{e:?}"),
        };

        assert_eq!(program.instructions.len(), 2);

        let mut interpreter: Interpreter<SimulatedPuzzle> = Interpreter::new(Arc::new(program), ());

        let expected_output = ["A= 1"];

        assert!(matches!(
            interpreter.step_until_halt(),
            PausedState::Halt {
                maybe_puzzle_idx_and_register: Some(ByPuzzleType::Puzzle((PuzzleIdx(0), _, _))),
            }
        ));

        assert_eq!(
            expected_output.len(),
            interpreter.state_mut().messages().len(),
            "{:?}",
            interpreter.state_mut().messages()
        );

        for (message, expected) in interpreter
            .state()
            .messages
            .iter()
            .zip(expected_output.iter())
        {
            assert_eq!(message, expected);
        }
    }

    #[test]
    fn solve() {
        let code = "
            .registers {
                A, B, C <- 3x3 builtin (30, 30, 30)
            }

            -- One algorithm
                add A 20
                add B 10
                add C 15

            -- Reduced to one solve instruction
            spot1:
                solved-goto A spot2
                add A 1
            -- Adding to B will be irrelevant because it will be zeroed out later
                add B 1
                goto spot1
            spot2:
                solved-goto B spot3
                add B 1
                goto spot2
            spot3:
                solved-goto C spot4
                add C 1
                goto spot3
            spot4:

                print \"A=\" A
                print \"B=\" B
                halt \"C=\" C
        ";

        let program = match compile(&File::from(code), |_| unreachable!()) {
            Ok(v) => v,
            Err(e) => panic!("{e:?}"),
        };

        // println!("{program:#?}");
        assert_eq!(program.instructions.len(), 1 + 1 + 3);

        let mut interpreter: Interpreter<SimulatedPuzzle> = Interpreter::new(Arc::new(program), ());

        let expected_output = ["A= 0", "B= 0", "C= 0"];

        assert!(matches!(
            interpreter.step_until_halt(),
            PausedState::Halt {
                maybe_puzzle_idx_and_register: Some(ByPuzzleType::Puzzle((PuzzleIdx(0), _, _))),
            }
        ));

        assert_eq!(
            expected_output.len(),
            interpreter.state_mut().messages().len(),
            "{:?}",
            interpreter.state_mut().messages()
        );

        for (message, expected) in interpreter
            .state()
            .messages
            .iter()
            .zip(expected_output.iter())
        {
            assert_eq!(message, expected);
        }
    }
}

use std::fmt::Debug;
use std::sync::Arc;

use sarlacc::Intern;
use puzzle_theory::numbers::{Int, U};
use puzzle_theory::permutations::{Algorithm, PermutationGroup};
use crate::span::WithSpan;

/// The facelets needed for `solved-goto`
#[derive(Debug, Clone)]
pub struct Facelets {
    facelets: Vec<usize>,
    pieces: Vec<Intern<str>>,
    order: Int<U>,
}

impl Facelets {
    #[must_use]
    pub fn new(facelets: Vec<usize>, pieces: Vec<Intern<str>>, order: Int<U>) -> Self {
        Self {
            facelets,
            pieces,
            order,
        }
    }

    #[must_use]
    pub fn facelets(&self) -> &[usize] {
        &self.facelets
    }

    #[must_use]
    pub fn pieces(&self) -> &[Intern<str>] {
        &self.pieces
    }

    #[must_use]
    pub fn order(&self) -> Int<U> {
        self.order
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PuzzleIdx(pub usize);

/// A qter instruction
#[derive(Debug)]
pub enum Instruction {
    Goto { instruction_idx: usize },
    SolvedGoto {
        instruction_idx: usize,
        puzzle_idx: PuzzleIdx,
        facelets: Facelets,
    },
    Input {
        message: String,
        decoding: Decoding,
    },
    Halt {
        message: String,
        maybe_reg: Option<Decoding>,
    },
    Print {
        message: String,
        maybe_reg: Option<Decoding>,
    },
    PerformAlgorithm{
        puzzle: PuzzleIdx,
        alg: Algorithm,
    },
    Solve(PuzzleIdx),
    RepeatUntil {
        puzzle_idx: PuzzleIdx,
        facelets: Facelets,
        alg: Algorithm,
    },
}

/// How to decode a register
#[derive(Debug, Clone)]
pub struct Decoding {
    pub puzzle_idx: PuzzleIdx,
    pub facelets: Facelets,
    pub algorithm: Algorithm,
}

/// A qter program
#[derive(Debug)]
pub struct Program {
    /// A list of puzzles to be used for registers
    pub puzzles: Vec<WithSpan<Arc<PermutationGroup>>>,
    /// The program itself
    pub instructions: Vec<WithSpan<Instruction>>,
}

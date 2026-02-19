#![expect(async_fn_in_trait)]

// Our interpreter doesn't care whether whether our futures are `Send` and any code using the interpreter is likely to hardcode a particular `PuzzleState` impl so will know statically whether the future is `Send`

#[cfg(feature = "remote_robot")]
mod remote_robot;

#[cfg(feature = "remote_robot")]
pub use remote_robot::*;
use serde::{Deserialize, Serialize};

use std::{convert::Infallible, error::Error, fmt::Display, sync::Arc};

use puzzle_theory::{
    numbers::{I, Int, U, lcm_iter},
    permutations::{Algorithm, Permutation, PermutationGroup},
};
use qter_core::{
    Program, PuzzleIdx, TheoreticalIdx,
    architectures::{chromatic_orders_by_facelets, decode},
};
use tokio_stream::StreamExt;

/// An instance of a theoretical register. Analagous to the `Puzzle` structure.
pub struct TheoreticalState {
    value: Int<U>,
    order: Int<U>,
}

impl TheoreticalState {
    pub fn add_to_i(&mut self, amt: Int<I>) {
        self.add_to(amt % self.order);
    }

    pub fn add_to(&mut self, amt: Int<U>) {
        self.value += amt % self.order;

        if self.value >= self.order {
            self.value -= self.order;
        }
    }

    pub fn zero_out(&mut self) {
        self.value = Int::zero();
    }

    #[must_use]
    pub fn order(&self) -> Int<U> {
        self.order
    }

    #[must_use]
    pub fn value(&self) -> Int<U> {
        self.value
    }
}

pub trait PuzzleState {
    type InitializationArg;
    type Error;

    /// Initialize the `Puzzle` in the solved state
    async fn initialize(
        perm_group: Arc<PermutationGroup>,
        args: Self::InitializationArg,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// Perform an algorithm on the puzzle state
    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Self::Error>;

    /// Check whether the given facelets are solved
    async fn facelets_solved(&mut self, facelets: &[usize]) -> Result<bool, Self::Error>;

    /// Decode the permutation using the register generator and the given facelets.
    ///
    /// In general, an arbitrary scramble cannot be decoded. If this is the case, the function will return `None`.
    ///
    /// This function should not alter the cube state unless it returns `None`.
    async fn print(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<Int<U>>, Self::Error>;

    /// Decode the register without requiring the cube state to be unaltered.
    async fn halt(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<Int<U>>, Self::Error> {
        self.print(facelets, generator).await
    }

    /// Repeat the algorithm until the given facelets are solved.
    ///
    /// Returns None if the facelets cannot be solved by repeating the algorithm.
    async fn repeat_until(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<()>, Self::Error>;

    /// Bring the puzzle to the solved state
    async fn solve(&mut self) -> Result<(), Self::Error>;
}

pub trait RobotLike {
    type InitializationArg;
    type Error;

    /// Initialize the puzzle. The puzzle is expected to be initialized in the solved state, so the `initialize` call should solve it if necessary.
    async fn initialize(
        perm_group: Arc<PermutationGroup>,
        args: Self::InitializationArg,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// Perform an algorithm on the puzzle.
    ///
    /// It is not guaranteed that all moves are physically completed by the time the future finishes, however the implementor guarantees that that wouldn't cause inconsistency. If this method returns an error, the puzzle may be in an unspecified intermediate state.
    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Self::Error>;

    // Wait for all queued moves to finish. Returns a oneshot that will be triggered either when all previously queued moves are finished, or
    // async fn await_moves(
    //     &mut self,
    // ) -> Result<impl Future<Output = Result<(), Self::Error>>, Self::Error>;

    /// Return the puzzle state as a permutation
    async fn take_picture(&mut self) -> Result<&Permutation, Self::Error>;

    /// Solve the current cube state. Same guarantees as `compose_into`.
    async fn solve(&mut self) -> Result<(), Self::Error> {
        let mut state = self.take_picture().await?.clone();
        state.invert();
        self.compose_perm(&state).await
    }

    /// Compose a permutation to the robot; used for solving an unknown permutation. Same guarantees as `compose_into`.
    async fn compose_perm(&mut self, perm: &Permutation) -> Result<(), Self::Error>;
}

pub struct RobotState<R: RobotLike> {
    robot: R,
    perm_group: Arc<PermutationGroup>,
}

impl<R: RobotLike> RobotState<R> {
    async fn repeat_until(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<Int<U>>, R::Error> {
        let mut sum = Int::<U>::zero();

        let chromatic_orders = chromatic_orders_by_facelets(generator);
        let order = lcm_iter(facelets.iter().map(|&i| chromatic_orders[i]));

        while !self.facelets_solved(facelets).await? {
            sum += Int::<U>::one();

            if sum >= order {
                eprintln!(
                    "Decoding failure! Performed as many cycles as the size of the register."
                );
                return Ok(None);
            }

            self.compose_into(generator).await?;
        }

        Ok(Some(sum))
    }
}

impl<R: RobotLike> PuzzleState for RobotState<R> {
    type InitializationArg = R::InitializationArg;
    type Error = R::Error;

    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Self::Error> {
        self.robot.compose_into(alg).await
    }

    async fn initialize(
        perm_group: Arc<PermutationGroup>,
        args: Self::InitializationArg,
    ) -> Result<Self, Self::Error> {
        Ok(RobotState {
            perm_group: Arc::clone(&perm_group),
            robot: R::initialize(perm_group, args).await?,
        })
    }

    async fn facelets_solved(&mut self, facelets: &[usize]) -> Result<bool, Self::Error> {
        let state = self.robot.take_picture().await?;

        for &facelet in facelets {
            let maps_to = state.mapping().get(facelet);
            if self.perm_group.facelet_colors()[maps_to]
                != self.perm_group.facelet_colors()[facelet]
            {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn print(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<Int<U>>, Self::Error> {
        let before = self.robot.take_picture().await?.to_owned();

        let Some(c) = self.halt(facelets, generator).await? else {
            return Ok(None);
        };

        let mut exponentiated = generator.to_owned();
        exponentiated.exponentiate(c.into());

        self.compose_into(&exponentiated).await?;

        if &before != self.robot.take_picture().await? {
            eprintln!("Printing did not return the cube to the original state!");
            return Ok(None);
        }
        Ok(Some(c))
    }

    async fn halt(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<Int<U>>, Self::Error> {
        let mut generator = generator.to_owned();
        generator.exponentiate(-Int::<U>::one());

        // `repeat until` has the same behavior as `halt`
        self.repeat_until(facelets, &generator).await
    }

    async fn repeat_until(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<()>, Self::Error> {
        Ok(self.repeat_until(facelets, generator).await?.map(|_| ()))
    }

    async fn solve(&mut self) -> Result<(), Self::Error> {
        self.robot.solve().await
    }
}

#[derive(Clone, Debug)]
pub struct SimulatedPuzzle {
    perm_group: Arc<PermutationGroup>,
    pub(crate) state: Permutation,
}

impl SimulatedPuzzle {
    /// Get the state underlying the puzzle
    pub fn puzzle_state(&self) -> &Permutation {
        &self.state
    }
}

impl PuzzleState for SimulatedPuzzle {
    type InitializationArg = ();
    type Error = Infallible;

    async fn initialize(perm_group: Arc<PermutationGroup>, (): ()) -> Result<Self, Infallible> {
        Ok(SimulatedPuzzle {
            state: Permutation::identity(),
            perm_group,
        })
    }

    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Infallible> {
        self.state.compose_into(alg.permutation());
        Ok(())
    }

    async fn facelets_solved(&mut self, facelets: &[usize]) -> Result<bool, Infallible> {
        for &facelet in facelets {
            let maps_to = self.state.mapping().get(facelet);
            if self.perm_group.facelet_colors()[maps_to]
                != self.perm_group.facelet_colors()[facelet]
            {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn print(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<Int<U>>, Infallible> {
        Ok(decode(&self.state, facelets, generator))
    }

    async fn solve(&mut self) -> Result<(), Infallible> {
        self.state = Permutation::identity();
        Ok(())
    }

    async fn repeat_until(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<()>, Infallible> {
        let mut generator = generator.to_owned();
        generator.exponentiate(-Int::<U>::one());
        let Some(v) = decode(&self.state, facelets, &generator) else {
            return Ok(None);
        };
        generator.exponentiate(-v);
        <Self as PuzzleState>::compose_into(self, &generator).await?;
        Ok(Some(()))
    }
}

impl RobotLike for SimulatedPuzzle {
    type InitializationArg = ();
    type Error = Infallible;

    async fn initialize(perm_group: Arc<PermutationGroup>, (): ()) -> Result<Self, Infallible> {
        <Self as PuzzleState>::initialize(perm_group, ()).await
    }

    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Infallible> {
        <Self as PuzzleState>::compose_into(self, alg).await
    }

    async fn take_picture(&mut self) -> Result<&Permutation, Infallible> {
        Ok(self.puzzle_state())
    }

    async fn solve(&mut self) -> Result<(), Self::Error> {
        self.state = Permutation::identity();
        Ok(())
    }

    async fn compose_perm(&mut self, perm: &Permutation) -> Result<(), Infallible> {
        self.state.compose_into(perm);
        Ok(())
    }
}

/// Simulates the current puzzle state and takes actions when there is a mismatch between the simulated puzzle state and the result of `R::take_picture`.
pub struct WrapSimulatedPuzzle<R: RobotLike> {
    robot: R,
    puzzle: Permutation,
    behavior: MismatchBehavior,
}

/// What to do if the simulated puzzle and result of `take_picture` mismatch
#[derive(Clone, Copy, Debug)]
#[derive(Serialize, Deserialize)]
#[serde(tag = "behavior")]
pub enum MismatchBehavior {
    /// Return the simulated state. `R::take_picture` will never be invoked.
    ReturnSimulation,
    /// Return the observed state. Effectively makes this wrapper transparent.
    ReturnObserved,
    /// Return a mismatch error if there is a discrepency.
    Error,
    /// Run `compose_perm` to attempt to fix the discrepency and repeat up to `retry_count` times. If `retry_count` is reached, return the simulated state.
    Fix { retry_count: usize },
}

#[derive(Debug)]
pub enum WrapSimulationErr<E> {
    Robot(E),
    IncorrectObservation {
        found: Permutation,
        expected: Permutation,
    },
}

impl<E> From<E> for WrapSimulationErr<E> {
    fn from(value: E) -> Self {
        Self::Robot(value)
    }
}

impl<E: Display> Display for WrapSimulationErr<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WrapSimulationErr::Robot(r) => r.fmt(f),
            WrapSimulationErr::IncorrectObservation { found, expected } => {
                write!(
                    f,
                    "The picture taken of the puzzle mismatched the simulated puzzle. Found {found}, expected {expected}"
                )
            }
        }
    }
}

impl<E: Error> Error for WrapSimulationErr<E> {}

impl<R: RobotLike> RobotLike for WrapSimulatedPuzzle<R> {
    type InitializationArg = (MismatchBehavior, R::InitializationArg);
    type Error = WrapSimulationErr<R::Error>;

    async fn initialize(
        perm_group: Arc<PermutationGroup>,
        (behavior, args): Self::InitializationArg,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let mut this = WrapSimulatedPuzzle {
            robot: R::initialize(perm_group, args).await?,
            puzzle: Permutation::identity(),
            behavior,
        };

        this.solve().await?;

        Ok(this)
    }

    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Self::Error> {
        self.puzzle.compose_into(alg.permutation());
        Ok(self.robot.compose_into(alg).await?)
    }

    async fn take_picture(&mut self) -> Result<&Permutation, Self::Error> {
        match self.behavior {
            MismatchBehavior::ReturnSimulation => Ok(&self.puzzle),
            MismatchBehavior::ReturnObserved => Ok(self.robot.take_picture().await?),
            MismatchBehavior::Error => {
                let found_state = self.robot.take_picture().await?;

                if found_state == &self.puzzle {
                    Ok(found_state)
                } else {
                    Err(WrapSimulationErr::IncorrectObservation {
                        found: found_state.clone(),
                        expected: self.puzzle.clone(),
                    })
                }
            }
            MismatchBehavior::Fix { retry_count } => {
                let mut retries_left = retry_count;

                while retries_left > 0 {
                    retries_left -= 1;

                    let found_state = self.robot.take_picture().await?;

                    if found_state == &self.puzzle {
                        break;
                    }

                    // Let the true state be T, let the observed state be O.
                    // We need some permutation X to compose into O to get T.
                    // O X = T
                    // X = O⁻¹ T

                    let mut fix = found_state.clone();
                    fix.invert();
                    fix.compose_into(&self.puzzle);

                    self.robot.compose_perm(&fix).await?;
                }

                Ok(&self.puzzle)
            }
        }
    }

    async fn solve(&mut self) -> Result<(), Self::Error> {
        self.puzzle = Permutation::identity();
        self.robot.solve().await?;

        // if `ReturnObserved`, then we want to avoid calling `self.robot.take_picture()` needlessly
        if !matches!(self.behavior, MismatchBehavior::ReturnObserved) {
            // otherwise the behaviour is actually the same as in `take_picture`
            self.take_picture().await?;
        }

        Ok(())
    }

    async fn compose_perm(&mut self, perm: &Permutation) -> Result<(), Self::Error> {
        self.puzzle.compose_into(perm);
        Ok(self.robot.compose_perm(perm).await?)
    }
}

/// A collection of the states of every puzzle and theoretical register
pub(crate) struct PuzzleStates<P: PuzzleState> {
    theoretical_states: Vec<TheoreticalState>,
    puzzle_states: Vec<P>,
}

impl<P: PuzzleState> PuzzleStates<P>
where
    P::InitializationArg: Clone,
{
    pub(crate) async fn new(
        program: &Program,
        args: P::InitializationArg,
    ) -> Result<Self, P::Error> {
        let theoretical_states = program
            .theoretical
            .iter()
            .map(|order| TheoreticalState {
                value: Int::zero(),
                order: **order,
            })
            .collect();

        let puzzle_states = tokio_stream::iter(program.puzzles.iter())
            .then(|perm_group| P::initialize(Arc::clone(perm_group), args.clone()))
            .collect::<Result<Vec<_>, _>>()
            .await?;

        Ok(PuzzleStates {
            theoretical_states,
            puzzle_states,
        })
    }
}

impl<P: PuzzleState> PuzzleStates<P> {
    pub(crate) async fn new_only_one_puzzle(
        program: &Program,
        args: P::InitializationArg,
    ) -> Result<Self, P::Error> {
        let theoretical_states = program
            .theoretical
            .iter()
            .map(|order| TheoreticalState {
                value: Int::zero(),
                order: **order,
            })
            .collect();

        let puzzle_states = if program.puzzles.is_empty() {
            Vec::new()
        } else if program.puzzles.len() == 1 {
            vec![P::initialize(Arc::clone(&program.puzzles[0]), args).await?]
        } else {
            panic!("Expected at most one puzzle in the program");
        };

        Ok(PuzzleStates {
            theoretical_states,
            puzzle_states,
        })
    }

    #[must_use]
    pub fn theoretical_state(&self, idx: TheoreticalIdx) -> &TheoreticalState {
        &self.theoretical_states[idx.0]
    }

    pub fn theoretical_state_mut(&mut self, idx: TheoreticalIdx) -> &mut TheoreticalState {
        &mut self.theoretical_states[idx.0]
    }

    pub fn puzzle_state_mut(&mut self, idx: PuzzleIdx) -> &mut P {
        &mut self.puzzle_states[idx.0]
    }
}

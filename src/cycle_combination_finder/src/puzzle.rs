use std::num::{NonZeroU8, NonZeroU16};

use puzzle_theory::ksolve::{KSolve, KSolveSet};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PuzzleDefCreationError {
    #[error(
        "Orbit constraints must match number of KSolve sets. Expected {expected} but found \
         {actual}"
    )]
    InvalidOrbitConstraintsLength { expected: usize, actual: usize },
    #[error("Puzzle must have at least one orbit")]
    NoOrbits,
}

pub struct PuzzleDef {
    orbit_defs: Vec<OrbitDef>,
    orientation_sum_constraint: OrientationSumConstraint,
    parity_constraint: ParityConstraint,
}

#[derive(Clone, Copy, Debug)]
pub struct OrbitDef {
    pub piece_count: NonZeroU16,
    pub orientation_count: NonZeroU8,
    pub orientation_sum_constraint: OrientationSumConstraint,
    pub parity_constraint: ParityConstraint,
}

#[derive(Clone, Copy, Debug)]
pub enum OrientationSumConstraint {
    Zero,
    None,
}

#[derive(Clone, Copy, Debug)]
pub enum ParityConstraint {
    Even,
    None,
}

// impl From<&KSolveSet> for OrbitDef {
//     fn from(orbit: &KSolveSet) -> Self {
//         Self {
//             piece_count: orbit.piece_count(),
//             orientation_count:
// NonZeroU16::new(u16::from(orbit.orientation_count().get())).unwrap(),
//         }
//     }
// }

impl PuzzleDef {
    /// "Naively" make a [`PuzzleDef`] from a [`KSolve`]. It is naive in the
    /// sense that the fields for orientation and parity constraints are stubbed
    /// in because they are not implemented.
    ///
    /// # Errors
    ///
    /// Returns a [`PuzzleDefCreationError`] if any of its variants are
    /// applicable.
    pub fn from_ksolve_naive(
        ksolve: &KSolve,
        orientation_sum_constraint: OrientationSumConstraint,
        parity_constraint: ParityConstraint,
        orbit_constraints: Vec<(OrientationSumConstraint, ParityConstraint)>,
    ) -> Result<Self, PuzzleDefCreationError> {
        if orbit_constraints.len() != ksolve.sets().len() {
            return Err(PuzzleDefCreationError::InvalidOrbitConstraintsLength {
                expected: ksolve.sets().len(),
                actual: orbit_constraints.len(),
            });
        }
        let orbit_defs = ksolve
            .sets()
            .iter()
            .zip(orbit_constraints)
            .map(
                |(set, (orbit_orientation_sum_constraint, orbit_parity_constraint))| {
                    OrbitDef::from_ksolveset_naive(
                        set,
                        orbit_orientation_sum_constraint,
                        orbit_parity_constraint,
                    )
                },
            )
            .collect::<Vec<_>>();
        Ok(Self {
            orbit_defs,
            orientation_sum_constraint,
            parity_constraint,
        })
    }

    /// "Naively" make a [`PuzzleDef`] from a [`Vec<OrbitDef>`]. It is naive in
    /// the sense that the fields for orientation and parity constraints are
    /// stubbed in because they are not implemented.
    ///
    /// # Errors
    ///
    /// Returns a [`PuzzleDefCreationError`] if any of its variants are
    /// applicable.
    pub fn from_orbit_defs_naive(
        orbit_defs: Vec<OrbitDef>,
        orientation_sum_constraint: OrientationSumConstraint,
        parity_constraint: ParityConstraint,
    ) -> Result<Self, PuzzleDefCreationError> {
        if orbit_defs.is_empty() {
            Err(PuzzleDefCreationError::NoOrbits)
        } else {
            Ok(Self {
                orbit_defs,
                orientation_sum_constraint,
                parity_constraint,
            })
        }
    }

    #[must_use]
    pub fn orbit_defs(&self) -> &[OrbitDef] {
        &self.orbit_defs
    }

    #[must_use]
    pub fn orientation_sum_constraint(&self) -> OrientationSumConstraint {
        self.orientation_sum_constraint
    }

    #[must_use]
    pub fn parity_constraint(&self) -> ParityConstraint {
        self.parity_constraint
    }
}

impl OrbitDef {
    #[must_use]
    pub fn from_ksolveset_naive(
        set: &KSolveSet,
        orientation_sum_constraint: OrientationSumConstraint,
        parity_constraint: ParityConstraint,
    ) -> Self {
        Self {
            piece_count: set.piece_count(),
            orientation_count: set.orientation_count(),
            orientation_sum_constraint,
            parity_constraint,
        }
    }
}

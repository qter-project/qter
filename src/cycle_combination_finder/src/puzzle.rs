use std::num::NonZeroU16;

use bitgauss::BitMatrix;
use puzzle_theory::ksolve::KSolve;
use thiserror::Error;

pub mod cubeN;
pub mod minxN;
pub mod misc;

#[derive(Error, Debug)]
pub enum PuzzleDefCreationError {
    #[error(
        "Orbit constraints must match number of KSolve sets. Expected {expected} but found \
         {actual}"
    )]
    InvalidOrbitConstraintsLength { expected: usize, actual: usize },
    #[error("Puzzle must have at least one orbit")]
    NoOrbits,
    #[error("Even parity constraint contains the duplicated index {0}")]
    DuplicateIndicies(usize),
    #[error(
        "Even parity constraint index is out of bounds. Expected a maximum of {length} but found \
         {actual}"
    )]
    OutOfBounds { length: usize, actual: usize },
    #[error("Orientation count of {0} cannot be 0 or 1")]
    InvalidOrientationCount(u8),
    #[error(
        "You should not supply one orbit as an even constraint. Instead, set the \
         `parity_constraint` field to even on that orbit"
    )]
    SingleParityConstraint,
}

#[derive(Clone, Debug)]
pub struct PuzzleDef {
    orbit_defs: Vec<OrbitDef>,
    even_parity_constraints: BitMatrix,
}

#[derive(Clone, Copy, Debug)]
pub struct OrbitDef {
    pub piece_count: NonZeroU16,
    pub orientation: OrientationStatus,
    pub parity_constraint: ParityConstraint,
}

#[derive(Clone, Copy, Debug)]
pub struct PartialOrbitDef {
    pub piece_count: NonZeroU16,
    pub orientation: OrientationStatus,
}

#[derive(Clone, Copy, Debug)]
pub enum OrientationStatus {
    CanOrient {
        count: u8,
        sum_constraint: OrientationSumConstraint,
    },
    CannotOrient,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OrientationSumConstraint {
    Zero,
    None,
}

#[derive(Clone, Debug)]
pub struct EvenParityConstraints(pub Vec<Vec<usize>>);

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
        orbit_constraints: Vec<(OrientationSumConstraint, ParityConstraint)>,
        even_parity_constraints: EvenParityConstraints,
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
                |(ksolveset, (orbit_orientation_sum_constraint, orbit_parity_constraint))| {
                    let piece_count = ksolveset.piece_count();
                    let orientation = if ksolveset.orientation_count().get() == 1 {
                        OrientationStatus::CannotOrient
                    } else {
                        OrientationStatus::CanOrient {
                            count: ksolveset.orientation_count().get(),
                            sum_constraint: orbit_orientation_sum_constraint,
                        }
                    };
                    OrbitDef {
                        piece_count,
                        orientation,
                        parity_constraint: orbit_parity_constraint,
                    }
                },
            )
            .collect::<Vec<_>>();
        Self::new(orbit_defs, even_parity_constraints)
    }

    /// # Errors
    ///
    /// Returns a [`PuzzleDefCreationError`] if any of its variants are
    /// applicable.
    pub fn new(
        // orbit_defs: Vec<PartialOrbitDef>,
        orbit_defs: Vec<OrbitDef>,
        EvenParityConstraints(raw_even_parity_constraints): EvenParityConstraints,
    ) -> Result<Self, PuzzleDefCreationError> {
        if orbit_defs.is_empty() {
            return Err(PuzzleDefCreationError::NoOrbits);
        }
        orbit_defs
            .iter()
            .try_for_each(|&orbit_def| match orbit_def.orientation {
                OrientationStatus::CanOrient { count, .. } if count == 0 || count == 1 => {
                    Err(PuzzleDefCreationError::InvalidOrientationCount(count))
                }
                _ => Ok(()),
            })?;
        // let even_parity_constraints = if let Some(cols) = raw_even_parity_constraints
        //     .iter()
        //     .flatten()
        //     .copied()
        //     .max()
        //     .map(|orbit_index| orbit_index + 1)
        // {
        let cols = orbit_defs.len();
        let rows = raw_even_parity_constraints.len();
        let mut even_parity_constraints = BitMatrix::zeros(rows, cols);
        for (i, even_parity_constraint) in raw_even_parity_constraints.into_iter().enumerate() {
            for j in even_parity_constraint {
                if j >= cols {
                    return Err(PuzzleDefCreationError::OutOfBounds {
                        length: cols,
                        actual: i,
                    });
                }
                if even_parity_constraints.bit(i, j) {
                    return Err(PuzzleDefCreationError::DuplicateIndicies(j));
                }
                even_parity_constraints.set_bit(i, j, true);
            }
        }
        let pivot_cols = even_parity_constraints.gauss(true);
        let rank = pivot_cols.len();
        let even_parity_constraints = if rank == rows {
            even_parity_constraints
        } else {
            BitMatrix::build(rank, cols, |i, j| even_parity_constraints[(i, j)])
        };
        Ok(Self {
            orbit_defs,
            even_parity_constraints,
        })
    }

    #[must_use]
    pub fn orbit_defs(&self) -> &[OrbitDef] {
        &self.orbit_defs
    }

    #[must_use]
    pub fn even_parity_constraints(&self) -> &BitMatrix {
        &self.even_parity_constraints
    }
}

impl OrbitDef {
    #[must_use]
    pub fn orientation_count(self) -> u8 {
        match self.orientation {
            OrientationStatus::CanOrient { count, .. } => count,
            OrientationStatus::CannotOrient => 1,
        }
    }
}

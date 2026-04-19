use std::{
    fmt::{self, Debug, Formatter},
    num::NonZeroU16,
};

use bitgauss::BitMatrix;
use fxhash::FxHashMap;
use puzzle_theory::ksolve::KSolve;
use thiserror::Error;
use union_find::{QuickUnionUf, UnionBySize, UnionFind};

use crate::{FIRST_133_PRIMES, gauss_jordan_without_zero_rows};

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
    ConstraintIndexOutOfBounds { length: usize, actual: usize },
    #[error("Orientation count of {0} cannot be 0 or 1")]
    InvalidOrientationCount(u8),
    #[error("Orbit has too many pieces. Expected a maximum of {max} but found {actual}")]
    OrbitTooManyPieces { actual: u16, max: u16 },
    #[error("Orbit has too much orientation. Expected a maximum of {max} but found {actual}")]
    OrbitTooMuchOrientation { actual: u8, max: u16 },
}

#[derive(Clone)]
pub struct PuzzleDef<const N: usize> {
    orbit_defs: Vec<OrbitDef>,
    even_parity_constraints: BitMatrix,
    connected_components: Vec<Vec<usize>>,
}

#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct OrbitDef {
    // We need to uphold the invariant that piece_count < PRIME_AFTER_LAST
    pub(crate) piece_count: NonZeroU16,
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

impl<const N: usize> Debug for PuzzleDef<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("PuzzleDef")
            .field("orbit_defs", &self.orbit_defs)
            .finish_non_exhaustive()
    }
}

impl<const N: usize> PuzzleDef<N> {
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
        orbit_constraints: Vec<OrientationSumConstraint>,
        even_parity_constraints: EvenParityConstraints,
    ) -> Result<Self, PuzzleDefCreationError> {
        if orbit_constraints.len() != ksolve.sets().len() {
            return Err(PuzzleDefCreationError::InvalidOrbitConstraintsLength {
                expected: ksolve.sets().len(),
                actual: orbit_constraints.len(),
            });
        }
        let parial_orbit_defs = ksolve
            .sets()
            .iter()
            .zip(orbit_constraints)
            .map(|(ksolveset, orbit_orientation_sum_constraint)| {
                let piece_count = ksolveset.piece_count();
                let orientation = if ksolveset.orientation_count().get() == 1 {
                    OrientationStatus::CannotOrient
                } else {
                    OrientationStatus::CanOrient {
                        count: ksolveset.orientation_count().get(),
                        sum_constraint: orbit_orientation_sum_constraint,
                    }
                };
                PartialOrbitDef {
                    piece_count,
                    orientation,
                }
            })
            .collect::<Vec<_>>();
        Self::new(parial_orbit_defs, even_parity_constraints)
    }

    /// # Errors
    ///
    /// Returns a [`PuzzleDefCreationError`] if any of its variants are
    /// applicable.
    pub fn new(
        partial_orbit_defs: Vec<PartialOrbitDef>,
        EvenParityConstraints(raw_even_parity_constraints): EvenParityConstraints,
    ) -> Result<Self, PuzzleDefCreationError> {
        if partial_orbit_defs.is_empty() {
            return Err(PuzzleDefCreationError::NoOrbits);
        }

        let mut orbit_defs = partial_orbit_defs
            .into_iter()
            .map(
                |PartialOrbitDef {
                     piece_count,
                     orientation,
                 }| {
                    match orientation {
                        OrientationStatus::CanOrient { count, .. } if count == 0 || count == 1 => {
                            return Err(PuzzleDefCreationError::InvalidOrientationCount(count));
                        }
                        _ => (),
                    }
                    if piece_count.get() >= FIRST_133_PRIMES[N] {
                        return Err(PuzzleDefCreationError::OrbitTooManyPieces {
                            max: FIRST_133_PRIMES[N],
                            actual: piece_count.get(),
                        });
                    }
                    let ret = OrbitDef {
                        piece_count,
                        orientation,
                        parity_constraint: ParityConstraint::None,
                    };
                    if u16::from(ret.orientation_count()) >= FIRST_133_PRIMES[N] {
                        return Err(PuzzleDefCreationError::OrbitTooMuchOrientation {
                            max: FIRST_133_PRIMES[N],
                            actual: ret.orientation_count(),
                        });
                    }
                    Ok(ret)
                },
            )
            .collect::<Result<Vec<_>, PuzzleDefCreationError>>()?;

        let cols = orbit_defs.len();
        let rows = raw_even_parity_constraints.len();
        let mut even_parity_constraints = BitMatrix::zeros(rows, cols);
        for (i, even_parity_constraint) in raw_even_parity_constraints.into_iter().enumerate() {
            for j in even_parity_constraint {
                if j >= cols {
                    return Err(PuzzleDefCreationError::ConstraintIndexOutOfBounds {
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

        let pivot_cols = gauss_jordan_without_zero_rows(&mut even_parity_constraints, rows);
        let cols = even_parity_constraints.cols();
        let rows = even_parity_constraints.rows();
        let mut uf = QuickUnionUf::<UnionBySize>::new(cols);
        for free_col in (0..cols).filter(|col| !pivot_cols.contains(col)) {
            for row in (0..rows).filter_map(|row| {
                let constraints_row = even_parity_constraints.row(row);
                if constraints_row.bit(free_col) {
                    Some(constraints_row)
                } else {
                    None
                }
            }) {
                for equal_orbit_index in row
                    .iter()
                    .enumerate()
                    .filter_map(|(i, bit)| if bit { Some(i) } else { None })
                {
                    uf.union(free_col, equal_orbit_index);
                }
            }
        }
        let mut connected_components = FxHashMap::<usize, Vec<usize>>::default();
        for (orbit_index, &root) in uf.link_parent().iter().enumerate() {
            connected_components
                .entry(root)
                .or_default()
                .push(orbit_index);
        }
        let connected_components = connected_components.into_values().collect::<Vec<_>>();

        for singular_component in connected_components
            .iter()
            .filter_map(|connected_component| {
                let [singular_component] = *connected_component.as_slice() else {
                    return None;
                };
                Some(singular_component)
            })
        {
            match (0..rows)
                .filter(|&row| even_parity_constraints[(row, singular_component)])
                .count()
            {
                // This case is handled in possible_orders
                0 => (),
                1 => {
                    orbit_defs[singular_component].parity_constraint = ParityConstraint::Even;
                }
                // It should never be the case that two or more 1s exist in a singular component
                // because:
                // - They must be pivot columns because or they would not be a singular component;
                //   and
                // - You can row-reduce it to either exactly zero or one 1s
                2.. => unreachable!(),
            }
        }
        Ok(Self {
            orbit_defs,
            even_parity_constraints,
            connected_components,
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

    #[must_use]
    pub fn connected_components(&self) -> &[Vec<usize>] {
        &self.connected_components
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

#[cfg(test)]
mod tests {
    #[test_log::test]
    fn edge_cases() {}
}

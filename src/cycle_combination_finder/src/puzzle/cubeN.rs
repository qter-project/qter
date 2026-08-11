use std::sync::LazyLock;

use crate::puzzle::{
    EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef, PuzzleDef,
};

/// Generate the orbit definitions and even parity constraints of an `n`x`n`x`n`
/// cube. Feed this to `PuzzleDef::new` to create a puzzle.
#[allow(clippy::missing_panics_doc)]
#[must_use]
pub fn cube(n: usize) -> (Vec<PartialOrbitDef>, EvenParityConstraints) {
    if n <= 1 {
        return (vec![], EvenParityConstraints(vec![]));
    }

    let mut partial_orbit_dfs = vec![];
    let mut even_parity_constraints = vec![];

    // all cubes n > 1 have 8 corners
    partial_orbit_dfs.push(PartialOrbitDef {
        name: Some("corners".to_string()),
        piece_count: 8.try_into().unwrap(),
        orientation: OrientationStatus::CanOrient {
            count: 3,
            sum_constraint: OrientationSumConstraint::Zero,
        },
    });

    if n % 2 == 1 {
        // odd cubes have 12 edges whose parity is equivalent to corner parity
        partial_orbit_dfs.push(PartialOrbitDef {
            name: Some("edges".to_string()),
            piece_count: 12.try_into().unwrap(),
            orientation: OrientationStatus::CanOrient {
                count: 2,
                sum_constraint: OrientationSumConstraint::Zero,
            },
        });
        even_parity_constraints.push(vec!["edges".to_string(), "corners".to_string()]);

        // odd cubes have n/2 - 1 sets of 24 +centers, which form a + shape. each set's
        // parity is determined by the corners and the wings it shares a slice with
        for c2 in 1..n / 2 {
            partial_orbit_dfs.push(PartialOrbitDef {
                name: Some(format!("+centers{c2}")),
                piece_count: 24.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            });
            even_parity_constraints.push(vec![
                "corners".to_string(),
                format!("wings{c2}"),
                format!("+centers{c2}"),
            ]);
        }
    }

    // the cube has n/2 - 1 sets of 24 wings
    for w in 1..n / 2 {
        partial_orbit_dfs.push(PartialOrbitDef {
            name: Some(format!("wings{w}")),
            piece_count: 24.try_into().unwrap(),
            orientation: OrientationStatus::CannotOrient,
        });
    }

    // the cube has (n/2 - 1)^2 sets of 24 centers
    for c1 in 1..n / 2 {
        for c2 in 1..n / 2 {
            if c1 == c2 {
                // centers with equal indices form an x shape: xcenters. their parity is
                // determined only by the corners, since the associated wing parity doubles
                // and therefore always cancels out
                partial_orbit_dfs.push(PartialOrbitDef {
                    name: Some(format!("xcenters{c1}")),
                    piece_count: 24.try_into().unwrap(),
                    orientation: OrientationStatus::CannotOrient,
                });
                even_parity_constraints.push(vec!["corners".to_string(), format!("xcenters{c1}")]);
            } else {
                // the other centers fall on a skewed slope from the cube's sides: obliques.
                // their parity is determined by the corners and both sets of wings they
                // share a slice with
                partial_orbit_dfs.push(PartialOrbitDef {
                    name: Some(format!("obliques{c1};{c2}")),
                    piece_count: 24.try_into().unwrap(),
                    orientation: OrientationStatus::CannotOrient,
                });
                even_parity_constraints.push(vec![
                    "corners".to_string(),
                    format!("wings{c1}"),
                    format!("wings{c2}"),
                    format!("obliques{c1};{c2}"),
                ]);
            }
        }
    }

    let even_parity_constraints = EvenParityConstraints(
        even_parity_constraints
            .into_iter()
            .map(|names| {
                names
                    .into_iter()
                    .map(|name| {
                        partial_orbit_dfs
                            .iter()
                            .position(|orbit| orbit.name.as_deref() == Some(name.as_str()))
                            .expect("even parity constraint references an unknown orbit")
                    })
                    .collect()
            })
            .collect(),
    );
    (partial_orbit_dfs, even_parity_constraints)
}

pub static CUBE2: LazyLock<PuzzleDef<8>> = LazyLock::new(|| {
    let (orbit_defs, even_parity_constraints) = cube(2);
    PuzzleDef::new(orbit_defs, even_parity_constraints).unwrap()
});

pub static CUBE3: LazyLock<PuzzleDef<8>> = LazyLock::new(|| {
    let (orbit_defs, even_parity_constraints) = cube(3);
    PuzzleDef::new(orbit_defs, even_parity_constraints).unwrap()
});

pub static CUBE4: LazyLock<PuzzleDef<16>> = LazyLock::new(|| {
    let (orbit_defs, even_parity_constraints) = cube(4);
    PuzzleDef::new(orbit_defs, even_parity_constraints).unwrap()
});

pub static CUBE5: LazyLock<PuzzleDef<16>> = LazyLock::new(|| {
    let (orbit_defs, even_parity_constraints) = cube(5);
    PuzzleDef::new(orbit_defs, even_parity_constraints).unwrap()
});

pub static CUBE6: LazyLock<PuzzleDef<16>> = LazyLock::new(|| {
    let (orbit_defs, even_parity_constraints) = cube(6);
    PuzzleDef::new(orbit_defs, even_parity_constraints).unwrap()
});

pub static CUBE7: LazyLock<PuzzleDef<16>> = LazyLock::new(|| {
    let (orbit_defs, even_parity_constraints) = cube(7);
    PuzzleDef::new(orbit_defs, even_parity_constraints).unwrap()
});

pub static CUBE8: LazyLock<PuzzleDef<16>> = LazyLock::new(|| {
    let (orbit_defs, even_parity_constraints) = cube(8);
    PuzzleDef::new(orbit_defs, even_parity_constraints).unwrap()
});

#[cfg(test)]
mod tests {
    use crate::puzzle::{
        EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef,
        cubeN::cube,
    };

    #[test]
    fn trivial_cube() {
        let (orbits, EvenParityConstraints(constraints)) = cube(1);
        assert!(orbits.is_empty());
        assert!(constraints.is_empty());
    }

    #[test]
    fn cube2_matches_hardcoded() {
        assert_eq!(
            cube(2),
            (
                vec![PartialOrbitDef {
                    name: Some("corners".to_string()),
                    piece_count: 8.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 3,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                }],
                EvenParityConstraints(vec![]),
            )
        );
    }

    #[test]
    fn cube3_matches_hardcoded() {
        assert_eq!(
            cube(3),
            (
                vec![
                    PartialOrbitDef {
                        name: Some("corners".to_string()),
                        piece_count: 8.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 3,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                    },
                    PartialOrbitDef {
                        name: Some("edges".to_string()),
                        piece_count: 12.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 2,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                    },
                ],
                EvenParityConstraints(vec![vec![1, 0]]),
            )
        );
    }

    #[test]
    fn cube4_matches_hardcoded() {
        assert_eq!(
            cube(4),
            (
                vec![
                    PartialOrbitDef {
                        name: Some("corners".to_string()),
                        piece_count: 8.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 3,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                    },
                    PartialOrbitDef {
                        name: Some("wings1".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("xcenters1".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                ],
                EvenParityConstraints(vec![vec![0, 2]]),
            )
        );
    }

    #[test]
    fn cube5_matches_hardcoded() {
        assert_eq!(
            cube(5),
            (
                vec![
                    PartialOrbitDef {
                        name: Some("corners".to_string()),
                        piece_count: 8.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 3,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                    },
                    PartialOrbitDef {
                        name: Some("edges".to_string()),
                        piece_count: 12.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 2,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                    },
                    PartialOrbitDef {
                        name: Some("+centers1".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("wings1".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("xcenters1".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                ],
                EvenParityConstraints(vec![vec![1, 0], vec![0, 3, 2], vec![0, 4]]),
            )
        );
    }

    #[test]
    fn cube6_matches_hardcoded() {
        assert_eq!(
            cube(6),
            (
                vec![
                    PartialOrbitDef {
                        name: Some("corners".to_string()),
                        piece_count: 8.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 3,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                    },
                    PartialOrbitDef {
                        name: Some("wings1".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("wings2".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("xcenters1".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("obliques1;2".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("obliques2;1".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("xcenters2".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                ],
                EvenParityConstraints(vec![
                    vec![0, 3],
                    vec![0, 1, 2, 4],
                    vec![0, 2, 1, 5],
                    vec![0, 6],
                ]),
            )
        );
    }

    #[test]
    fn cube7_matches_hardcoded() {
        assert_eq!(
            cube(7),
            (
                vec![
                    PartialOrbitDef {
                        name: Some("corners".to_string()),
                        piece_count: 8.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 3,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                    },
                    PartialOrbitDef {
                        name: Some("edges".to_string()),
                        piece_count: 12.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 2,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                    },
                    PartialOrbitDef {
                        name: Some("+centers1".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("+centers2".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("wings1".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("wings2".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("xcenters1".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("obliques1;2".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("obliques2;1".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("xcenters2".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                ],
                EvenParityConstraints(vec![
                    vec![1, 0],
                    vec![0, 4, 2],
                    vec![0, 5, 3],
                    vec![0, 6],
                    vec![0, 4, 5, 7],
                    vec![0, 5, 4, 8],
                    vec![0, 9],
                ]),
            )
        );
    }

    #[test]
    fn cube8_matches_hardcoded() {
        assert_eq!(
            cube(8),
            (
                vec![
                    PartialOrbitDef {
                        name: Some("corners".to_string()),
                        piece_count: 8.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 3,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                    },
                    PartialOrbitDef {
                        name: Some("wings1".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("wings2".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("wings3".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("xcenters1".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("obliques1;2".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("obliques1;3".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("obliques2;1".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("xcenters2".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("obliques2;3".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("obliques3;1".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("obliques3;2".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("xcenters3".to_string()),
                        piece_count: 24.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                ],
                EvenParityConstraints(vec![
                    vec![0, 4],
                    vec![0, 1, 2, 5],
                    vec![0, 1, 3, 6],
                    vec![0, 2, 1, 7],
                    vec![0, 8],
                    vec![0, 2, 3, 9],
                    vec![0, 3, 1, 10],
                    vec![0, 3, 2, 11],
                    vec![0, 12],
                ]),
            )
        );
    }
}

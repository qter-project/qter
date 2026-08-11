use std::sync::LazyLock;

use crate::puzzle::{
    EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef, PuzzleDef,
};

/// Generate the orbit definitions and even parity constraints of an order-`n`
/// minx. Feed this to `PuzzleDef::new` to create a puzzle.
#[allow(clippy::missing_panics_doc)]
#[must_use]
pub fn minx(n: usize) -> (Vec<PartialOrbitDef>, EvenParityConstraints) {
    if n <= 1 {
        return (vec![], EvenParityConstraints(vec![]));
    }

    let mut partial_orbit_defs = vec![];
    let mut even_parity_constraints = vec![];

    // all minxes n > 1 have 20 corners
    partial_orbit_defs.push(PartialOrbitDef {
        name: Some("corners".to_string()),
        piece_count: 20.try_into().unwrap(),
        orientation: OrientationStatus::CanOrient {
            count: 3,
            sum_constraint: OrientationSumConstraint::Zero,
        },
    });
    // every minx move induces only 5-cycles, so every piece type has even parity
    even_parity_constraints.push(vec!["corners".to_string()]);

    if n % 2 == 1 {
        // odd minxes have 30 edges
        partial_orbit_defs.push(PartialOrbitDef {
            name: Some("edges".to_string()),
            piece_count: 30.try_into().unwrap(),
            orientation: OrientationStatus::CanOrient {
                count: 2,
                sum_constraint: OrientationSumConstraint::Zero,
            },
        });
        even_parity_constraints.push(vec!["edges".to_string()]);

        // odd minxes have n/2 - 1 sets of 60 +centers
        for c2 in 1..n / 2 {
            partial_orbit_defs.push(PartialOrbitDef {
                name: Some(format!("+centers{c2}")),
                piece_count: 60.try_into().unwrap(),
                orientation: OrientationStatus::CannotOrient,
            });
            even_parity_constraints.push(vec![format!("+centers{c2}")]);
        }
    }

    // the minx has n/2 - 1 sets of 60 wings
    for w in 1..n / 2 {
        partial_orbit_defs.push(PartialOrbitDef {
            name: Some(format!("wings{w}")),
            piece_count: 60.try_into().unwrap(),
            orientation: OrientationStatus::CannotOrient,
        });
        even_parity_constraints.push(vec![format!("wings{w}")]);
    }

    // the minx has (n/2 - 1)^2 sets of 60 centers
    for c1 in 1..n / 2 {
        for c2 in 1..n / 2 {
            if c1 == c2 {
                // centers with equal indices are called xcenters, following the cube naming
                partial_orbit_defs.push(PartialOrbitDef {
                    name: Some(format!("xcenters{c1}")),
                    piece_count: 60.try_into().unwrap(),
                    orientation: OrientationStatus::CannotOrient,
                });
                even_parity_constraints.push(vec![format!("xcenters{c1}")]);
            } else {
                // the other centers are called obliques
                partial_orbit_defs.push(PartialOrbitDef {
                    name: Some(format!("obliques{c1};{c2}")),
                    piece_count: 60.try_into().unwrap(),
                    orientation: OrientationStatus::CannotOrient,
                });
                even_parity_constraints.push(vec![format!("obliques{c1};{c2}")]);
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
                        partial_orbit_defs
                            .iter()
                            .position(|orbit| orbit.name.as_deref() == Some(name.as_str()))
                            .expect("even parity constraint references an unknown orbit")
                    })
                    .collect()
            })
            .collect(),
    );
    (partial_orbit_defs, even_parity_constraints)
}

pub static MINX2: LazyLock<PuzzleDef<8>> = LazyLock::new(|| {
    let (orbit_defs, even_parity_constraints) = minx(2);
    PuzzleDef::new(orbit_defs, even_parity_constraints).unwrap()
});

pub static MINX3: LazyLock<PuzzleDef<16>> = LazyLock::new(|| {
    let (orbit_defs, even_parity_constraints) = minx(3);
    PuzzleDef::new(orbit_defs, even_parity_constraints).unwrap()
});

pub static MINX4: LazyLock<PuzzleDef<32>> = LazyLock::new(|| {
    let (orbit_defs, even_parity_constraints) = minx(4);
    PuzzleDef::new(orbit_defs, even_parity_constraints).unwrap()
});

pub static MINX5: LazyLock<PuzzleDef<32>> = LazyLock::new(|| {
    let (orbit_defs, even_parity_constraints) = minx(5);
    PuzzleDef::new(orbit_defs, even_parity_constraints).unwrap()
});

pub static MINX6: LazyLock<PuzzleDef<32>> = LazyLock::new(|| {
    let (orbit_defs, even_parity_constraints) = minx(6);
    PuzzleDef::new(orbit_defs, even_parity_constraints).unwrap()
});

#[cfg(test)]
mod tests {
    use crate::puzzle::{
        EvenParityConstraints, OrientationStatus, OrientationSumConstraint, PartialOrbitDef,
        minxN::minx,
    };

    #[test]
    fn trivial_minx() {
        let (orbits, EvenParityConstraints(constraints)) = minx(1);
        assert!(orbits.is_empty());
        assert!(constraints.is_empty());
    }

    #[test]
    fn minx2_matches_hardcoded() {
        assert_eq!(
            minx(2),
            (
                vec![PartialOrbitDef {
                    name: Some("corners".to_string()),
                    piece_count: 20.try_into().unwrap(),
                    orientation: OrientationStatus::CanOrient {
                        count: 3,
                        sum_constraint: OrientationSumConstraint::Zero,
                    },
                }],
                EvenParityConstraints(vec![vec![0]]),
            )
        );
    }

    #[test]
    fn minx3_matches_hardcoded() {
        assert_eq!(
            minx(3),
            (
                vec![
                    PartialOrbitDef {
                        name: Some("corners".to_string()),
                        piece_count: 20.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 3,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                    },
                    PartialOrbitDef {
                        name: Some("edges".to_string()),
                        piece_count: 30.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 2,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                    },
                ],
                EvenParityConstraints(vec![vec![0], vec![1]]),
            )
        );
    }

    #[test]
    fn minx4_matches_hardcoded() {
        assert_eq!(
            minx(4),
            (
                vec![
                    PartialOrbitDef {
                        name: Some("corners".to_string()),
                        piece_count: 20.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 3,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                    },
                    PartialOrbitDef {
                        name: Some("wings1".to_string()),
                        piece_count: 60.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("xcenters1".to_string()),
                        piece_count: 60.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                ],
                EvenParityConstraints(vec![vec![0], vec![1], vec![2]]),
            )
        );
    }

    #[test]
    fn minx5_matches_hardcoded() {
        assert_eq!(
            minx(5),
            (
                vec![
                    PartialOrbitDef {
                        name: Some("corners".to_string()),
                        piece_count: 20.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 3,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                    },
                    PartialOrbitDef {
                        name: Some("edges".to_string()),
                        piece_count: 30.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 2,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                    },
                    PartialOrbitDef {
                        name: Some("+centers1".to_string()),
                        piece_count: 60.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("wings1".to_string()),
                        piece_count: 60.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("xcenters1".to_string()),
                        piece_count: 60.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                ],
                EvenParityConstraints(vec![vec![0], vec![1], vec![2], vec![3], vec![4]]),
            )
        );
    }

    #[test]
    fn minx6_matches_hardcoded() {
        assert_eq!(
            minx(6),
            (
                vec![
                    PartialOrbitDef {
                        name: Some("corners".to_string()),
                        piece_count: 20.try_into().unwrap(),
                        orientation: OrientationStatus::CanOrient {
                            count: 3,
                            sum_constraint: OrientationSumConstraint::Zero,
                        },
                    },
                    PartialOrbitDef {
                        name: Some("wings1".to_string()),
                        piece_count: 60.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("wings2".to_string()),
                        piece_count: 60.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("xcenters1".to_string()),
                        piece_count: 60.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("obliques1;2".to_string()),
                        piece_count: 60.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("obliques2;1".to_string()),
                        piece_count: 60.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                    PartialOrbitDef {
                        name: Some("xcenters2".to_string()),
                        piece_count: 60.try_into().unwrap(),
                        orientation: OrientationStatus::CannotOrient,
                    },
                ],
                EvenParityConstraints(vec![
                    vec![0],
                    vec![1],
                    vec![2],
                    vec![3],
                    vec![4],
                    vec![5],
                    vec![6],
                ]),
            )
        );
    }
}

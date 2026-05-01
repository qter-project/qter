use std::borrow::Cow;

use bitgauss::BitMatrix;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParityAssignment {
    Unassigned,
    Assigned(bool),
}

fn apply_constraints(
    constraints: &BitMatrix,
    mut curr_assignment: Cow<[ParityAssignment]>,
    i: usize,
    v: bool,
) -> Option<Vec<ParityAssignment>> {
    assert!(matches!(curr_assignment[i], ParityAssignment::Unassigned));
    for constraint in (0..constraints.rows()).filter_map(|j| {
        let constraint = constraints.row(j);
        if constraint.bit(i) {
            Some(constraint)
        } else {
            None
        }
    }) {
        match constraint
            .iter()
            .enumerate()
            // The first `i` entries are assigned, as per the backtracking algorithm
            .skip(i)
            .filter(|&(j, c)| c && matches!(curr_assignment[j], ParityAssignment::Unassigned))
            .count()
        {
            // We asserted earlier that curr_assignment[i] is unassigned. We also know constraint[i]
            // is true from the filter map. Therefore there will always be at least one count of
            // remaining 1s
            0 => unreachable!(),
            1 => {
                assert!(matches!(curr_assignment[i], ParityAssignment::Unassigned));
                let expected_v = constraint
                    .iter()
                    .take(curr_assignment.len())
                    .zip(curr_assignment.iter())
                    .enumerate()
                    .fold(false, |parity, (j, (c, &a))| {
                        if c && j != i {
                            let ParityAssignment::Assigned(other_v) = a else {
                                // In the 1 branch the only unassigned is `curr_assignment[i]`
                                unreachable!();
                            };
                            parity ^ other_v
                        } else {
                            parity
                        }
                    });
                if v != expected_v {
                    return None;
                }
            }
            2 => {
                assert!(matches!(curr_assignment[i], ParityAssignment::Unassigned));
                let mut parity = false;
                let mut other: Option<&mut ParityAssignment> = None;
                for (j, a) in constraint
                    .iter()
                    .take(curr_assignment.len())
                    .zip(curr_assignment.to_mut())
                    .enumerate()
                    .filter_map(|(j, (c, a))| if c { Some((j, a)) } else { None })
                {
                    match a {
                        ParityAssignment::Unassigned if j == i => (),
                        ParityAssignment::Unassigned => match other {
                            Some(_) => {
                                // We only have one other 1 that is not `i`. `other` will only be
                                // `Some` once; therefore this is unreachable.
                                unreachable!();
                            }
                            None => {
                                other = Some(a);
                            }
                        },
                        ParityAssignment::Assigned(v) => {
                            parity ^= *v;
                        }
                    }
                }
                // We guarantee that there are two 1s, one of which is at `i` and the other of
                // which must have been found at `other`.
                let other = other.unwrap();
                let expected_other = ParityAssignment::Assigned(parity ^ v);
                match other {
                    ParityAssignment::Unassigned => {
                        *other = expected_other;
                    }
                    ParityAssignment::Assigned(_) if *other == expected_other => (),
                    ParityAssignment::Assigned(_) => {
                        return None;
                    }
                }
            }
            3.. => (),
        }
    }
    let mut curr_assignment = curr_assignment.into_owned();
    curr_assignment[i] = ParityAssignment::Assigned(v);
    Some(curr_assignment)
}

/// Enumerate all possible parity assignments for a connected component of
/// orbits.
///
/// # Panics
///
/// The constraints matrix *must* be in row-reduced echelon form, with zero rows
/// removed. This function is otherwise allowed to panic or produce nonsensical
/// results.
///
/// This function also panics if the constraints matrix has one or less columns;
/// this singular orbit can have all of the possible orders evaluated more
/// efficiently.
pub fn backtrack_ac3(constraints: &BitMatrix) -> impl Iterator<Item = impl Iterator<Item = bool>> {
    assert!(constraints.cols() > 1);

    gen {
        let mut stack = vec![(0, vec![ParityAssignment::Unassigned; constraints.cols()])];
        while let Some((i, curr_assignment)) = stack.pop() {
            match curr_assignment.get(i) {
                None => {
                    yield curr_assignment.into_iter().map(|a| match a {
                        // We *must* have assigned everything for `i` to be out of bounds.
                        // This implies we have walked the entire list and backtracked all possible
                        // assignments at every position.
                        ParityAssignment::Unassigned => unreachable!(),
                        ParityAssignment::Assigned(v) => v,
                    });
                }
                Some(ParityAssignment::Assigned(_)) => {
                    stack.push((i + 1, curr_assignment));
                }
                Some(ParityAssignment::Unassigned) => {
                    if let Some(applied_false) =
                        apply_constraints(constraints, Cow::Borrowed(&curr_assignment), i, false)
                    {
                        stack.push((i + 1, applied_false));
                    }

                    if let Some(applied_true) =
                        apply_constraints(constraints, Cow::Owned(curr_assignment), i, true)
                    {
                        stack.push((i + 1, applied_true));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bitgauss::BitMatrix;

    use crate::ac3::backtrack_ac3;

    #[test_log::test]
    fn simple() {
        let constraints = BitMatrix::from_bool_vec(&[vec![true, true]]);
        assert_eq!(
            backtrack_ac3(&constraints)
                .map(Iterator::collect::<Vec<_>>)
                .collect::<Vec<_>>(),
            vec![vec![true, true], vec![false, false]]
        );
    }

    #[test_log::test]
    fn complex() {
        let constraints = BitMatrix::from_bool_vec(&[
            vec![true, false, false, false, true],
            vec![false, false, true, false, true],
            vec![false, false, false, true, true],
        ]);
        assert_eq!(
            backtrack_ac3(&constraints)
                .map(Iterator::collect::<Vec<_>>)
                .collect::<Vec<_>>(),
            vec![
                vec![true, true, true, true, true],
                vec![true, false, true, true, true],
                vec![false, true, false, false, false],
                vec![false, false, false, false, false]
            ],
        );

        let constraints = BitMatrix::from_bool_vec(&[
            vec![true, false, false, false, true],
            vec![false, true, false, false, true],
            vec![false, false, true, true, true],
        ]);
        assert_eq!(
            backtrack_ac3(&constraints)
                .map(Iterator::collect::<Vec<_>>)
                .collect::<Vec<_>>(),
            vec![
                vec![true, true, true, false, true],
                vec![true, true, false, true, true],
                vec![false, false, true, true, false],
                vec![false, false, false, false, false]
            ],
        );

        let constraints = BitMatrix::from_bool_vec(&[
            vec![true, false, false, false, false, true],
            vec![false, true, false, false, false, true],
            vec![false, false, true, true, true, true],
        ]);
        assert_eq!(
            backtrack_ac3(&constraints)
                .map(Iterator::collect::<Vec<_>>)
                .collect::<Vec<_>>(),
            vec![
                vec![true, true, true, true, true, true],
                vec![true, true, true, false, false, true],
                vec![true, true, false, true, false, true],
                vec![true, true, false, false, true, true],
                vec![false, false, true, true, false, false],
                vec![false, false, true, false, true, false],
                vec![false, false, false, true, true, false],
                vec![false, false, false, false, false, false],
            ],
        );

        let constraints = BitMatrix::from_bool_vec(&[
            vec![true, false, false, false, false, true],
            vec![false, true, false, false, true, true],
            vec![false, false, true, true, true, false],
        ]);
        assert_eq!(
            backtrack_ac3(&constraints)
                .map(Iterator::collect::<Vec<_>>)
                .collect::<Vec<_>>(),
            vec![
                vec![true, true, true, true, false, true],
                vec![true, true, false, false, false, true],
                vec![true, false, true, false, true, true],
                vec![true, false, false, true, true, true],
                vec![false, true, true, false, true, false],
                vec![false, true, false, true, true, false],
                vec![false, false, true, true, false, false],
                vec![false, false, false, false, false, false],
            ],
        );
    }

    #[test_log::test]
    fn cube7() {
        let constraints = BitMatrix::from_bool_vec(&[
            vec![
                true, false, false, false, false, false, false, false, false, true,
            ],
            vec![
                false, true, false, false, false, false, false, false, false, true,
            ],
            vec![
                false, false, true, false, false, true, false, false, true, false,
            ],
            vec![
                false, false, false, true, false, true, false, false, false, true,
            ],
            vec![
                false, false, false, false, true, true, false, false, true, true,
            ],
            vec![
                false, false, false, false, false, false, true, false, false, true,
            ],
            vec![
                false, false, false, false, false, false, false, true, true, false,
            ],
        ]);

        assert_eq!(
            backtrack_ac3(&constraints)
                .map(Iterator::collect::<Vec<_>>)
                .collect::<Vec<_>>(),
            vec![
                vec![
                    true, true, true, true, true, false, true, false, false, true
                ],
                vec![true, true, true, true, false, false, true, true, true, true],
                vec![true, true, true, false, true, true, true, true, true, true],
                vec![
                    true, true, true, false, false, true, true, false, false, true
                ],
                vec![
                    true, true, false, true, true, false, true, false, false, true
                ],
                vec![
                    true, true, false, true, false, false, true, true, true, true
                ],
                vec![true, true, false, false, true, true, true, true, true, true],
                vec![
                    true, true, false, false, false, true, true, false, false, true
                ],
                vec![
                    false, false, true, true, true, true, false, false, false, false
                ],
                vec![
                    false, false, true, true, false, true, false, true, true, false
                ],
                vec![
                    false, false, true, false, true, false, false, true, true, false
                ],
                vec![
                    false, false, true, false, false, false, false, false, false, false
                ],
                vec![
                    false, false, false, true, true, true, false, false, false, false
                ],
                vec![
                    false, false, false, true, false, true, false, true, true, false
                ],
                vec![
                    false, false, false, false, true, false, false, true, true, false
                ],
                vec![
                    false, false, false, false, false, false, false, false, false, false
                ]
            ]
        );
    }
}

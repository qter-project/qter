use std::borrow::Cow;

use bitgauss::BitMatrix;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParityAssignment {
    Unassigned,
    Assigned(bool),
}

fn apply_constriants(
    constraints: &BitMatrix,
    mut curr_assignment: Cow<[ParityAssignment]>,
    i: usize,
    v: bool,
) -> Option<Vec<ParityAssignment>> {
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
            .skip(i)
            .filter(|&(j, c)| c && matches!(curr_assignment[j], ParityAssignment::Unassigned))
            .count()
        {
            0 => panic!(),
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
                                panic!();
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
                for (j, (c, a)) in constraint
                    .iter()
                    .take(curr_assignment.len())
                    .zip(curr_assignment.to_mut())
                    .enumerate()
                {
                    if !c {
                        continue;
                    }
                    match a {
                        ParityAssignment::Unassigned if j == i => (),
                        ParityAssignment::Unassigned => match other {
                            Some(_) => {
                                panic!();
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
#[must_use]
pub fn backtrack_ac3(constraints: &BitMatrix) -> Vec<Vec<bool>> {
    assert!(constraints.cols() > 1);
    let mut ret = vec![];

    let mut stack = vec![(0, vec![ParityAssignment::Unassigned; constraints.cols()])];
    while let Some((i, curr_assignment)) = stack.pop() {
        match curr_assignment.get(i) {
            None => {
                ret.push(
                    curr_assignment
                        .into_iter()
                        .map(|a| match a {
                            ParityAssignment::Unassigned => panic!(),
                            ParityAssignment::Assigned(v) => v,
                        })
                        .collect::<Vec<_>>(),
                );
            }
            Some(ParityAssignment::Assigned(_)) => {
                stack.push((i + 1, curr_assignment));
            }
            Some(ParityAssignment::Unassigned) => {
                if let Some(applied_false) =
                    apply_constriants(constraints, Cow::Borrowed(&curr_assignment), i, false)
                {
                    stack.push((i + 1, applied_false));
                }

                if let Some(applied_true) =
                    apply_constriants(constraints, Cow::Owned(curr_assignment), i, true)
                {
                    stack.push((i + 1, applied_true));
                }
            }
        }
    }
    ret
}

#[cfg(test)]
mod tests {
    use bitgauss::BitMatrix;

    use crate::ac3::backtrack_ac3;

    #[test_log::test]
    fn simple() {
        let constraints = BitMatrix::from_bool_vec(&[vec![true, true]]);
        assert_eq!(
            backtrack_ac3(&constraints),
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
            backtrack_ac3(&constraints),
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
            backtrack_ac3(&constraints),
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
            backtrack_ac3(&constraints),
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
            backtrack_ac3(&constraints),
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
            backtrack_ac3(&constraints),
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

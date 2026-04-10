use std::borrow::Cow;

use bitgauss::BitMatrix;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParityAssignment {
    Unassigned,
    Assigned(bool),
}

fn apply_constriants(
    constraints: &BitMatrix,
    mut curr_assignment: Cow<'_, [ParityAssignment]>,
    i: usize,
    v: bool,
) -> Option<Vec<ParityAssignment>> {
    for j in 0..constraints.rows() {
        let constraint = constraints.row(j);
        if !constraint.bit(i) {
            continue;
        }
        match constraint
            .iter()
            .enumerate()
            .skip(i)
            .filter(|&(k, c)| c && matches!(curr_assignment[k], ParityAssignment::Unassigned))
            .count()
        {
            0 => panic!(),
            1 => {
                assert!(matches!(curr_assignment[j], ParityAssignment::Unassigned));
                let expected_v = constraint
                    .iter()
                    .take(curr_assignment.len())
                    .zip(curr_assignment.iter())
                    .enumerate()
                    .fold(false, |parity, (k, (c, &a))| {
                        if c && k != i {
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
                assert!(matches!(curr_assignment[j], ParityAssignment::Unassigned));
                let mut parity = false;
                let mut other: Option<&mut ParityAssignment> = None;
                for (k, (c, a)) in constraint
                    .iter()
                    .take(curr_assignment.len())
                    .zip(curr_assignment.to_mut())
                    .enumerate()
                {
                    if !c {
                        continue;
                    }
                    match a {
                        ParityAssignment::Unassigned if k != i => match other {
                            Some(_) => {
                                panic!();
                            }
                            None => {
                                other = Some(a);
                            }
                        },
                        ParityAssignment::Unassigned => (),
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
                    other @ ParityAssignment::Assigned(_) if *other != expected_other => {
                        return None;
                    }
                    ParityAssignment::Assigned(_) => (),
                }
            }
            3.. => (),
        }
    }
    let mut curr_assignment = curr_assignment.into_owned();
    curr_assignment[i] = ParityAssignment::Assigned(v);
    Some(curr_assignment)
}

pub fn backtrack_ac3(constraints: &BitMatrix) -> Vec<Vec<ParityAssignment>> {
    assert_ne!(constraints.cols(), 1);
    let mut ret = vec![];

    let mut stack = vec![(0, vec![ParityAssignment::Unassigned; constraints.cols()])];
    while let Some((i, curr_assignment)) = stack.pop() {
        match curr_assignment[i] {
            ParityAssignment::Assigned(_) => {
                if curr_assignment
                    .iter()
                    .all(|a| matches!(a, ParityAssignment::Assigned(_)))
                {
                    ret.push(curr_assignment);
                }
            }
            ParityAssignment::Unassigned => {
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
    use fxhash::FxHashMap;
    use union_find::{QuickUnionUf, UnionBySize, UnionFind};

    use crate::{
        ac3::backtrack_ac3,
        puzzle::{EvenParityConstraints, OrbitDef, OrientationStatus, ParityConstraint, PuzzleDef},
    };

    #[test]
    fn foo() {
        let a = PuzzleDef::new(
            vec![
                OrbitDef {
                    piece_count: 1.try_into().unwrap(),
                    orientation: OrientationStatus::CannotOrient,
                    parity_constraint: ParityConstraint::None,
                };
                2
            ],
            EvenParityConstraints(vec![vec![0, 1], vec![0]])
        )
        .unwrap();

        let mut even_parity_constraints = a.even_parity_constraints().clone();

        // let mut rows = even_parity_constraints.rows();
        // let mut constraints =
        //     BitMatrix::build(rows, cols, |i, j|
        // even_parity_constraints[i].contains(&j)); let pivot_cols =
        // constraints.gauss(true); let rank = pivot_cols.len();
        // if rank != rows {
        //     rows = rank;
        //     constraints = BitMatrix::build(rows, cols, |i, j| constraints[(i, j)]);
        // }

        let cols = even_parity_constraints.cols();
        let rows = even_parity_constraints.rows();
        let pivot_cols = even_parity_constraints.gauss(true);
        println!("{}", even_parity_constraints);
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
            connected_components.entry(root).or_default().push(orbit_index);
        }
        for connected_component in connected_components.into_values() {
            let mut connected_component_parity_constraints = BitMatrix::build(
                even_parity_constraints.rows(),
                connected_component.len(),
                |i, j| even_parity_constraints[(i, j + connected_component[0])],
            );
            let pivot_cols = connected_component_parity_constraints.gauss(true);
            let rank = pivot_cols.len();
            if even_parity_constraints.rows() != rank {
                connected_component_parity_constraints =
                    BitMatrix::build(rank, connected_component.len(), |i, j| {
                        connected_component_parity_constraints[(i, j)]
                    });
            }
            println!("{:?}", connected_component);
            println!(
                "{}: {}",
                connected_component_parity_constraints.rows(),
                connected_component_parity_constraints
            );
            if connected_component.len() == 1 {
                continue;
            }
            println!(
                "{:#?}",
                backtrack_ac3(&connected_component_parity_constraints)
            );
        }
        // let mut sizes = sizes.into_values().collect::<Vec<_>>();
        // sizes.sort_by_key(|size| Reverse(size.len()));
        // println!("{:?}", sizes);
        // println!("{:?}", uf);
        // println!("{}", constraints);
        // println!("{}", b);
        // backtrack_ac3(b, &[0, 1, 2, 3]);
        panic!();
    }
}

use std::{cmp::Ordering, mem};

use itertools::Itertools;

use crate::num::{Matrix, Vector};

#[derive(Clone, Debug)]
pub struct EdgeCloud {
    edges: Vec<(Vector<3>, Vector<3>)>,
}

impl EdgeCloud {
    pub fn new(mut edges: Vec<(Vector<3>, Vector<3>)>) -> EdgeCloud {
        sort_edge_cloud(&mut edges);

        EdgeCloud { edges }
    }

    pub fn try_symmetry(self, matrix: &Matrix<3, 3>) -> bool {
        if self.edges.is_empty() {
            return false;
        }

        let mut edges = self.edges.into_iter().dedup_with_count().collect_vec();
        let mut current_edge = edges[0].clone();

        loop {
            let (eq_count, (start, end)) = &current_edge;
            let mut new_start = matrix * start;
            let mut new_end = matrix * end;
            maybe_flip_edge(&mut new_start, &mut new_end);
            match edges.binary_search_by(|(_, v)| edge_compare(&v.0, &v.1, &new_start, &new_end)) {
                Ok(idx) => {
                    if edges[idx].0 != *eq_count {
                        return false;
                    }

                    if edges.len() == 1 {
                        return true;
                    }

                    current_edge = edges.remove(idx);

                    if idx == 0 {
                        current_edge = edges[0].clone();
                    }
                }
                Err(_) => return false,
            }
        }
    }

    pub fn epsilon_eq(&self, other: &EdgeCloud) -> bool {
        edge_cloud_eq(&self.edges, &other.edges)
    }
}

fn maybe_flip_edge(a: &mut Vector<3>, b: &mut Vector<3>) {
    let ordering = a
        .inner()
        .iter()
        .zip(b.inner().iter())
        .map(|(x1, x2)| x1.cmp(x2))
        .find_or_last(|v| !matches!(v, Ordering::Equal))
        .unwrap();

    if matches!(ordering, Ordering::Greater) {
        mem::swap(a, b);
    }
}

fn edge_compare(a1: &Vector<3>, a2: &Vector<3>, b1: &Vector<3>, b2: &Vector<3>) -> Ordering {
    a1.inner()
        .iter()
        .zip(b1.inner().iter())
        .chain(a2.inner().iter().zip(b2.inner().iter()))
        .map(|(x1, x2)| x1.cmp(x2))
        .find_or_last(|v| !matches!(v, Ordering::Equal))
        .unwrap()
}

fn sort_edge_cloud(cloud: &mut [(Vector<3>, Vector<3>)]) {
    for (a, b) in &mut *cloud {
        maybe_flip_edge(a, b);
    }

    cloud.sort_unstable_by(|(a1, a2), (b1, b2)| edge_compare(a1, a2, b1, b2));
}

fn edge_cloud_eq(cloud1: &[(Vector<3>, Vector<3>)], cloud2: &[(Vector<3>, Vector<3>)]) -> bool {
    cloud1
        .iter()
        .zip(cloud2)
        .all(|((a1, b1), (a2, b2))| a1 == a2 && b1 == b2)
}

#[cfg(test)]
mod tests {
    use crate::{
        Face, exact_trig::rotation_degree, num::{Vector, rotation_about}, shapes::TETRAHEDRON
    };

    use super::EdgeCloud;

    #[test]
    fn equality() {
        let edge_cloud_one = EdgeCloud::new(vec![
            (Vector::new([[1, 2, 3]]), Vector::new([[4, 5, 6]])),
            (Vector::new([[3, 2, 1]]), Vector::new([[6, 5, 4]])),
            (Vector::new([[4, 8, 3]]), Vector::new([[2, 5, 6]])),
        ]);

        let edge_cloud_two = EdgeCloud::new(vec![
            (Vector::new([[4, 5, 6]]), Vector::new([[1, 2, 3]])),
            (Vector::new([[4, 8, 3]]), Vector::new([[2, 5, 6]])),
            (Vector::new([[6, 5, 4]]), Vector::new([[3, 2, 1]])),
        ]);

        // println!("{edge_cloud_one:?}");
        // println!("{edge_cloud_two:?}");

        assert!(edge_cloud_one.epsilon_eq(&edge_cloud_two));
    }

    #[test]
    fn try_symmetry() {
        let tetrahedron = EdgeCloud::new(TETRAHEDRON.0.iter().flat_map(Face::edges).collect());

        assert_eq!(
            tetrahedron
                .clone()
                .try_symmetry(&rotation_about(Vector::new([[0, 1, 0]]), rotation_degree(1, 3).clone())),
            true,
        );

        assert_eq!(
            tetrahedron.try_symmetry(&rotation_about(Vector::new([[0, 1, 0]]), rotation_degree(1, 5).clone())),
            false
        );
    }
}

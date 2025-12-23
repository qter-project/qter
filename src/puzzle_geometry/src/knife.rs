use std::{collections::VecDeque, iter, mem};

use internment::ArcIntern;
use itertools::Itertools;

use crate::{
    Face, FaceSubspaceInfo, Point, PuzzleGeometryError,
    num::{Matrix, Num, Vector},
};

/// Defines a generic cut surface; may or may not be planar or have only two regions.
///
/// Regions are represented by an `Option<ArcIntern<str>>`. A point "outside the region" can be represented by None. Having multiple regions in the same `CutSurface` is allowed.
pub trait CutSurface: core::fmt::Debug {
    /// Get the region that a point is in
    fn region(&self, point: Point) -> Option<ArcIntern<str>>;

    /// Tell whether a point is on the boundary of a cut plane
    fn on_boundary(&self, point: Point) -> bool;

    /// Get all points on boundaries intersected by the line segment spanned by the two points. The returned points must be in order from closest to A to closest to B, and `on_boundary` called on any of them must return `true`.
    fn boundaries_between(&self, point_a: Point, point_b: Point) -> Vec<Point>;

    /// Return a series of points that when connected as line segments including A and B, connects A and B through the boundary. A and B are guaranteed to already be on the boundary. `on_boundary` when called on any of the points must return `true`.
    fn join(&self, a: Point, b: Point, subspace_info: FaceSubspaceInfo) -> Vec<Point>;
}

#[derive(Clone, Debug)]
pub struct PlaneCut {
    pub spot: Vector<3>,
    pub normal: Vector<3>,
    pub name: ArcIntern<str>,
}

impl CutSurface for PlaneCut {
    fn region(&self, point: Point) -> Option<ArcIntern<str>> {
        match self
            .normal
            .clone()
            .dot(point.0 - self.spot.clone())
            .cmp_zero()
        {
            std::cmp::Ordering::Less => None,
            std::cmp::Ordering::Equal => {
                panic!("Argument to region should not be exactly on the boundary")
            }
            std::cmp::Ordering::Greater => Some(ArcIntern::clone(&self.name)),
        }
    }

    fn on_boundary(&self, point: Point) -> bool {
        self.normal
            .clone()
            .dot(point.0 - self.spot.clone())
            .is_zero()
    }

    fn boundaries_between(&self, a: Point, b: Point) -> Vec<Point> {
        let a_dot = self.normal.clone().dot(a.0.clone() - self.spot.clone());
        let b_dot = self.normal.clone().dot(b.0.clone() - self.spot.clone());

        if a_dot.cmp_zero() == b_dot.cmp_zero() {
            return vec![];
        }

        let frac = a_dot.clone().abs() / (a_dot.abs() + b_dot.abs());

        let mut point = Point(a.0);
        point.0 = b.0 * &frac + (point.0 * &(Num::from(1) - frac.clone()));
        assert!(
            self.on_boundary(point.clone()),
            "{:?}, {:?}, {frac:?}",
            point.clone(),
            self.normal.clone().dot(point.0 - self.spot.clone())
        );

        vec![point]
    }

    fn join(&self, _: Point, _: Point, _: FaceSubspaceInfo) -> Vec<Point> {
        vec![]
    }
}

#[derive(Debug, Clone)]
struct Cycle<T>(VecDeque<T>);

impl<T> Cycle<T> {
    fn go_forward(&mut self) {
        let Some(value) = self.0.pop_front() else {
            return;
        };
        self.0.push_back(value);
    }

    fn go_backward(&mut self) {
        let Some(value) = self.0.pop_back() else {
            return;
        };
        self.0.push_front(value);
    }

    fn spot(&self) -> Option<&T> {
        self.0.front()
    }

    fn spot_mut(&mut self) -> Option<&mut T> {
        self.0.front_mut()
    }

    fn remove_spot(&mut self) -> Option<T> {
        self.0.pop_front()
    }

    fn prev(&self) -> Option<&T> {
        self.0.back()
    }

    fn prev_mut(&mut self) -> Option<&mut T> {
        self.0.back_mut()
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn insert_before(&mut self, value: T) {
        self.0.push_back(value);
    }
}

pub(crate) fn do_cut<S: CutSurface + ?Sized>(
    surface: &S,
    face: &Face,
    subspace_info: &FaceSubspaceInfo,
) -> Result<Vec<(Face, Option<ArcIntern<str>>)>, PuzzleGeometryError> {
    assert!(!face.points.is_empty());

    // Convert the list of 3d points into a list of 2d edges, split on boundaries, with the edge's region included.
    let mut edges = Cycle(
        face.points
            .iter()
            .circular_tuple_windows()
            .take(face.points.len())
            .flat_map(|(a, b)| {
                iter::once(a.clone()).chain(
                    surface
                        .boundaries_between(a.clone(), b.clone())
                        .into_iter()
                        .filter(|v| v.0 != a.0 && v.0 != b.0),
                )
            })
            .collect_vec()
            .iter()
            .circular_tuple_windows()
            .map(|(a, b)| {
                let middle = Point(a.0.clone() / &Num::from(2) + (b.0.clone() / &Num::from(2)));

                (
                    (
                        &subspace_info.make_2d * &(a.0.clone() - subspace_info.offset.clone()),
                        &subspace_info.make_2d * &(b.0.clone() - subspace_info.offset.clone()),
                    ),
                    if surface.on_boundary(middle.clone()) {
                        None
                    } else {
                        Some(surface.region(middle))
                    },
                )
            })
            .collect::<VecDeque<_>>(),
    );

    let mut faces = Vec::new();

    let ninety_deg = Matrix::new([[0, 1], [-1, 0]]);

    while edges.len() >= 3 {
        // Merge collinear edges
        let mut i = 0;
        while i < edges.len() && edges.len() > 1 {
            let a = edges.prev().unwrap();
            let b = edges.spot().unwrap();
            if a.1 == b.1
                && (&ninety_deg * &(a.0.1.clone() - a.0.0.clone()))
                    .dot(b.0.1.clone() - a.0.0.clone())
                    .is_zero()
            {
                edges.prev_mut().unwrap().0.1 = b.0.1.clone();
                edges.remove_spot();
                continue;
            }

            i += 1;
            edges.go_forward();
        }

        if edges.len() < 3 {
            break;
        }

        recolor_border_edges(&mut edges);

        faces.push(take_face_out(&mut edges, surface, face, subspace_info)?);
    }

    faces.retain(|v| v.0.is_valid().is_ok());

    Ok(faces)
}

/// Recolors border edges that are sandwiched between edges of the same color
///
/// This is necessary because with the color pattern [Some(A), None, Some(A), None], `take_face_out` will separate that into two faces even though it shouldn't do that.
fn recolor_border_edges(
    edges: &mut Cycle<((Vector<2>, Vector<2>), Option<Option<ArcIntern<str>>>)>,
) {
    let mut i = 0;

    while i < edges.len() {
        if edges.spot().unwrap().1.is_none() {
            let after = edges.0.iter().find_map(|v| v.1.clone());
            let before = edges.0.iter().rev().find_map(|v| v.1.clone());

            if after == before {
                edges.spot_mut().unwrap().1 = after;
            }
        }

        edges.go_forward();
        i += 1;
    }
}

fn take_face_out<S: CutSurface + ?Sized>(
    edges: &mut Cycle<((Vector<2>, Vector<2>), Option<Option<ArcIntern<str>>>)>,
    surface: &S,
    face: &Face,
    subspace_info: &FaceSubspaceInfo,
) -> Result<(Face, Option<ArcIntern<str>>), PuzzleGeometryError> {
    // Find a collection of edges that can be merged
    // This algorithm tries to find a collection of vertices that "peeks out" and comes back to the same region.
    // If a collection of vertices didn't come back to the same region, then it would be impossible to merge them because we couldn't prove that they're the only vertices in the group.
    // As long as the regions don't have a cyclic structure, this should always happen somewhere.

    // Traverse to a region boundary
    let mut i = 0;

    while i < edges.len() {
        let prev_region = edges.prev().unwrap().1.as_ref();
        let region = edges.spot().unwrap().1.as_ref();

        if match region {
            Some(v) => Some(v) != prev_region && prev_region.is_some(),
            None => prev_region.is_some(),
        } {
            break;
        }

        edges.go_backward();
        i += 1;
    }

    if i == edges.len() {
        // All of the edges are in the same region

        let region = edges.spot().unwrap().1.clone().ok_or_else(|| {
            PuzzleGeometryError::CyclicalCutSurface(format!("{surface:?}"), face.to_owned())
        })?;

        return Ok((
            Face {
                points: mem::replace(edges, Cycle(VecDeque::new()))
                    .0
                    .into_iter()
                    .map(|v| Point(&subspace_info.make_3d * &v.0.0 + subspace_info.offset.clone()))
                    .collect_vec(),
                color: ArcIntern::clone(&face.color),
            },
            region,
        ));
    }

    // Scan the edges for regions that work

    #[allow(clippy::items_after_statements)]
    #[derive(Debug)]
    enum StateMachine {
        Begin {
            previous_region: Option<ArcIntern<str>>,
        },
        InRegion {
            previous_region: Option<ArcIntern<str>>,
            region: Option<ArcIntern<str>>,
        },
        SeeingWhatsNext {
            previous_region: Option<ArcIntern<str>>,
            region: Option<ArcIntern<str>>,
        },
    }

    i = 0;
    let mut state = StateMachine::Begin {
        previous_region: edges.prev().unwrap().1.as_ref().unwrap().to_owned(),
    };
    let mut found_region = None;
    // *2 because it may need to go over to check if a region is valid
    while i < edges.len() * 2 {
        state = match state {
            StateMachine::Begin { previous_region } => match &edges.spot().unwrap().1 {
                Some(region) => StateMachine::InRegion {
                    previous_region,
                    region: region.to_owned(),
                },
                None => StateMachine::Begin { previous_region },
            },
            StateMachine::InRegion {
                previous_region,
                region,
            } => match &edges.spot().unwrap().1 {
                Some(next_region) => {
                    if *next_region == region {
                        StateMachine::InRegion {
                            previous_region,
                            region,
                        }
                    } else if *next_region == previous_region {
                        found_region = Some(region);
                        break;
                    } else {
                        StateMachine::Begin {
                            previous_region: region,
                        }
                    }
                }
                None => StateMachine::SeeingWhatsNext {
                    previous_region,
                    region,
                },
            },
            StateMachine::SeeingWhatsNext {
                previous_region,
                region,
            } => match &edges.spot().unwrap().1 {
                Some(new_region) => {
                    if *new_region == previous_region || *new_region == region {
                        found_region = Some(region);
                        break;
                    }

                    StateMachine::Begin {
                        previous_region: region,
                    }
                }
                None => StateMachine::SeeingWhatsNext {
                    previous_region,
                    region,
                },
            },
        };
        i += 1;
        edges.go_forward();
    }

    // Split off the edges that are in the region

    let Some(found_region) = found_region else {
        return Err(PuzzleGeometryError::CyclicalCutSurface(
            format!("{surface:?}"),
            face.to_owned(),
        ));
    };

    edges.go_backward();

    while edges.spot().unwrap().1.is_none() {
        edges.go_backward();
    }

    let mut face_edges = Vec::new();

    while edges.spot().unwrap().1.as_ref() == Some(&found_region) {
        face_edges.push(edges.remove_spot().unwrap());
        edges.go_backward();
    }
    edges.go_forward();
    face_edges.reverse();

    // Turn the edges in the region into a face

    let mut points = face_edges
        .iter()
        .map(|v| v.0.0.clone())
        .chain(iter::once(face_edges.last().unwrap().0.1.clone()))
        .map(|v| Point(&subspace_info.make_3d * &v + subspace_info.offset.clone()))
        .collect_vec();

    let first = points.first().unwrap().clone();
    let last = points.last().unwrap().clone();

    let mut joiner =
        VecDeque::from(surface.join(last.clone(), first.clone(), subspace_info.clone()));

    points.extend(joiner.iter().cloned());

    joiner.push_front(first);
    joiner.push_back(last);

    joiner
        .into_iter()
        .tuple_windows()
        .map(|(a, b)| {
            (
                (
                    &subspace_info.make_2d * &(a.0 - subspace_info.offset.clone()),
                    &subspace_info.make_2d * &(b.0 - subspace_info.offset.clone()),
                ),
                None,
            )
        })
        .for_each(|edge| {
            edges.insert_before(edge);
        });

    Ok((
        Face {
            points,
            color: ArcIntern::clone(&face.color),
        },
        found_region,
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use internment::ArcIntern;

    use crate::{Face, Point, do_cut, knife::PlaneCut, num::Vector};

    use super::{Cycle, recolor_border_edges};

    #[test]
    fn recolor() {
        let mut edges = Cycle(VecDeque::from(vec![
            ((Vector::zero(), Vector::zero()), Some(None)),
            ((Vector::zero(), Vector::zero()), None),
            ((Vector::zero(), Vector::zero()), None),
            ((Vector::zero(), Vector::zero()), None),
            (
                (Vector::zero(), Vector::zero()),
                Some(Some(ArcIntern::from("Green"))),
            ),
            ((Vector::zero(), Vector::zero()), None),
            ((Vector::zero(), Vector::zero()), None),
            ((Vector::zero(), Vector::zero()), None),
            (
                (Vector::zero(), Vector::zero()),
                Some(Some(ArcIntern::from("Green"))),
            ),
            ((Vector::zero(), Vector::zero()), Some(None)),
            ((Vector::zero(), Vector::zero()), None),
            ((Vector::zero(), Vector::zero()), None),
            ((Vector::zero(), Vector::zero()), None),
        ]));

        recolor_border_edges(&mut edges);
        edges.go_forward();

        // println!("{:#?}", edges.0);

        assert_eq!(edges.len(), 13);
        assert!(edges.0.iter().take(3).all(|v| v.1.is_none()));
        assert!(
            edges
                .0
                .iter()
                .skip(3)
                .take(5)
                .all(|v| v.1 == Some(Some(ArcIntern::from("Green"))))
        );
        assert!(edges.0.iter().skip(8).take(5).all(|v| v.1 == Some(None)));
    }

    #[test]
    fn plane_cut() {
        let face = Face {
            points: vec![
                Point(Vector::new([[1, 0, 1]])),
                Point(Vector::new([[1, 0, -1]])),
                Point(Vector::new([[-1, 0, -1]])),
                Point(Vector::new([[-1, 0, 1]])),
            ],
            color: ArcIntern::from("orange"),
        };

        let cutted = do_cut(
            &PlaneCut {
                spot: Vector::new_ratios([[(1, 2), (0, 1), (0, 1)]]),
                normal: Vector::new([[1, 0, 0]]),
                name: ArcIntern::from("R"),
            },
            &face,
            &face.subspace_info(),
        )
        .unwrap();
        // println!("{cutted:?}");

        assert_eq!(cutted.len(), 2);

        let face1 = Face {
            points: vec![
                Point(Vector::new([[1, 0, 1]])),
                Point(Vector::new([[1, 0, -1]])),
                Point(Vector::new_ratios([[(1, 2), (0, 1), (-1, 1)]])),
                Point(Vector::new_ratios([[(1, 2), (0, 1), (1, 1)]])),
            ],
            color: ArcIntern::from("orange"),
        };

        let face2 = Face {
            points: vec![
                Point(Vector::new_ratios([[(1, 2), (0, 1), (1, 1)]])),
                Point(Vector::new_ratios([[(1, 2), (0, 1), (-1, 1)]])),
                Point(Vector::new([[-1, 0, -1]])),
                Point(Vector::new([[-1, 0, 1]])),
            ],
            color: ArcIntern::from("orange"),
        };

        if cutted[0].0.epsilon_eq(&face1) {
            assert_eq!(cutted[0].1, Some(ArcIntern::from("R")));
            assert!(cutted[1].0.epsilon_eq(&face2));
            assert_eq!(cutted[1].1, None);
        } else {
            assert!(cutted[1].0.epsilon_eq(&face1));
            assert_eq!(cutted[1].1, Some(ArcIntern::from("R")));
            assert!(cutted[0].0.epsilon_eq(&face2));
            assert_eq!(cutted[0].1, None);
        }
    }
}

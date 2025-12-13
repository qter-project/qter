#![warn(clippy::pedantic)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::float_cmp)]
#![allow(mixed_script_confusables)]

use std::{
    cmp::Ordering,
    collections::{BTreeSet, HashMap},
    mem,
    num::NonZeroU16,
    sync::{Arc, LazyLock, OnceLock},
};

use edge_cloud::EdgeCloud;
use internment::ArcIntern;
use itertools::Itertools;
use knife::{CutSurface, do_cut};
use ksolve::{KSolve, KSolveMove, KSolveSet};
use num::{Matrix, Num, Vector, rotate_to, rotation_about};
use qter_core::{
    Span,
    architectures::{Permutation, PermutationGroup},
    union_find::UnionFind,
};
use thiserror::Error;

mod edge_cloud;
pub mod knife;
pub mod ksolve;
pub mod num;
pub mod shapes;

// Note... X is left to right, Y is down to up, and Z is forwards to backwards
// The coordinate system is right-handed

type PuzzleDescriptionString<'a> = &'a str;

#[derive(Error, Debug)]
pub enum PuzzleGeometryError {
    #[error("The vertices of the face are not coplanar: {0:?}")]
    FaceNotCoplanar(Face),
    #[error("The face forms a line or a point rather than a plane, or has collinear edges: {0:?}")]
    FaceIsDegenerate(Face),
    #[error(
        "A cut surface has cyclical structure and cannot be cut. Consider re-ordering the cut surfaces. Cut: {0}; Face: {1:?}"
    )]
    CyclicalCutSurface(String, Face),
    #[error("The slice {0} does not have any rotational symmetry")]
    PuzzleLacksSymmetry(ArcIntern<str>),
}

static DEG_180: LazyLock<Vector<2>> = LazyLock::new(|| Vector::new([[-1, 0]]));
static DEG_120: LazyLock<Vector<2>> = LazyLock::new(|| {
    Vector::new([[
        Num::from(-1) / Num::from(2),
        Num::from(1) / Num::from(2) * Num::from(3).sqrt(),
    ]])
});
static DEG_90: LazyLock<Vector<2>> = LazyLock::new(|| Vector::new([[0, 1]]));
static DEG_72: LazyLock<Vector<2>> = LazyLock::new(|| {
    let fourth = Num::from(1) / Num::from(4);
    Vector::new([[
        Num::from(5).sqrt() / Num::from(4) - fourth.clone(),
        (Num::from(2) * Num::from(5).sqrt() + Num::from(10)).sqrt() * fourth,
    ]])
});
static DEG_36: LazyLock<Vector<2>> = LazyLock::new(|| {
    let fourth = Num::from(1) / Num::from(4);
    Vector::new([[
        fourth.clone() * Num::from(5).sqrt() + fourth.clone(),
        fourth * (Num::from(-2) * Num::from(5).sqrt() + Num::from(10)).sqrt(),
    ]])
});

#[derive(Clone, Debug)]
pub struct Point(Vector<3>);

#[derive(Clone, Debug)]
pub struct Face {
    pub points: Vec<Point>,
    pub color: ArcIntern<str>,
}

impl Face {
    fn is_valid(&self) -> Result<(), PuzzleGeometryError> {
        // TEST DEGENERACY

        if self.points.len() <= 2 {
            return Err(PuzzleGeometryError::FaceIsDegenerate(self.to_owned()));
        }

        if self
            .points
            .iter()
            .circular_tuple_windows()
            .any(|(a, b, c)| {
                let line1 = b.0.clone() - a.0.clone();
                let line2 = b.0.clone() - c.0.clone();

                let abs_dot = line1.clone().dot(line2.clone()).abs();

                abs_dot.clone() * abs_dot == line1.norm_squared() * line2.norm_squared()
            })
        {
            return Err(PuzzleGeometryError::FaceIsDegenerate(self.to_owned()));
        }

        // TEST COPLANAR

        let FaceSubspaceInfo {
            make_3d,
            make_2d,
            offset,
        } = self.subspace_info();

        // Project points into the subspace
        let plane_proj = &make_3d * &make_2d;

        for point in self.points.iter().skip(3) {
            let offsetted = point.0.clone() - offset.clone();
            if &plane_proj * &offsetted != offsetted {
                return Err(PuzzleGeometryError::FaceNotCoplanar(self.to_owned()));
            }
        }

        Ok(())
    }

    fn transformed(&self, matrix: &Matrix<3, 3>) -> Self {
        Self {
            points: self
                .points
                .iter()
                .map(|point| Point(matrix * &point.0))
                .collect(),
            color: ArcIntern::clone(&self.color),
        }
    }

    fn edges(&self) -> impl Iterator<Item = (Vector<3>, Vector<3>)> {
        self.points
            .iter()
            .cycle()
            .tuple_windows()
            .take(self.points.len())
            .map(|(a, b)| (a.0.clone(), b.0.clone()))
    }

    fn edge_cloud(&self) -> EdgeCloud {
        EdgeCloud::new(self.edges().collect())
    }

    #[allow(dead_code)] // This is a false positive???
    fn epsilon_eq(&self, other: &Face) -> bool {
        self.edge_cloud().epsilon_eq(&other.edge_cloud())
    }

    /// Returns a pair of matrices where the first matrix projects a 2D vector into the 3D subspace spanned by this face, and the second computes the projection of a 3D vector into the 2D subspace.
    ///
    /// Also returns an origin vector to capture the translation of the face with respect to ⟨0, 0, 0⟩.
    fn subspace_info(&self) -> FaceSubspaceInfo {
        let offset = self.points[0].0.clone();

        // These two vectors define a 3D subspace that all points in the face should lie in
        let basis1 = self.points[1].0.clone() - offset.clone();
        let basis2 = self.points[2].0.clone() - offset.clone();

        // Transforms a 2D space into the 3D subspace
        // Make it orthogonal because that's nice to have
        let make_3d =
            Matrix::new([basis1.vec_into_inner(), basis2.vec_into_inner()]).mk_orthonormal();
        // Project points in 3D space into the subspace and into the 2D space
        // The transpose is the pseudo-inverse because `make_3d` is orthonormal and has full column rank
        let make_2d = make_3d.clone().transpose();

        FaceSubspaceInfo {
            make_3d,
            make_2d,
            offset,
        }
    }

    fn centroid(&self) -> Vector<3> {
        self.points.iter().map(|v| &v.0).cloned().sum::<Vector<3>>() / &Num::from(self.points.len())
    }
}

/// Encodes the information about the plane on which a face lies.
#[derive(Clone, Debug)]
pub struct FaceSubspaceInfo {
    /// A matrix that converts a 2D vector to a 3D one in the subspace parallel to the face. To get a point on the face's plane, add `offset`.
    make_3d: Matrix<3, 2>,
    /// Projects a 3D vector into the subspace parallel to the face. Given a point on the face's plane, subtract `offset` first.
    make_2d: Matrix<2, 3>,
    /// The offset of the face from the origin. Subspaces must always include the origin due to how subspaces work mathematically so when projecting in/out, it is necessary to take the offset into account.
    offset: Vector<3>,
}

impl FaceSubspaceInfo {
    #[must_use]
    pub fn make_3d(&self, vec: &Vector<2>) -> Vector<3> {
        (&self.make_3d * vec) + self.offset.clone()
    }

    #[must_use]
    pub fn make_2d(&self, vec: Vector<3>) -> Vector<2> {
        &self.make_2d * &(vec - self.offset.clone())
    }
}

#[derive(Clone, Debug)]
pub struct Polyhedron(pub Vec<Face>);

#[derive(Clone, Debug)]
pub struct PuzzleGeometryDefinition {
    pub polyhedron: Polyhedron,
    pub cut_surfaces: Vec<Arc<dyn CutSurface>>,
    pub definition: Span,
}

#[derive(Clone, Debug)]
pub struct PuzzleGeometry {
    stickers: Vec<(Face, Vec<ArcIntern<str>>)>,
    turns: HashMap<ArcIntern<str>, (Vector<3>, Matrix<3, 3>, usize)>,
    definition: Span,
    perm_group: OnceLock<(Arc<PermutationGroup>, BTreeSet<usize>)>,
    non_fixed_stickers: OnceLock<Vec<(Face, Vec<ArcIntern<str>>)>>,
    ksolve: OnceLock<Arc<KSolve>>,
}

impl PuzzleGeometry {
    /// Get the puzzle as a permutation group over facelets
    pub fn permutation_group(&self) -> Arc<PermutationGroup> {
        Arc::clone(&self.calc_permutation_group().0)
    }

    fn calc_permutation_group(&self) -> &(Arc<PermutationGroup>, BTreeSet<usize>) {
        self.perm_group.get_or_init(|| {
            let clouds = self.stickers()
                .iter()
                .map(|v| v.0.edge_cloud())
                .collect::<Vec<_>>();

            let mut base_generators = Vec::new();

            for (name, turn) in &self.turns {
                let mut mapping = Vec::new();

                for sticker in self.stickers() {
                    if !sticker.1.contains(name) {
                        mapping.push(mapping.len());
                        continue;
                    }

                    let mut face = sticker.0.clone();
                    for point in &mut face.points {
                        *point = Point(&turn.1 * &(point.0.clone() - turn.0.clone()) + turn.0.clone());
                    }

                    let cloud = face.edge_cloud();

                    let (spot, _) = clouds
                        .iter()
                        .find_position(|test_cloud| cloud.epsilon_eq(test_cloud)).expect("We already verified this turn to work when creating the PuzzleGeometry instance");

                    mapping.push(spot);
                }

                base_generators.push((name, mapping, turn.2));
            }

            let to_skip = (0..self.stickers().len()).filter(|i| base_generators.iter().all(|(_, mapping, _)| mapping[*i] == *i)).collect::<BTreeSet<_>>();

            let mut generators = HashMap::new();

            for (name, mapping, symm) in base_generators {
                let base = Permutation::from_mapping(mapping.into_iter().enumerate().filter(|(i, _)| !to_skip.contains(i)).map(|(_, v)| v - to_skip.range(0..v).count()).collect());
                let mut current = base.clone();

                let names = turn_names(name, symm);

                for name in names {
                    generators.insert(name, current.clone());
                    current.compose_into(&base);
                }
            }

            (Arc::new(PermutationGroup::new(
                self.stickers()
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !to_skip.contains(i))
                    .map(|(_, v)| ArcIntern::clone(&v.0.color))
                    .collect(),
                generators,
                self.definition.clone(),
            )), to_skip)
        })
    }

    #[must_use]
    pub fn stickers(&self) -> &[(Face, Vec<ArcIntern<str>>)] {
        &self.stickers
    }

    pub fn non_fixed_stickers(&self) -> &[(Face, Vec<ArcIntern<str>>)] {
        self.non_fixed_stickers.get_or_init(|| {
            let (_, fixed) = self.calc_permutation_group();

            self.stickers
                .iter()
                .enumerate()
                .filter(|(i, _)| !fixed.contains(i))
                .map(|(_, v)| v.clone())
                .collect()
        })
    }

    /// Returns the orientation number for each sticker as well as the orientation count for each orbit. The way the algorithm works, you get both numbers.
    ///
    /// Assigns signature facelets in an unspecified but consistent way
    fn number_facelet_orientations(
        group: &PermutationGroup,
        sticker_orbits: &UnionFind<()>,
        orbits: &[Vec<Vec<usize>>],
    ) -> (Vec<usize>, Vec<usize>) {
        let mut facelet_orientation_numbers: Vec<Option<usize>> = vec![None; group.facelet_count()];
        let mut orientation_counts = Vec::new();

        for orbit in orbits {
            // Number the very first piece arbitrarily
            let piece = &orbit[0];
            let mut reps_to_count = HashMap::new();

            for i in piece {
                let rep = sticker_orbits.find(*i).root_idx();
                let value = reps_to_count.entry(rep).or_insert(0);
                facelet_orientation_numbers[*i] = Some(*value);
                *value += 1;
            }

            let ori_count = reps_to_count
                .values()
                .all_equal_value()
                .expect("All values to be equal");
            orientation_counts.push(*ori_count);

            // Number all of the pieces such that orientation is invariant over as many turns as possible

            // This also ensures that the orientation offset is the same for every sticker for every move.
            // If the sticker numbers were assigned arbitrarily, it would be possible for the 0 sticker to move to a 1 sticker and a 1 sticker to move to a 0 sticker which would be bad.

            let mut overall_not_done = true;

            while overall_not_done {
                overall_not_done = false;

                for generator in group.generators() {
                    let mut not_done = true;

                    while not_done {
                        not_done = false;

                        for (from, to) in generator.1.mapping().iter().copied().enumerate() {
                            if let Some(number) = facelet_orientation_numbers[from]
                                && facelet_orientation_numbers[to].is_none()
                            {
                                facelet_orientation_numbers[to] = Some(number);
                                not_done = true;
                                overall_not_done = true;
                            }
                        }
                    }
                }
            }
        }

        (
            facelet_orientation_numbers
                .into_iter()
                .map(|v| v.unwrap())
                .collect(),
            orientation_counts,
        )
    }

    /// Get the puzzle in its `KSolve` representation
    ///
    /// # Panics
    ///
    /// May panic if calculated numbers fall outside of the bit width of the fields of `KSolve`
    #[must_use]
    pub fn ksolve(&self) -> Arc<KSolve> {
        // Note: the KSolve permutation vector is **1-indexed**. See the test
        // cases for examples. It also exposes `zero_indexed_transformation` as
        // a convenience method.
        Arc::clone(self.ksolve.get_or_init(|| {
            let group = self.permutation_group();

            let mut sticker_orbits = UnionFind::<()>::new(group.facelet_count());

            for (_, generator) in group.generators() {
                for (a, b) in generator.mapping().iter().enumerate() {
                    sticker_orbits.union(a, *b, ());
                }
            }

            let mut pieces: HashMap<Vec<ArcIntern<str>>, Vec<usize>> = HashMap::new();

            for (sticker, (_, regions)) in self.non_fixed_stickers().iter().enumerate() {
                pieces
                    .entry(regions.iter().sorted_unstable().cloned().collect())
                    .or_default()
                    .push(sticker);
            }

            let mut orbits: Vec<Vec<Vec<usize>>> = Vec::new();

            'next_piece: for (_, piece) in pieces {
                let orbit_rep = sticker_orbits.find(piece[0]).root_idx();
                for maybe_orbit in &mut orbits {
                    if maybe_orbit[0].len() != piece.len() {
                        continue;
                    }

                    for facelet in &maybe_orbit[0] {
                        if sticker_orbits.find(*facelet).root_idx() == orbit_rep {
                            maybe_orbit.push(piece);
                            continue 'next_piece;
                        }
                    }
                }

                orbits.push(vec![piece]);
            }

            let (facelet_orientation_numbers, orientation_counts) =
                Self::number_facelet_orientations(&group, &sticker_orbits, &orbits);

            let mut sets: Vec<KSolveSet> = Vec::new();

            for (i, (orbit, orientation_count)) in
                orbits.iter().zip(orientation_counts.iter()).enumerate()
            {
                // TODO: Reasonable names?

                sets.push(KSolveSet {
                    name: i.to_string(),
                    piece_count: u16::try_from(orbit.len()).unwrap().try_into().unwrap(),
                    orientation_count: (u8::try_from(*orientation_count))
                        .unwrap()
                        .try_into()
                        .unwrap(),
                });
            }

            let mut moves: Vec<KSolveMove> = Vec::new();

            let mut sticker_to_piece_mapping = vec![0; group.facelet_count()];

            for orbit in &orbits {
                for (piece_idx, piece) in orbit.iter().enumerate() {
                    for i in piece {
                        sticker_to_piece_mapping[*i] = piece_idx;
                    }
                }
            }

            for (name, perm) in group.generators() {
                let mut transformation = Vec::new();

                for (orbit, ori_count) in orbits.iter().zip(orientation_counts.iter()) {
                    let mut this_orbit_transform = Vec::new();

                    for piece in orbit {
                        let first_one_goes_to = perm.mapping()[piece[0]];

                        let starting_orientation = facelet_orientation_numbers[piece[0]];
                        let new_orientation = facelet_orientation_numbers[first_one_goes_to];
                        // Add ori_count first to prevent wraparound from subtraction
                        let extra_orientation = (ori_count + new_orientation
                            - starting_orientation)
                            .rem_euclid(*ori_count);

                        let piece_goes_to = sticker_to_piece_mapping[first_one_goes_to];

                        this_orbit_transform.push((
                            NonZeroU16::try_from(u16::try_from(piece_goes_to + 1).unwrap())
                                .unwrap(),
                            u8::try_from(extra_orientation).unwrap(),
                        ));
                    }

                    transformation.push(this_orbit_transform);
                }

                moves.push(KSolveMove {
                    transformation,
                    name: name.to_string(),
                });
            }

            moves.sort_by(|a, b| turn_compare(a.name(), b.name()));

            Arc::new(KSolve {
                name: self.definition.to_string(),
                sets,
                moves,
                symmetries: Vec::new(),
            })
        }))
    }
}

impl PuzzleGeometryDefinition {
    /// Consume a `PuzzleGeometryDefinition` and return a `PuzzleGeometry`
    ///
    /// # Errors
    ///
    /// If the validity of the faces is not satisfied, or if the puzzle does
    /// not have the expected symmetries, this function will return an error.
    #[expect(clippy::missing_panics_doc)]
    pub fn geometry(self) -> Result<PuzzleGeometry, PuzzleGeometryError> {
        let mut faces: Vec<(Face, Vector<3>)> = vec![];
        for face in self.polyhedron.0 {
            face.is_valid()?;
            let centroid = face.centroid();
            faces.push((face, centroid));
        }

        faces.sort_by(|a, b| point_compare(&a.1, &b.1));

        let mut stickers: Vec<(Face, Vec<ArcIntern<str>>)> = Vec::new();

        for (face, _) in faces {
            let subspace_info = face.subspace_info();

            let mut face_stickers = vec![(face, vec![])];

            for cut_surface in &self.cut_surfaces {
                let mut new_stickers = Vec::new();

                for (sticker, name_components) in face_stickers {
                    new_stickers.extend(
                        do_cut(&**cut_surface, &sticker, &subspace_info)?
                            .into_iter()
                            .map(move |(new_face, name_component)| {
                                let mut name_components = name_components.clone();
                                if let Some(component) = name_component {
                                    name_components.push(component);
                                }
                                (new_face, name_components)
                            }),
                    );
                }

                face_stickers = new_stickers;
            }

            face_stickers.sort_by_cached_key(|v| {
                let [[x, y]] = subspace_info.make_2d(v.0.centroid()).into_inner();
                [-y, x]
            });

            stickers.extend(face_stickers);
        }

        let mut turns = HashMap::new();
        let names = stickers.iter().flat_map(|v| v.1.iter()).unique();

        for name in names {
            let stickers = stickers
                .iter()
                .filter(|(_, names)| names.contains(name))
                .map(|(face, included_in)| (face, included_in.clone()))
                .collect_vec();

            // The center of mass must be preserved over rotations therefore any axis of symmetry must pass through it.
            let center_of_mass = stickers
                .iter()
                .flat_map(|v| &v.0.points)
                .map(|v| v.0.clone())
                .sum::<Vector<3>>()
                / &Num::from(stickers.len());

            let mut edges = stickers.iter().flat_map(|v| v.0.edges()).collect_vec();

            for edge in &mut edges {
                edge.0 -= center_of_mass.clone();
                edge.1 -= center_of_mass.clone();
            }

            // Compute the vector that we think is facing "out". Our heuristic will be to calculate the centroid of all of the points farthest away from the centroid of our stickers. Then, "outside" will face exactly away from that second centroid. The justification is that since the side facing out is tiled with stickers whereas the side facing in is not, then the centroid will be closer to that outer face. That means that the points farthest away from the centroid will be on the back face. By taking their centroid, we get a point that is behind the centroid. Therefore, negating that vector gives a point in front of the centroid.
            // In cases with symmetry where this centroid is exactly the normal centroid, we take out to be the difference between this centroid and the predefined center of the whole shape (which is just the origin).

            // Take the first point from each edge since we would rather not process points twice as many times as we have to
            let farthest_points = edges
                .iter()
                .map(|v| &v.0)
                .max_set_by_key(|v| (*v).clone().norm_squared());
            let len = farthest_points.len();
            let second_centroid =
                farthest_points.into_iter().cloned().sum::<Vector<3>>() / &Num::from(len);

            let out_direction = if second_centroid.is_zero() {
                center_of_mass.clone()
            } else {
                -second_centroid
            };

            // Narrow down the edges that could potentially map to each other so that we don't have to try all of them
            // Currently, we only classify edges by the distance from the origin of the two endpoints
            let mut edge_classifications: Vec<((Num, Num), Vec<(Matrix<3, 1>, Matrix<3, 1>)>)> =
                Vec::new();

            'next_edge: for edge in &edges {
                let mut a = edge.0.clone().norm_squared();
                let mut b = edge.1.clone().norm_squared();
                if a > b {
                    mem::swap(&mut a, &mut b);
                }

                for ((maybe_a, maybe_b), list) in &mut edge_classifications {
                    if a == *maybe_a && b == *maybe_b {
                        list.push(edge.clone());
                        continue 'next_edge;
                    }
                }

                edge_classifications.push(((a, b), vec![edge.clone()]));
            }

            // Find the smallest set of edges that can map together and operate on them.
            let edges_that_might_map_together = edge_classifications
                .into_iter()
                .min_by_key(|v| v.1.len())
                .unwrap()
                .1;

            let from = Matrix::new([
                edges_that_might_map_together[0].0.clone().vec_into_inner(),
                edges_that_might_map_together[0].1.clone().vec_into_inner(),
            ]);

            let matrices = edges_that_might_map_together
                .into_iter()
                .flat_map(|(a, b)| [(a.clone(), b.clone()), (b, a)])
                .skip(1)
                .map(|v| {
                    let to = Matrix::new([v.0.vec_into_inner(), v.1.vec_into_inner()]);
                    rotate_to(from.clone(), to)
                })
                .filter(|v| {
                    // Remove counterclockwise rotations; it would be cursed if `R` was counterclockwise
                    let v = v.inner();
                    // This is the axis about which the turn would be counter-clockwise
                    // https://en.wikipedia.org/wiki/Rotation_matrix#Determining_the_axis
                    let axis = Vector::new([[
                        v[1][2].clone() - v[2][1].clone(),
                        v[2][0].clone() - v[0][2].clone(),
                        v[0][1].clone() - v[1][0].clone(),
                    ]]);

                    // If the axis is the zero vector, then the rotation is either 0 or 180 degrees and there isn't a sense of "clockwise"
                    if axis.is_zero() {
                        return true;
                    }

                    // If the counterclockwise axis is facing out, then this turn is counterclockwise and we should not process it. If this was truly a valid turn, then we will see the clockwise version by seeing the edge in the clockwise direction.
                    axis.dot(out_direction.clone()).cmp_zero().is_gt()
                });

            let cloud = EdgeCloud::new(edges);

            match matrices
                .filter_map(|matrix| {
                    cloud
                        .clone()
                        .try_symmetry(&matrix)
                        .map(|degree| (matrix, degree))
                })
                .max_by_key(|v| v.1)
            {
                None | Some((_, 1)) => {
                    return Err(PuzzleGeometryError::PuzzleLacksSymmetry(name.clone()));
                }
                Some((matrix, degree)) => {
                    turns.insert(name.clone(), (center_of_mass, matrix, degree));
                }
            }
        }

        Ok(PuzzleGeometry {
            stickers,
            turns,
            definition: self.definition,
            perm_group: OnceLock::new(),
            ksolve: OnceLock::new(),
            non_fixed_stickers: OnceLock::new(),
        })
    }
}

fn turn_names(base_name: &ArcIntern<str>, symm: usize) -> Vec<ArcIntern<str>> {
    let mut names_begin = Vec::new();
    let mut names_end = Vec::new();

    let mut i = 1;

    while names_begin.len() + names_end.len() < symm - 1 {
        if names_begin.len() == names_end.len() {
            if i == 1 {
                names_begin.push(ArcIntern::clone(base_name));
            } else {
                names_begin.push(ArcIntern::from(format!("{base_name}{i}")));
            }
        } else {
            if i == 1 {
                names_end.push(ArcIntern::from(format!("{base_name}'")));
            } else {
                names_end.push(ArcIntern::from(format!("{base_name}{i}'")));
            }

            i += 1;
        }
    }

    names_begin.extend(names_end.into_iter().rev());

    names_begin
}

fn turn_compare(a: &str, b: &str) -> Ordering {
    // Separates a turn name into the name, number, and whether it's prime
    fn separate(name: &str) -> (&str, &str, bool) {
        let (without_prime, prime) = match name.strip_suffix('\'') {
            Some(prefix) => (prefix, true),
            None => (name, false),
        };

        let without_number = without_prime.trim_end_matches(|c: char| c.is_ascii_digit());

        (
            without_number,
            without_prime.split_at(without_number.len()).1,
            prime,
        )
    }

    let (a_name, a_numbers, a_prime) = separate(a);
    let (b_name, b_numbers, b_prime) = separate(b);

    match a_name.cmp(b_name) {
        Ordering::Equal => match a_prime.cmp(&b_prime) {
            Ordering::Equal => {
                let ordering = match a_numbers.len().cmp(&b_numbers.len()) {
                    Ordering::Equal => a_numbers.cmp(b_numbers),
                    ordering => ordering,
                };

                if a_prime {
                    ordering.reverse()
                } else {
                    ordering
                }
            }
            ordering => ordering,
        },
        ordering => ordering,
    }
}

/// Sort all of the faces from top-to-bottom first, counter-clockwise second, and in-to-out third.
fn point_compare(a: &Vector<3>, b: &Vector<3>) -> Ordering {
    fn region(x: &Num, z: &Num) -> u8 {
        match (x.cmp_zero(), z.cmp_zero()) {
            (Ordering::Less, Ordering::Equal | Ordering::Less) => 1,
            (Ordering::Equal | Ordering::Greater, Ordering::Less) => 2,
            (Ordering::Greater, Ordering::Greater | Ordering::Equal) => 3,
            (Ordering::Less | Ordering::Equal, Ordering::Greater) => 4,
            (Ordering::Equal, Ordering::Equal) => 5,
        }
    }

    fn de_rotate(x: Num, z: Num, region: u8) -> (Num, Num) {
        match region {
            1 => (x, z),
            2 => (z, -x),
            3 => (-x, -z),
            4 => (-z, x),
            _ => unreachable!(),
        }
    }

    let [x1, y1, z1] = a.vec_inner();
    let [x2, y2, z2] = b.vec_inner();

    match y2.cmp(y1) {
        Ordering::Equal => {
            let r1 = region(x1, z1);
            let r2 = region(x2, z2);

            match r1.cmp(&r2) {
                Ordering::Equal => {
                    if r1 == 5 {
                        return Ordering::Equal;
                    }

                    let (x1, z1) = de_rotate(x1.clone(), z1.clone(), r1);
                    let (x2, z2) = de_rotate(x2.clone(), z2.clone(), r2);

                    [(z1.clone() / x1.clone()), z1.abs()].cmp(&[(z2.clone() / x2), z2.abs()])
                }
                v => v,
            }
        }
        v => v,
    }
}

#[cfg(test)]
mod tests {
    use std::{cmp::Ordering, collections::HashSet, sync::Arc};

    use crate::{
        DEG_36, DEG_72, DEG_90, DEG_120, DEG_180, Face, Point, PuzzleGeometryDefinition,
        PuzzleGeometryError,
        knife::{CutSurface, PlaneCut},
        ksolve::KSolveMove,
        num::{Num, Vector},
        point_compare,
        shapes::{CUBE, DODECAHEDRON, TETRAHEDRON, print_shapes},
        turn_compare, turn_names,
    };
    use internment::ArcIntern;
    use itertools::Itertools;
    use qter_core::{Int, Span, U, architectures::Permutation, schreier_sims::StabilizerChain};

    #[test]
    fn valid_rotators() {
        assert_eq!(DEG_180.clone().norm(), Num::from(1));
        assert_eq!(DEG_120.clone().norm(), Num::from(1));
        assert_eq!(DEG_90.clone().norm(), Num::from(1));
        assert_eq!(DEG_72.clone().norm(), Num::from(1));
        assert_eq!(DEG_36.clone().norm(), Num::from(1));
    }

    #[test]
    fn test_turn_names() {
        assert_eq!(
            turn_names(&ArcIntern::from("R"), 4),
            [
                ArcIntern::from("R"),
                ArcIntern::from("R2"),
                ArcIntern::from("R'")
            ]
        );
        assert_eq!(
            turn_names(&ArcIntern::from("U"), 5),
            [
                ArcIntern::from("U"),
                ArcIntern::from("U2"),
                ArcIntern::from("U2'"),
                ArcIntern::from("U'")
            ]
        );
    }

    #[test]
    fn degeneracy() {
        let valid = Face {
            points: vec![Point(Vector::new([[1, 2, 3]]))],
            color: ArcIntern::from("aliceblue"),
        }
        .is_valid();
        assert!(matches!(
            valid,
            Err(PuzzleGeometryError::FaceIsDegenerate(_))
        ));

        let valid = Face {
            points: vec![
                Point(Vector::new([[1, 2, 3]])),
                Point(Vector::new([[5, 4, 3]])),
            ],
            color: ArcIntern::from("oklch(1 2 3)"),
        }
        .is_valid();
        assert!(matches!(
            valid,
            Err(PuzzleGeometryError::FaceIsDegenerate(_))
        ));

        let valid = Face {
            points: vec![
                Point(Vector::new([[2, 2, 3]])),
                Point(Vector::new([[3, 4, 6]])),
                Point(Vector::new([[4, 6, 9]])),
            ],
            color: ArcIntern::from("fuschia"),
        }
        .is_valid();
        assert!(matches!(
            valid,
            Err(PuzzleGeometryError::FaceIsDegenerate(_))
        ));
    }

    #[test]
    fn not_coplanar() {
        let valid = Face {
            points: vec![
                Point(Vector::new([[2, 2, 3]])),
                Point(Vector::new([[3, 4, 6]])),
                Point(Vector::new([[4, 6, 11]])),
                Point(Vector::new([[6, 6, 11]])),
            ],
            color: ArcIntern::from("blue"),
        }
        .is_valid();

        assert!(matches!(
            valid,
            Err(PuzzleGeometryError::FaceNotCoplanar(_))
        ));

        let valid = Face {
            points: vec![
                Point(Vector::new([[1, 1, 1]])),
                Point(Vector::new([[1, 1, 0]])),
                Point(Vector::new([[1, 0, 0]])),
                Point(Vector::new([[1, 0, 1]])),
            ],
            color: ArcIntern::from("bruh"),
        }
        .is_valid();

        assert!(matches!(valid, Ok(())));
    }

    #[test]
    fn test_point_compare() {
        fn test<N: Into<Num>>(x1: N, y1: N, z1: N, x2: N, y2: N, z2: N, expected: Ordering) {
            let a = Vector::new([[x1, y1, z1]]);
            let b = Vector::new([[x2, y2, z2]]);
            assert_eq!(point_compare(&a, &b), expected, "{a:?}, {b:?}");
            assert_eq!(point_compare(&b, &a), expected.reverse(), "{b:?}, {a:?}");
            assert_eq!(point_compare(&a, &a), Ordering::Equal, "{a:?}, {a:?}");
            assert_eq!(point_compare(&b, &b), Ordering::Equal, "{a:?}, {a:?}");
        }

        // Top to bottom first
        test(1, 1, 1, 1, 0, 1, Ordering::Less);
        test(1, -10, 1, 1, 0, 1, Ordering::Greater);

        // Counterclockwise second; boundary condition
        test(-1, 0, 0, 0, 0, -1, Ordering::Less);
        test(0, 0, -1, 1, 0, 0, Ordering::Less);
        test(1, 0, 0, 0, 0, 1, Ordering::Less);
        test(0, 0, 1, -1, 0, 0, Ordering::Greater);

        // Non-boundary condition
        test(-1, 0, -1, 1, 0, -1, Ordering::Less);
        test(1, 0, -1, 1, 0, 1, Ordering::Less);
        test(1, 0, 1, -1, 0, 1, Ordering::Less);
        test(-1, 0, 1, -1, 0, -1, Ordering::Greater);

        // Same region, different angle, same distance
        test(-2, 0, -1, -1, 0, -2, Ordering::Less);
        test(1, 0, -2, 2, 0, -1, Ordering::Less);
        test(2, 0, 1, 1, 0, 2, Ordering::Less);
        test(-1, 0, 2, -2, 0, 1, Ordering::Less);

        // Same region, different angle, different distance
        test(-3, 0, -1, -1, 0, -2, Ordering::Less);
        test(1, 0, -2, 3, 0, -1, Ordering::Less);
        test(3, 0, 1, 1, 0, 2, Ordering::Less);
        test(-1, 0, 2, -3, 0, 1, Ordering::Less);

        // Same region, same angle, different distance
        test(-1, 0, -1, -2, 0, -2, Ordering::Less);
        test(1, 0, -1, 2, 0, -2, Ordering::Less);
        test(1, 0, 1, 2, 0, 2, Ordering::Less);
        test(-1, 0, 1, -2, 0, 2, Ordering::Less);
    }

    #[test]
    fn three_by_three() {
        let cube = PuzzleGeometryDefinition {
            polyhedron: CUBE.to_owned(),
            cut_surfaces: vec![
                Arc::from(PlaneCut {
                    spot: Vector::new_ratios([[(1, 3), (0, 1), (0, 1)]]),
                    normal: Vector::new([[1, 0, 0]]),
                    name: ArcIntern::from("R"),
                }),
                Arc::from(PlaneCut {
                    spot: Vector::new_ratios([[(-1, 3), (0, 1), (0, 1)]]),
                    normal: Vector::new([[-1, 0, 0]]),
                    name: ArcIntern::from("L"),
                }),
                Arc::from(PlaneCut {
                    spot: Vector::new_ratios([[(0, 1), (1, 3), (0, 1)]]),
                    normal: Vector::new([[0, 1, 0]]),
                    name: ArcIntern::from("U"),
                }),
                Arc::from(PlaneCut {
                    spot: Vector::new_ratios([[(0, 1), (-1, 3), (0, 1)]]),
                    normal: Vector::new([[0, -1, 0]]),
                    name: ArcIntern::from("D"),
                }),
                Arc::from(PlaneCut {
                    spot: Vector::new_ratios([[(0, 1), (0, 1), (-1, 3)]]),
                    normal: Vector::new([[0, 0, -1]]),
                    name: ArcIntern::from("F"),
                }),
                Arc::from(PlaneCut {
                    spot: Vector::new_ratios([[(0, 1), (0, 1), (1, 3)]]),
                    normal: Vector::new([[0, 0, 1]]),
                    name: ArcIntern::from("B"),
                }),
            ],
            definition: Span::new(ArcIntern::from("3x3"), 0, 3),
        };

        let geometry = cube.geometry().unwrap();
        assert_eq!(geometry.stickers().len(), 54);

        for turn in &geometry.turns {
            assert_eq!(turn.1.2, 4);
        }
        assert_eq!(geometry.turns.len(), 6);

        let group = geometry.permutation_group();
        assert_eq!(group.facelet_count(), 48);

        assert_eq!(
            StabilizerChain::new(&group).cardinality(),
            "43252003274489856000".parse::<Int<U>>().unwrap()
        );

        // https://www.math.rwth-aachen.de/homes/GAP/WWW2/Doc/Examples/rubik.html
        assert_eq!(
            group.get_generator("U").unwrap(),
            &Permutation::from_cycles(vec![
                vec![0, 2, 7, 5],
                vec![1, 4, 6, 3],
                vec![8, 32, 24, 16],
                vec![9, 33, 25, 17],
                vec![10, 34, 26, 18]
            ])
        );
        assert_eq!(
            group.get_generator("L").unwrap(),
            &Permutation::from_cycles(vec![
                vec![8, 10, 15, 13],
                vec![9, 12, 14, 11],
                vec![0, 16, 40, 39],
                vec![3, 19, 43, 36],
                vec![5, 21, 45, 34]
            ])
        );
        assert_eq!(
            group.get_generator("F").unwrap(),
            &Permutation::from_cycles(vec![
                vec![16, 18, 23, 21],
                vec![17, 20, 22, 19],
                vec![5, 24, 42, 15],
                vec![6, 27, 41, 12],
                vec![7, 29, 40, 10]
            ])
        );
        assert_eq!(
            group.get_generator("R").unwrap(),
            &Permutation::from_cycles(vec![
                vec![24, 26, 31, 29],
                vec![25, 28, 30, 27],
                vec![2, 37, 42, 18],
                vec![4, 35, 44, 20],
                vec![7, 32, 47, 23]
            ])
        );
        assert_eq!(
            group.get_generator("B").unwrap(),
            &Permutation::from_cycles(vec![
                vec![32, 34, 39, 37],
                vec![33, 36, 38, 35],
                vec![2, 8, 45, 31],
                vec![1, 11, 46, 28],
                vec![0, 13, 47, 26]
            ])
        );
        assert_eq!(
            group.get_generator("D").unwrap(),
            &Permutation::from_cycles(vec![
                vec![40, 42, 47, 45],
                vec![41, 44, 46, 43],
                vec![13, 21, 29, 37],
                vec![14, 22, 30, 38],
                vec![15, 23, 31, 39]
            ])
        );

        let ksolve = geometry.ksolve();

        // Make sure all of the moves are sorted properly
        assert_eq!(
            ksolve.moves().iter().map(KSolveMove::name).collect_vec(),
            vec![
                "B", "B2", "B'", "D", "D2", "D'", "F", "F2", "F'", "L", "L2", "L'", "R", "R2",
                "R'", "U", "U2", "U'"
            ]
        );

        assert_eq!(ksolve.moves().len(), 18);

        assert_eq!(ksolve.sets().len(), 2);
        let corner_idx = usize::from(ksolve.sets()[0].piece_count().get() != 8);
        let edge_idx = 1 - corner_idx;
        assert_eq!(ksolve.sets()[corner_idx].piece_count().get(), 8);
        assert_eq!(ksolve.sets()[edge_idx].piece_count().get(), 12);
        assert_eq!(ksolve.sets()[corner_idx].orientation_count().get(), 3);
        assert_eq!(ksolve.sets()[edge_idx].orientation_count().get(), 2);

        for generator in ksolve.moves() {
            let transform = generator.transformation();

            for (idx, orbit_transform) in transform.iter().enumerate() {
                let mut amt_moved = 0;
                let mut orientation_sum = 0;
                for (idx, spot) in orbit_transform.iter().enumerate() {
                    if idx + 1 == spot.0.get() as usize {
                        assert_eq!(spot.1, 0);
                    } else {
                        amt_moved += 1;
                        orientation_sum += spot.1;
                    }
                }

                assert_eq!(amt_moved, 4);

                if idx == edge_idx {
                    assert_eq!(orientation_sum.rem_euclid(2), 0);
                } else {
                    assert_eq!(orientation_sum.rem_euclid(3), 0);
                }
            }
        }
    }

    #[test]
    fn pyraminx() {
        let up = TETRAHEDRON.0[0].points[0].clone().0;
        let down1 = TETRAHEDRON.0[3].points[0].clone().0;
        let down2 = TETRAHEDRON.0[3].points[1].clone().0;
        let down3 = TETRAHEDRON.0[3].points[2].clone().0;

        let pyraminx = PuzzleGeometryDefinition {
            polyhedron: TETRAHEDRON.to_owned(),
            cut_surfaces: vec![
                Arc::from(PlaneCut {
                    spot: up.clone() / &Num::from(9),
                    normal: up.clone(),
                    name: ArcIntern::from("A"),
                }),
                Arc::from(PlaneCut {
                    spot: down1.clone() / &Num::from(9),
                    normal: down1.clone(),
                    name: ArcIntern::from("B"),
                }),
                Arc::from(PlaneCut {
                    spot: down2.clone() / &Num::from(9),
                    normal: down2.clone(),
                    name: ArcIntern::from("C"),
                }),
                Arc::from(PlaneCut {
                    spot: down3.clone() / &Num::from(9),
                    normal: down3.clone(),
                    name: ArcIntern::from("D"),
                }),
                Arc::from(PlaneCut {
                    spot: (up.clone() / &Num::from(9)) * &Num::from(5),
                    normal: up.clone(),
                    name: ArcIntern::from("E"),
                }),
                Arc::from(PlaneCut {
                    spot: (down1.clone() / &Num::from(9)) * &Num::from(5),
                    normal: down1.clone(),
                    name: ArcIntern::from("F"),
                }),
                Arc::from(PlaneCut {
                    spot: (down2.clone() / &Num::from(9)) * &Num::from(5),
                    normal: down2.clone(),
                    name: ArcIntern::from("G"),
                }),
                Arc::from(PlaneCut {
                    spot: (down3.clone() / &Num::from(9)) * &Num::from(5),
                    normal: down3.clone(),
                    name: ArcIntern::from("H"),
                }),
            ],
            definition: Span::new(ArcIntern::from("pyraminx"), 0, 8),
        };

        let geometry = pyraminx.geometry().unwrap();
        assert_eq!(geometry.stickers().len(), 36);

        for turn in &geometry.turns {
            assert_eq!(turn.1.2, 3);
        }
        assert_eq!(geometry.turns.len(), 8);

        let group = geometry.permutation_group();
        assert_eq!(group.facelet_count(), 36);

        assert_eq!(
            StabilizerChain::new(&group).cardinality(),
            "75582720".parse::<Int<U>>().unwrap()
        );
    }

    #[test]
    fn megaminx() {
        let megaminx = PuzzleGeometryDefinition {
            polyhedron: DODECAHEDRON.clone(),
            // idk if this cut depth is right but WHO CARES HAHAHAH
            cut_surfaces: DODECAHEDRON
                .0
                .iter()
                .map(|v| {
                    let centroid = v.centroid();

                    Arc::from(PlaneCut {
                        spot: v.centroid() * &Num::from(8) / &Num::from(9),
                        normal: centroid,
                        name: ArcIntern::clone(&v.color),
                    }) as Arc<dyn CutSurface + 'static>
                })
                .collect(),
            definition: Span::new(ArcIntern::from("dodecahedron"), 0, "dodecahedron".len()),
        };
        // print_shapes(megaminx.polyhedron.0.iter());

        let megaminx = megaminx.geometry().unwrap();

        assert_eq!(megaminx.ksolve().sets.len(), 2);
        assert_eq!(
            megaminx
                .ksolve()
                .sets
                .iter()
                .map(|v| (v.piece_count.get(), v.orientation_count.get()))
                .collect::<HashSet<_>>(),
            HashSet::from([(20, 3), (30, 2)])
        );

        // print_shapes(shapes);
        assert_eq!(megaminx.ksolve().moves.len(), 12 * 4);

        assert_eq!(
            StabilizerChain::new(&megaminx.permutation_group()).cardinality(),
            "100669616553523347122516032313645505168688116411019768627200000000000"
                .parse::<Int<U>>()
                .unwrap()
        );
    }

    #[test]
    fn test_turn_compare() {
        assert_eq!(turn_compare("A", "B"), Ordering::Less);
        assert_eq!(turn_compare("A'", "B"), Ordering::Less);
        assert_eq!(turn_compare("B'", "B"), Ordering::Greater);
        assert_eq!(turn_compare("B'", "B'"), Ordering::Equal);
        assert_eq!(turn_compare("B2'", "B2'"), Ordering::Equal);
        assert_eq!(turn_compare("B2'", "B2"), Ordering::Greater);
        assert_eq!(turn_compare("B2", "B2'"), Ordering::Less);
        assert_eq!(turn_compare("B2", "B3"), Ordering::Less);
        assert_eq!(turn_compare("B3", "B2"), Ordering::Greater);
        assert_eq!(turn_compare("B3", "B3"), Ordering::Equal);
        assert_eq!(turn_compare("B2'", "B3'"), Ordering::Greater);
        assert_eq!(turn_compare("B12'", "B3'"), Ordering::Less);
        assert_eq!(turn_compare("B3'", "B12'"), Ordering::Greater);
    }
}

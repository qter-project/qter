use wasm_minimal_protocol::*;
use serde::{Deserialize, Serialize};

initiate_protocol!();

#[derive(Deserialize, Clone)]
struct Params {
    faces: String,
    offset: (f64, f64),
    scale_amt: f64,
    distance: f64,
    back: bool,
}

#[derive(Serialize)]
enum Curve {
    Line {
        start: (f64, f64),
        end: (f64, f64),
    },
    Bezier {
        start: (f64, f64),
        control: (f64, f64),
        end: (f64, f64),
    }
}

#[derive(Serialize)]
enum Color {
    Sticker(char),
    Shadow,
}

#[derive(Serialize)]
enum Action {
    Anchor {
        name: String,
        spot: (f64, f64),
    },
    MergePath {
        color: Color,
        curves: Vec<Curve>,
    }
}

type Vec2 = (f64, f64);
type Mat2 = [Vec2; 2];

fn cosd(deg: f64) -> f64 {
    deg.to_radians().cos()
}

fn sind(deg: f64) -> f64 {
    deg.to_radians().sin()
}

fn inverse(m: Mat2) -> Mat2 {
    let (col1, col2) = (m[0], m[1]);
    let inv_det = 1.0 / (col1.0 * col2.1 - col1.1 * col2.0);
    [
        (col2.1 * inv_det, -col1.1 * inv_det),
        (-col2.0 * inv_det, col1.0 * inv_det),
    ]
}

fn make_matrices(points: [Vec2; 3]) -> [Mat2; 3] {
    [
        inverse([points[0], points[1]]),
        inverse([points[1], points[2]]),
        inverse([points[2], points[0]]),
    ]
}

fn mul_coord(m: Mat2, coord: Vec2) -> Vec2 {
    (
        m[0].0 * coord.0 + m[1].0 * coord.1,
        m[0].1 * coord.0 + m[1].1 * coord.1,
    )
}

fn mat_mul(a: Mat2, b: Mat2) -> Mat2 {
    [
        (
            a[0].0 * b[0].0 + a[1].0 * b[0].1,
            a[0].1 * b[0].0 + a[1].1 * b[0].1,
        ),
        (
            a[0].0 * b[1].0 + a[1].0 * b[1].1,
            a[0].1 * b[1].0 + a[1].1 * b[1].1,
        ),
    ]
}

fn rot_scale_rot(b: f64, a: f64, ortho_squish: f64) -> Mat2 {
    [
        (
            -ortho_squish * sind(a) * sind(b) + cosd(a) * cosd(b),
            ortho_squish * cosd(a) * sind(b) + sind(a) * cosd(b),
        ),
        (
            -ortho_squish * sind(a) * cosd(b) - cosd(a) * sind(b),
            ortho_squish * cosd(a) * cosd(b) - sind(a) * sind(b),
        ),
    ]
}

struct PerspectiveCtx {
    by: f64,
    distance: f64,
    slope: f64,
    ortho_squish: f64,
    backside_spacing: f64,
    normal_matrices: [Mat2; 3],
    back_matrices: [Mat2; 3],
}

fn perspective_adjust(coord: Vec2, back: bool, ctx: &PerspectiveCtx) -> Vec2 {
    let mut depth = ctx.distance + if back { ctx.slope * 3.0 } else { 0.0 };
    let matrices = if back { &ctx.back_matrices } else { &ctx.normal_matrices };
    for matrix in matrices {
        let contributions = mul_coord(*matrix, coord);
        if contributions.0 >= 0.0 && contributions.1 >= 0.0 {
            let amt = (contributions.0 + contributions.1) * ctx.slope
                / (ctx.by * 2.0_f64.sqrt() * ctx.ortho_squish + if back { ctx.backside_spacing } else { 0.0 });
            if back {
                depth -= amt;
            } else {
                depth += amt;
            }
            break;
        }
    }
    let shrinkage = (ctx.slope + ctx.distance) / depth;
    (coord.0 * shrinkage, coord.1 * shrinkage)
}

fn maybe_back_spacing(coord: Vec2, angle: f64, back: bool, backside_spacing: f64) -> Vec2 {
    if back {
        (
            coord.0 + backside_spacing * cosd(angle),
            coord.1 + backside_spacing * sind(angle),
        )
    } else {
        coord
    }
}

struct FaceletCoords {
    c1: Vec2,
    c1b: Vec2,
    c1a: Vec2,
    c2: Vec2,
    c2b: Vec2,
    c2a: Vec2,
    c3: Vec2,
    c3b: Vec2,
    c3a: Vec2,
    c4: Vec2,
    c4b: Vec2,
    c4a: Vec2,
    center: Vec2,
}

fn facelet_coords(x0: f64, y0: f64, x1: f64, y1: f64, cx: f64, cy: f64, radius: f64) -> FaceletCoords {
    FaceletCoords {
        c1: (x0, y0),
        c1b: (x0 + radius, y0),
        c1a: (x0, y0 + radius),
        c2: (x0, y1),
        c2b: (x0, y1 - radius),
        c2a: (x0 + radius, y1),
        c3: (x1, y1),
        c3b: (x1 - radius, y1),
        c3a: (x1, y1 - radius),
        c4: (x1, y0),
        c4b: (x1, y0 + radius),
        c4a: (x1 - radius, y0),
        center: (cx, cy),
    }
}

fn transform_coords(
    transform: Transform,
    params: &Params,
    coords: &FaceletCoords,
    ctx: &PerspectiveCtx,
) -> FaceletCoords {
    let t = |c: Vec2| {
        let c = (c.0 - transform.center.0, c.1 - transform.center.1);
        let adjusted = perspective_adjust(mul_coord(transform.matrix, c), params.back, ctx);
        let spaced = maybe_back_spacing(adjusted, transform.back_offset_angle, params.back, ctx.backside_spacing);
        (params.offset.0 + params.scale_amt * spaced.0, params.offset.1 + params.scale_amt * spaced.1)
    };
    FaceletCoords {
        c1: t(coords.c1),
        c1b: t(coords.c1b),
        c1a: t(coords.c1a),
        c2: t(coords.c2),
        c2b: t(coords.c2b),
        c2a: t(coords.c2a),
        c3: t(coords.c3),
        c3b: t(coords.c3b),
        c3a: t(coords.c3a),
        c4: t(coords.c4),
        c4b: t(coords.c4b),
        c4a: t(coords.c4a),
        center: t(coords.center),
    }
}

fn rounded_rect_curves(c: &FaceletCoords) -> Vec<Curve> {
    vec![
        Curve::Line { start: c.c1a, end: c.c2b },
        Curve::Bezier { start: c.c2b, control: c.c2, end: c.c2a },
        Curve::Line { start: c.c2a, end: c.c3b },
        Curve::Bezier { start: c.c3b, control: c.c3, end: c.c3a },
        Curve::Line { start: c.c3a, end: c.c4b },
        Curve::Bezier { start: c.c4b, control: c.c4, end: c.c4a },
        Curve::Line { start: c.c4a, end: c.c1b },
        Curve::Bezier { start: c.c1b, control: c.c1, end: c.c1a },
    ]
}

#[derive(Clone, Copy)]
struct Transform {
    matrix: Mat2,
    center: Vec2,
    name: &'static str,
    back_offset_angle: f64,
}

#[wasm_func]
fn cube(input: &[u8]) -> Result<Vec<u8>, String> {
    let params: Params = ciborium::de::from_reader(input).map_err(|e| e.to_string())?;

    let mut actions = Vec::<Action>::new();

    if !params.faces.is_empty() {
        let ortho_squish = 1.0 / (2.0 * cosd(30.0));

        let faces: Vec<Vec<char>> = params
            .faces
            .split(' ')
            .map(|v| v.chars().collect())
            .collect();

        let by = (faces[0].len() as f64).sqrt().floor() as i32;
        let by_f = by as f64;

        let normal_matrices = make_matrices([
            (cosd(30.0), sind(30.0)),
            (cosd(150.0), sind(150.0)),
            (cosd(270.0), sind(270.0)),
        ]);
        let back_matrices = make_matrices([
            (cosd(90.0), sind(90.0)),
            (cosd(210.0), sind(210.0)),
            (cosd(330.0), sind(330.0)),
        ]);
        let slope = 1.0 / 3.0_f64.sqrt();
        let backside_spacing = 0.3;

        let ctx = PerspectiveCtx {
            by: by_f,
            distance: params.distance,
            slope,
            ortho_squish,
            backside_spacing,
            normal_matrices,
            back_matrices,
        };

        let extra_angle = if params.back { 60.0 } else { 0.0 };

        let back_matrix: Mat2 = [
            (-cosd(60.0), sind(60.0)),
            (sind(60.0), cosd(60.0)),
        ];

        let forward_transforms = [
            Transform {
                matrix: [
                    (cosd(135.0), sind(135.0) * ortho_squish),
                    (-sind(135.0), cosd(135.0) * ortho_squish),
                ],
                center: (0.0, by_f),
                name: "U",
                back_offset_angle: 30.0,
            },
            Transform {
                matrix: rot_scale_rot(45.0, 120.0, ortho_squish),
                center: (0.0, 0.0),
                name: "F",
                back_offset_angle: 270.0,
            },
            Transform {
                matrix: rot_scale_rot(135.0, 60.0, ortho_squish),
                center: (by_f, 0.0),
                name: "R",
                back_offset_angle: 150.0,
            },
        ];
        let back_transforms = [
            Transform {
                matrix: mat_mul(back_matrix, [
                    (cosd(135.0), sind(135.0) * ortho_squish),
                    (-sind(135.0), cosd(135.0) * ortho_squish),
                ]),
                center: (0.0, by_f),
                name: "B",
                back_offset_angle: 30.0,
            },
            Transform {
                matrix: mat_mul(back_matrix, rot_scale_rot(45.0, 120.0, ortho_squish)),
                center: (0.0, 0.0),
                name: "D",
                back_offset_angle: 270.0,
            },
            Transform {
                matrix: mat_mul(back_matrix, rot_scale_rot(135.0, 60.0, ortho_squish)),
                center: (by_f, 0.0),
                name: "L",
                back_offset_angle: 150.0,
            },
        ];

        let transforms = if params.back { &back_transforms } else { &forward_transforms };
        let radius = 0.2;

        actions.push(
            Action::Anchor { name: "center".into(), spot: params.offset });
        actions.push(Action::Anchor { name: "ufr".into(), spot: params.offset });

        for (facelets, transform) in faces.iter().zip(transforms.iter()) {
            for i in 0..by {
                for j in 0..by {
                    let idx = (by - 1) - i + j * by;
                    let color_char = facelets[idx as usize];
                    let coords_before = facelet_coords(
                        i as f64 + 0.03,
                        j as f64 + 0.03,
                        i as f64 + 0.97,
                        j as f64 + 0.97,
                        i as f64 + 0.5,
                        j as f64 + 0.5,
                        radius,
                    );
                    let coords = transform_coords(
                        *transform,
                        &params,
                        &coords_before,
                        &ctx,
                    );
                    actions.push(Action::MergePath {
                        color: Color::Sticker(color_char),
                        curves: rounded_rect_curves(&coords),
                    });
                    actions.push(Action::Anchor {
                        name: format!("{}{}", transform.name, idx),
                        spot: coords.center,
                    });
                }
            }
        }

        if params.back {
            for i in 0..3_usize {
                let dist = 0.06;
                let coords_before = facelet_coords(
                    dist,
                    dist,
                    by_f - dist,
                    by_f - dist,
                    0.5,
                    0.5,
                    radius,
                );
                let coords = transform_coords(
                    Transform {
                        matrix: forward_transforms[i].matrix,
                        center: transforms[i].center,
                        name: forward_transforms[i].name,
                        back_offset_angle: 30.0,
                    },
                    &Params { back: false, ..params.clone() },
                    &coords_before,
                    &ctx,
                );
                actions.push(Action::MergePath {
                    color: Color::Shadow,
                    curves: rounded_rect_curves(&coords),
                });
            }
        }

        for n in 0..(by - 1) {
            for &(angle, front_name, back_name, corner_name, back_corner_name) in &[
                (30.0_f64, "ur", "bl", "ubr", "ubl"),
                (150.0_f64, "uf", "dl", "ufl", "dfl"),
                (270.0_f64, "fr", "db", "dfr", "dbr"),
            ] {
                let coords = perspective_adjust(
                    (cosd(angle + extra_angle), sind(angle + extra_angle)),
                    params.back,
                    &ctx,
                );
                let coords = (
                    params.offset.0 + params.scale_amt * coords.0,
                    params.offset.1 + params.scale_amt * coords.1,
                );
                let name = if n == by - 2 {
                    (if params.back { back_corner_name } else { corner_name }).to_string()
                } else {
                    format!(
                        "{}{}",
                        if params.back { back_name } else { front_name },
                        if by > 3 { n.to_string() } else { String::new() },
                    )
                };
                actions.push(Action::Anchor { name, spot: coords });
            }
        }
    }

    let mut out = Vec::new();
    ciborium::ser::into_writer(&actions, &mut out).map_err(|e| e.to_string())?;

    Ok(out)
}

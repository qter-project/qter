#import "@preview/cetz:0.4.2"

#let colors = (
    "r": (rgb("#d86f9a"), none), ///red,
    "o": (rgb("#e4b37f"), none), ///orange,
    "w": (rgb("#ffffff"), (paint: gray, thickness: 1pt)), ///white,
    "y": (rgb("#e1e485"), none), ///yellow,
    "b": (rgb("#8498f0"), none), ///blue,
    "g": (rgb("#2cda9d"), none), ///green,
    "n": (rgb("#dddddd"), none), ///gray,
)

#let cube(faces, offset: (0, 0), scale-amt: 1, distance: 5, back: false, name: "") = {
    import cetz.draw: *

    if faces == "" {
        return
    }

    let ortho-squish = 1 / (2 * calc.cos(30deg))

    let faces = faces.split(" ").map(v => v.split("").filter(n => colors.keys().contains(n)).map(n => colors.at(n)))

    let by = calc.floor(calc.sqrt(faces.at(0).len()))

    let extra-angle = if back { 60deg } else { 0deg }

    let inverse((col1, col2)) = {
        let inv-det = 1 / (col1.at(0) * col2.at(1) - col1.at(1) * col2.at(0))

        ((col2.at(1) * inv-det, -col1.at(1) * inv-det), (-col2.at(0) * inv-det, col1.at(0) * inv-det))
    }

    let make-matrices(points) = (
        (points.at(0), points.at(1)),
        (points.at(1), points.at(2)),
        (points.at(2), points.at(0)),
    ).map(inverse)

    // These form the front three edges of the cube and have slope 1/sqrt(3) away from the viewer. By expressing a coordinate in terms of these vectors, we can calculate the Z depth.
    let normal-matrices = make-matrices((
        (calc.cos(30deg), calc.sin(30deg)),
        (calc.cos(30deg + 120deg), calc.sin(30deg + 120deg)),
        (calc.cos(30deg + 240deg), calc.sin(30deg + 240deg)),
    ))
    let back-matrices = make-matrices((
        (calc.cos(30deg + 60deg), calc.sin(30deg + 60deg)),
        (calc.cos(30deg + 120deg + 60deg), calc.sin(30deg + 120deg + 60deg)),
        (calc.cos(30deg + 240deg + 60deg), calc.sin(30deg + 240deg + 60deg)),
    ))
    let slope = 1 / calc.sqrt(3)
    // We would like the three corners on the edge of the outline that are nearest to the camera not to scale, so we calculate the neutral depth to make that happen
    let neutral-depth = slope + distance

    // To express the coordinates in terms of those vectors, we can construct inverse matrices to find out how much of each vector contributes to the final thingy

    let mul-coord(matrix, coord) = (
        matrix.at(0).at(0) * coord.at(0) + matrix.at(1).at(0) * coord.at(1),
        matrix.at(0).at(1) * coord.at(0) + matrix.at(1).at(1) * coord.at(1),
    )

    let backside-spacing = 0.3

    let perspective-adjust(coord, back) = {
        let depth = distance + if back { slope * 3 } else { 0 }

        // Express the coordinates in terms of two of the vectors
        for matrix in if back { back-matrices } else { normal-matrices } {
            let contributions = mul-coord(matrix, coord)
            // We need both coordinates to be positive; otherwise we're in the wrong sector
            if contributions.at(0) >= 0 and contributions.at(1) >= 0 {
                let amt = (
                    (contributions.at(0) + contributions.at(1))
                        * slope
                        / (by * calc.sqrt(2) * ortho-squish + if back { backside-spacing } else { 0 })
                )
                if back {
                    depth -= amt
                } else {
                    depth += amt
                }

                break
            }
        }

        // Now we have the Z depth; lets calculate the perspective shrinkage...

        let shrinkage = neutral-depth / depth

        (coord.at(0) * shrinkage, coord.at(1) * shrinkage)
    }

    group(name: name, {
        translate(offset)
        scale(scale-amt)

        anchor("center", (0, 0))
        anchor("ufr", (0, 0))

        let back-matrix = (
            (-calc.cos(60deg), calc.sin(60deg)),
            (calc.sin(60deg), calc.cos(60deg)),
        )

        let mat-mul(a, b) = (
            (
                (a.at(0).at(0) * b.at(0).at(0) + a.at(1).at(0) * b.at(0).at(1)),
                (a.at(0).at(1) * b.at(0).at(0) + a.at(1).at(1) * b.at(0).at(1)),
            ),
            (
                (a.at(0).at(0) * b.at(1).at(0) + a.at(1).at(0) * b.at(1).at(1)),
                (a.at(0).at(1) * b.at(1).at(0) + a.at(1).at(1) * b.at(1).at(1)),
            ),
        )

        let rot-scale-rot(b, a) = (
            (
                -ortho-squish * calc.sin(a) * calc.sin(b) + calc.cos(a) * calc.cos(b),
                ortho-squish * calc.cos(a) * calc.sin(b) + calc.sin(a) * calc.cos(b),
            ),
            (
                -ortho-squish * calc.sin(a) * calc.cos(b) - calc.cos(a) * calc.sin(b),
                ortho-squish * calc.cos(a) * calc.cos(b) - calc.sin(a) * calc.sin(b),
            ),
        )

        let forward-transforms = (
            (
                (
                    (calc.cos(135deg), calc.sin(135deg) * ortho-squish),
                    (-calc.sin(135deg), calc.cos(135deg) * ortho-squish),
                ),
                (0, by),
                "U",
                30deg,
            ),
            (rot-scale-rot(45deg, 120deg), (0, 0), "F", 270deg),
            (rot-scale-rot(135deg, 60deg), (by, 0), "R", 150deg),
        )

        let back-transforms = (
            (
                mat-mul(back-matrix, (
                    (calc.cos(135deg), calc.sin(135deg) * ortho-squish),
                    (-calc.sin(135deg), calc.cos(135deg) * ortho-squish),
                )),
                (0, by),
                "B",
                30deg,
            ),
            (mat-mul(back-matrix, rot-scale-rot(45deg, 120deg)), (0, 0), "D", 270deg),
            (mat-mul(back-matrix, rot-scale-rot(135deg, 60deg)), (by, 0), "L", 150deg),
        )

        let transforms = if back { back-transforms } else { forward-transforms }

        let maybe-back-spacing(coord, angle, back) = if back {
            (coord.at(0) + backside-spacing * calc.cos(angle), coord.at(1) + backside-spacing * calc.sin(angle))
        } else { coord }

        let transform-coords(matrix, center, back-angle, back, coords-before) = {
            let coords = (:)

            for (name, coord) in coords-before {
                let coord = (coord.at(0) - center.at(0), coord.at(1) - center.at(1))
                let ret = (:)
                coords.insert(name, maybe-back-spacing(
                    perspective-adjust(mul-coord(matrix, coord), back),
                    back-angle,
                    back,
                ))
            }

            coords
        }

        let radius = 0.2

        for (x, (facelets, (matrix, center, name, back-offset-angle))) in faces.zip(transforms).enumerate() {
            for i in range(0, by) {
                for j in range(0, by) {
                    let idx = (by - 1) - i + j * by
                    let (fill, stroke) = facelets.at(idx)
                    let coords = transform-coords(matrix, center, back-offset-angle, back, (
                        c1: (i + 0.03, j + 0.03),
                        c1b: (i + 0.03 + radius, j + 0.03),
                        c1a: (i + 0.03, j + 0.03 + radius),
                        c2: (i + 0.03, j + 0.97),
                        c2b: (i + 0.03, j + 0.97 - radius),
                        c2a: (i + 0.03 + radius, j + 0.97),
                        c3: (i + 0.97, j + 0.97),
                        c3b: (i + 0.97 - radius, j + 0.97),
                        c3a: (i + 0.97, j + 0.97 - radius),
                        c4: (i + 0.97, j + 0.03),
                        c4b: (i + 0.97, j + 0.03 + radius),
                        c4a: (i + 0.97 - radius, j + 0.03),
                        center: (i + 0.5, j + 0.5),
                    ))

                    merge-path(fill: fill, stroke: stroke, {
                        line(coords.c1a, coords.c2b)
                        bezier(coords.c2b, coords.c2a, coords.c2)
                        line(coords.c2a, coords.c3b)
                        bezier(coords.c3b, coords.c3a, coords.c3)
                        line(coords.c3a, coords.c4b)
                        bezier(coords.c4b, coords.c4a, coords.c4)
                        line(coords.c4a, coords.c1b)
                        bezier(coords.c1b, coords.c1a, coords.c1)
                    })

                    anchor(name + str(idx), coords.center)
                }
            }
        }

        if back {
            for i in range(0, 3) {
                let dist = 0.06
                let coords = transform-coords(forward-transforms.at(i).at(0), transforms.at(i).at(1), 30deg, false, (
                    c1: (dist, dist),
                    c1b: (dist + radius, dist),
                    c1a: (dist, dist + radius),
                    c2: (dist, by - dist),
                    c2b: (dist, by - dist - radius),
                    c2a: (dist + radius, by - dist),
                    c3: (by - dist, by - dist),
                    c3b: (by - dist - radius, by - dist),
                    c3a: (by - dist, by - dist - radius),
                    c4: (by - dist, dist),
                    c4b: (by - dist, dist + radius),
                    c4a: (by - dist - radius, dist),
                    center: (0.5, 0.5),
                ))

                merge-path(
                    fill: black.transparentize(92%),
                    stroke: (paint: black.transparentize(93%), thickness: 0.5pt),
                    {
                        line(coords.c1a, coords.c2b)
                        bezier(coords.c2b, coords.c2a, coords.c2)
                        line(coords.c2a, coords.c3b)
                        bezier(coords.c3b, coords.c3a, coords.c3)
                        line(coords.c3a, coords.c4b)
                        bezier(coords.c4b, coords.c4a, coords.c4)
                        line(coords.c4a, coords.c1b)
                        bezier(coords.c1b, coords.c1a, coords.c1)
                    },
                )
            }
        }

        for n in range(0, by - 1) {
            let dist = 1.2 + n

            for (angle, front-name, back-name, corner-name, back-corner-name) in (
                (30deg, "ur", "bl", "ubr", "ubl"),
                (30deg + 120deg, "uf", "dl", "ufl", "dfl"),
                (30deg + 240deg, "fr", "db", "dfr", "dbr"),
            ) {
                let coords = perspective-adjust((calc.cos(angle + extra-angle), calc.sin(angle + extra-angle)), back)
                let name = if n == by - 2 {
                    if back { back-corner-name } else { corner-name }
                } else {
                    if back { back-name } else { front-name } + if by > 3 { str(n) } else { "" }
                }
                anchor(name, coords)
            }
        }
    })
}


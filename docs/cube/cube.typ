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

#let p = plugin("./cube-module/cube_module.wasm")

#let cube(faces, offset: (0, 0), scale-amt: 1, distance: 5, back: false, name: "") = {
    import cetz.draw : *

    let actions = cbor(p.cube(cbor.encode((
        faces: faces,
        offset: (float(offset.at(0)), float(offset.at(1))),
        scale_amt: float(scale-amt),
        distance: float(distance),
        back: back
    ))))

    let resolve-color(color) = {
        if type(color) == str and color == "Shadow" {
            (black.transparentize(92%), (paint: black.transparentize(93%), thickness: 0.5pt))
        } else if "Sticker" in color {
            colors.at(color.at("Sticker"))
        } else {
            panic("invalid data from wasm")
        }
    }

    let render-curves(curves) = {
        for curve in curves {
            if "Line" in curve {
                let c = curve.at("Line")
                line(c.at("start"), c.at("end"))
            } else if "Bezier" in curve {
                let c = curve.at("Bezier")
                bezier(c.at("start"), c.at("end"), c.at("control"))
            } else {
                panic("invalid data from wasm")
            }
        }
    }

    let render(actions) = {
        for action in actions {
            if "Anchor" in action {
                let a = action.at("Anchor")
                anchor(a.at("name"), a.at("spot"))
            } else if "MergePath" in action {
                let m = action.at("MergePath")
                let (fill, stroke) = resolve-color(m.at("color"))
                merge-path(fill: fill, stroke: stroke, render-curves(m.at("curves")))
            } else {
                panic("invalid data from wasm")
            }
        }
    }

    cetz.draw.group(name: name, {
        render(actions)
    })
}

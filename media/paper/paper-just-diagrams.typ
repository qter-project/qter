#import "@preview/cetz:0.4.2"
#import "@preview/fletcher:0.5.8" as fletcher: diagram, edge, node, shapes
#import "../../docs/cube/cube.typ": *

#set page(width: 8.5in, height: 11in)

#let thing(body, sc: 100%, r: false) = rotate(
    if not r { 90deg } else { 0deg },
    reflow: true,
    box(width: 100%, height: 100%, align(center + horizon, scale(sc, body))),
)

/*
#thing(
    sc: 150%,
    grid(
        columns: 3,
        column-gutter: 4em,
        row-gutter: 1em,
        align(center, [#set text(1.5em); Corners]),
        align(center, [#set text(1.5em); Edges]),
        align(center, [#set text(1.5em); Centers]),

        cetz.canvas(length: 25pt, {
            cube("wnwnnnwnw gngnnngng rnrnnnrnr")
        }),
        cetz.canvas(length: 25pt, {
            cube("nwnwnwnwn ngngngngn nrnrnrnrn")
        }),
        cetz.canvas(length: 25pt, {
            cube("nnnnwnnnn nnnngnnnn nnnnrnnnn")
        }),

        image("corner.jpg", height: 10em, width: 10em),
        image("edge.jpg", height: 10em, width: 10em),
        image("core.jpg", height: 10em, width: 10em),
    ),
)

#pagebreak()

#thing(sc: 150%, cetz.canvas(length: 15pt, {
    import cetz.draw: *

    content((-14, 0), [#set text(2em); #sym.circle.dotted], anchor: "west")

    content((-9.9, 3.1), [#set text(1.5em); U])
    cube("wwwwwwwww rrrgggggg bbbrrrrrr", offset: (-9.9, 0))
    content((-4.9, 3.1), [#set text(1.5em); D])
    cube("wwwwwwwww ggggggooo rrrrrrggg", offset: (-4.9, 0))
    content((0.1, 3.1), [#set text(1.5em); R])
    cube("wwgwwgwwg ggyggyggy rrrrrrrrr", offset: (0.1, 0))
    content((5.1, 3.1), [#set text(1.5em); L])
    cube("bwwbwwbww wggwggwgg rrrrrrrrr", offset: (5.1, 0))
    content((10.1, 3.1), [#set text(1.5em); F])
    cube("wwwwwwooo ggggggggg wrrwrrwrr", offset: (10.1, 0))
    content((15.1, 3.1), [#set text(1.5em); B])
    cube("rrrwwwwww ggggggggg rryrryrry", offset: (15.1, 0))

    content((-14, -5.5), [#set text(2em); #sym.circle.dotted;2], anchor: "west")

    cube("wwwwwwwww bbbgggggg ooorrrrrr", offset: (-9.9, -5.5))
    cube("wwwwwwwww ggggggbbb rrrrrrooo", offset: (-4.9, -5.5))
    cube("wwywwywwy ggbggbggb rrrrrrrrr", offset: (0.1, -5.5))
    cube("ywwywwyww bggbggbgg rrrrrrrrr", offset: (5.1, -5.5))
    cube("wwwwwwyyy ggggggggg orrorrorr", offset: (10.1, -5.5))
    cube("yyywwwwww ggggggggg rrorrorro", offset: (15.1, -5.5))

    content((-14, -11), [#set text(2em); #sym.circle.dotted;#sym.quote.single], anchor: "west")

    cube("wwwwwwwww ooogggggg gggrrrrrr", offset: (-9.9, -11))
    cube("wwwwwwwww ggggggrrr rrrrrrbbb", offset: (-4.9, -11))
    cube("wwbwwbwwb ggwggwggw rrrrrrrrr", offset: (0.1, -11))
    cube("gwwgwwgww yggyggygg rrrrrrrrr", offset: (5.1, -11))
    cube("wwwwwwrrr ggggggggg yrryrryrr", offset: (10.1, -11))
    cube("ooowwwwww ggggggggg rrwrrwrrw", offset: (15.1, -11))
}))

#pagebreak()

#thing()[
    #image("Light U States.png")

    #v(2em)

    #scale(150%, cetz.canvas(length: 15pt, {
        import cetz.draw: *

        content((-9.9, 3.1), [#set text(1.5em); "Zero"])
        cube("wwwwwwwww ggggggggg rrrrrrrrr", offset: (-9.9, 0))
        content((-4.9, 3.1), [#set text(1.5em); "One"])
        cube("wwwwwwggg rrrggyggy wbbrrrrrr", offset: (-4.9, 0))
        content((0.1, 3.1), [#set text(1.5em); "Two"])
        cube("gwwgwwyyr rrwggbggb goorrbrrb", offset: (0.1, 0))
        content((4.9, 3.1), [#set text(1.5em); ...])
        content((10.1, 3.1), [#set text(1.5em); "104"])
        cube("wwbwwbwwr oowggwggw grrgrrgrr", offset: (10.1, 0))
        content((15.1, 3.1), [#set text(1.5em); "105"])
        cube("wwwwwwwww ggggggggg rrrrrrrrr", offset: (15.1, 0))
    }))
]

#pagebreak()

#thing(
    sc: 120%,
    grid(
        columns: 2,
        column-gutter: 7em,
        grid(
            columns: 1,
            row-gutter: 3em,
            cetz.canvas(length: 15pt, {
                import cetz.draw: *

                content((-9.9, 3.1), [#set text(1.5em); (R U) × ???])
                cube("oybbwgywr grwggwggb grobrgyob", offset: (-9.9, 0))
                content((-2.9, 3.1), [#set text(1.5em); (R U) × #underline()[0]])
                cube("wwwwwwwww ggggggggg rrrrrrrrr", offset: (-2.9, 0))
            }),

            cetz.canvas(length: 15pt, {
                import cetz.draw: *

                content((-9.9, 3.1), [#set text(1.5em); (0, ?)])
                cube("wwwwwwwww ggggggrrr rrrrrrbbb", offset: (-9.9, 0))
                content((-4.9, 3.1), [#set text(1.5em); (?, 0)])
                cube("wwwwwwwww bbbgggggg ooorrrrrr", offset: (-4.9, 0))
            }),

            scale(30%, reflow: true, image("Goto States.png")),
        ),

        grid(
            columns: 1,
            row-gutter: 3em,
            cetz.canvas(length: 15pt, {
                import cetz.draw: *

                content(((-9.9 + -4.9) / 2, 3.1), [#set text(1.2em); R' F' L U' L U L F U' R #h(0.4em) (1, 0)])
                cube("obwywwwgw bwggggggg rrbrrbrrr", offset: (-9.9, 0))
                cube("orgobwbbo yybyywyyy yowboooor", offset: (-4.9, 0), back: true)

                content(((-9.9 + -4.9) / 2, 3.1 - 7), [#set text(1.2em); U F R' D' R2 F R' U' D #h(0.4em) (0, 1)])
                cube("wwwwwywwg ggrwgyggb wgrbrryyy", offset: (-9.9, -7))
                cube("bbbbbbooy ggoryyrrr boooooyro", offset: (-4.9, -7), back: true)
            }),

            cetz.canvas(length: 15pt, {
                import cetz.draw: *

                content((-2.5, 3.1), [#set text(1.5em); R U])
                cube("wwwwwwggg rrrggyggy wbbrrrrrr", offset: (-2.5, 0), name: "1x")
                content((2.5, 3.1), [#set text(1.5em); $("R U")^3$])
                cube("yggywwbbw rrgggwggo rgyrrobbo", offset: (2.5, 0), name: "3x")

                circle("1x.center", radius: 1)
                circle("3x.center", radius: 1)
            }),
        ),
    ),
)

#pagebreak()

#image("Light Compilation Pipeline.png")

#pagebreak()

#thing(
    sc: 150%,
    grid(
        columns: 2,
        column-gutter: 2em,
        [
            ```l
            Puzzles
            📼 A: 3x3

            -----

            .registers {
                📼 X ~ A, B ← 3x3 builtin (90, 90)
            }

            add X.A 1

            move-right X 1
            print "A is" X.A

            move-left X 1
            print "A is" X.A
            ```
        ],
        [
            ```janet
            .macro deref {
                ($tape:tape $current:reg $to:reg) => {
                    // Move the head to the zero position
                    while not-solved $current {
                        dec $current
                        move-left $tape
                    }

                    // Move the head to `to`
                    while not-solved $to {
                        dec $to
                        inc $current
                        move-right $tape
                    }
                }
            }
            ```
        ],
    ),
)

#pagebreak()

#thing(sc: 150%, grid(
    columns: 2,
    column-gutter: 2em,
    [
        $
            & 0 #h(1em) && 1 #h(1em) && 2 #h(1em) && 3 #h(1em) && 4 \
            & ↓         && ↓         && ↓         && ↓         && ↓ \
            & 2         && 1         && 4         && 3         && 0 \
        $

        #v(1em)

        #let y(x) = text(fill: red, $#x$)

        $
                  a & =    && 2,         && 1,         && 4,         && 3,         && 0 \
                  b & =    && #y(4),     && #y(3),     && #y(0),     && #y(2),     && #y(1) \
                    &      && arrow.b    && arrow.b    && arrow.b    && arrow.b    && arrow.b \
            a dot b & = a( && #y(4)), a( && #y(3)), a( && #y(0)), a( && #y(2)), a( && #y(1)) \
                    & =    && 0,         && 3,         && 2,         && 4,         && 1 \
        $

        #v(1em)

        $
            &0 #h(1em) && 1 #h(1em) && 2 #h(1em) && 3 #h(1em) && 4 &&&&0 #h(1em) && 1 #h(1em) && 2 #h(1em) && 3 #h(1em) && 4 \
            a^(-1) = #h(0.5em) &↑ && ↑ && ↑ && ↑ && ↑ #h(1em) && → #h(1em) && ↓ && ↓ && ↓ && ↓ && ↓ \
            &2 && 1 && 4 && 3 && 0 &&&& 4 && 1 && 0 && 3 && 2 \
        $

        #v(1em)

        $
            (0, 2, 4)(1)(3)
        $

        #v(1em)

        $
            (0, 1, 2)(3, 4, 5, 6) -> lcm(3, 4) = 12
        $
    ],
    [
        #figure(scale(26%, reflow: true, image("Stickered Cube.png")))

        #[
            #set text(0.6em)

            $
                U & = ( 1, 3, 8, 6)( 2, 5, 7, 4)( 9,33,25,17)(10,34,26,18)(11,35,27,19) \
                D & = (41,43,48,46)(42,45,47,44)(14,22,30,38)(15,23,31,39)(16,24,32,40) \
                R & = (25,27,32,30)(26,29,31,28)( 3,38,43,19)( 5,36,45,21)( 8,33,48,24) \
                L & = ( 9,11,16,14)(10,13,15,12)( 1,17,41,40)( 4,20,44,37)( 6,22,46,35) \
                F & = (17,19,24,22)(18,21,23,20)( 6,25,43,16)( 7,28,42,13)( 8,30,41,11) \
                B & = (33,35,40,38)(34,37,39,36)( 3, 9,46,32)( 2,12,47,29)( 1,14,48,27) \
            $
        ]

        #figure(cetz.canvas(length: 15pt, {
            import cetz.draw: *

            content((-9.9, 3.1), [#set text(1.5em); ()])
            cube("roronoror roronoror roronoror", offset: (-9.9, 0))
            line((-7.5, 0), (-5.3, 0), mark: (end: "straight"))
            content((-2.9, 3.1), [#set text(1.5em); R])
            cube("roronoror roronoror roronoror", offset: (-2.9, 0))
        }))
    ],
))

#pagebreak()

#thing(sc: 150%)[
    #figure(cetz.canvas(length: 15pt, {
        import cetz.draw: *

        content((-9.9, 3.1), [#set text(1.5em); ()])
        cube("wwwwwwwww ggggggggg rrrrrrrrr", offset: (-9.9, 0))
        content((-3.9, 3.1), [#set text(1.5em); R U])
        cube("wwwwwwggg rrrggyggy wbbrrrrrr", offset: (-3.9, 0), name: "ufr")
        content((2.1, 3.1), [#set text(1.5em); R U F])
        cube("wwwwwwooy ggrggryyr gbbgrrgrr", offset: (2.1, 0), name: "fr")

        circle("ufr.center", radius: 1)
        circle("fr.fr", radius: 1)
    }))

    #figure(cetz.canvas(length: 15pt, {
        import cetz.draw: *

        cube("wwwwwwwww ggggggggg rrrrrrrrr", offset: (-2.5, 0))
        cube("bbbbbbbbb yyyyyyyyy ooooooooo", offset: (2.5, 0), back: true)

        line((5, 0), (8, 0), mark: (end: "straight"))

        translate((13, 0))

        cube("bbbbbbbbb nnnbbbnnn nnnnnnnnn", offset: (-2.5, 0))
        cube("nnnbbbnnn bbbbbbbbb nnnnnnnnn", offset: (2.5, 0), back: true)
    }))

    $
        ⟨U, D, R 2, F 2, L 2, B 2⟩ \
        ⟨U, D, R, F 2, L, B 2⟩ \
    $

    #figure(cetz.canvas(length: 15pt, {
        import cetz.draw: *

        cube("owoywoowb bgybgwybg oggorgwrw", offset: (-2.3, 0))
        cube("wbyrbybwr rryyyyrog bbgoogrrw", offset: (2.3, 0), back: true)

        line((5, 0), (8, 0), mark: (end: "straight"))

        cube("nbnbbnnbn nnbbbbbbn nbnnnbbnb", offset: (-2.3 + 13, 0))
        cube("bnbnbbnbn nnbbbbnnn nnnnnnnnb", offset: (2.3 + 13, 0), back: true)
    }))
]

#pagebreak()

#thing(r: true, sc: 150%)[
    #grid(
        columns: 2,
        column-gutter: 4em,
        {
            cetz.canvas(length: 15pt, {
                import cetz.draw: *

                content((0, 3.1), [#set text(1.5em); R])
                cube("bbnbbbbbn nnbbbbnnb nnnnnnnnn", offset: (-2.5, 0))
                cube("bnnbbbbnn nbbbbbnbb nnnnnnnnn", offset: (2.5, 0), back: true)
            })

            cetz.canvas(length: 15pt, {
                import cetz.draw: *

                content((0, 3.1), [#set text(1.5em); F])
                cube("bbbbbbnnn nbnnbnnbn bnnbnnbnn", offset: (-2.5, 0))
                cube("nnnbbbnnn bbbbbbnnn nnnnnnbbb", offset: (2.5, 0), back: true)
            })

            cetz.canvas(length: 15pt, {
                import cetz.draw: *

                cube("bbbbbbbbn nnnbbbnnn bnnnnnnnn", offset: (-2.5, 0), name: "cl")
                cube("bbbbbbbbn nnbbbbnnn nnnnnnnnn", offset: (2.5, 0), name: "ccl")

                content((-2.5, 3.1), [#set text(2em); $1$])
                content((2.5, 3.1), [#set text(2em); $2$])
                circle("cl.center", radius: 1)
                circle("ccl.center", radius: 1)
            })

            cetz.canvas(length: 15pt, {
                import cetz.draw: *

                content((0, 3.1), [#set text(1.5em); R])
                cube("bbnbbbbbn nnbbbbnnb nnnnnnnnn", offset: (-2.5, 0), name: "f")
                cube("bnnbbbbnn nbbbbbnbb nnnnnnnnn", offset: (2.5, 0), back: true)

                line("f.R0", "f.R2", mark: (end: "straight"), name: "A")
                content((rel: "A.mid", to: (-0.1, 0.1)), anchor: "south-east", stroke: white, [#set text(1.5em); +1])

                line("f.R8", "f.R6", mark: (end: "straight"), name: "A")
                content((rel: "A.mid", to: (0.1, -0.1)), anchor: "north-west", stroke: white, [#set text(1.5em); +1])

                line("f.R2", "f.R8", mark: (end: "straight"), name: "A")
                content((rel: "A.mid", to: (0.1, -0.1)), anchor: "south-west", stroke: white, [#set text(1.5em); +2])

                line("f.R6", "f.R0", mark: (end: "straight"), name: "A")
                content((rel: "A.mid", to: (-0.3, -0.1)), anchor: "east", stroke: white, [#set text(1.5em); +2])
            })
        },
        {
            cetz.canvas(length: 15pt, {
                import cetz.draw: *

                content((0, 3.1), [#set text(1.5em); $emptyset$])
                cube("wwwwwwwwg ggrgggggg wrrrrrrrr", offset: (-2.5, 0))
                cube("bbbbbbbbb yyyyyyyyy ooooooooo", offset: (2.5, 0), back: true)
            })

            cetz.canvas(length: 15pt, {
                import cetz.draw: *

                content((0, 3.1), [#set text(1.5em); $emptyset$])
                cube("wwwwwwwgw gwggggggg rrrrrrrrr", offset: (-2.5, 0))
                cube("bbbbbbbbb yyyyyyyyy ooooooooo", offset: (2.5, 0), back: true)
            })

            $
                (1, 2) · (1, 3) · (1, 4) \
                ↓ \
                (1, 2, 3) · (1, 4) \
                ↓ \
                (1, 2, 3, 4)
            $

            cetz.canvas(length: 15pt, {
                import cetz.draw: *

                content((0, 3.1), [#set text(1.5em); $emptyset$])
                cube("wwwwwwwww grggggggg rgrrrrrrr", offset: (-2.5, 0))
                cube("bbbbbbbbb yyyyyyyyy ooooooooo", offset: (2.5, 0), back: true)
            })
        },
    )
]

#pagebreak()

#thing(sc: 120%, grid(
    columns: 3,
    column-gutter: 5em,
    [
        #figure(cetz.canvas(length: 15pt, {
            import cetz.draw: *

            content((0, 3), [#set text(1.5em); R U])
            cube("wwwwwwggg rrrggyggy wbbrrrrrr", offset: (-2.5, 0))
            cube("ooowbbwbb byybyybyy oogoogooy", offset: (2.5, 0), back: true)
        }))

        #scale(70%, reflow: true)[$
            ("UFR") ("FDR", "UFL", "UBL", "UBR", "DBR") \
            ("FR", "UF", "UL", "UB", "UR", "BR", "DR")
        $]

        #figure(cetz.canvas(length: 15pt, {
            import cetz.draw: *

            content((0, 3), [#set text(1.5em); R U])
            cube("bbbbbbnbn nnnbbbnnb bnnnnnnnn", offset: (-2.5, 0))
            cube("nnnbbbbnn nbbbbbnbb nnnnnnnnb", offset: (2.5, 0), back: true)
        }))

        #scale(70%, reflow: true)[
            $
                  & "+1"     && "+2"   && "+0"   && "+0"   && "+2"   && "+1" \
                ( & "UFR") ( && "FDR", && "UFL", && "UBL", && "UBR", && "DBR") \
            $
            $
                  & "+0"  && "+0"  && "+0"  && "+0"  && "+0"  && "+0"  && "+0" \
                ( & "FR", && "UF", && "UL", && "UB", && "UR", && "BR", && "DR")
            $
        ]
    ],
    [
        #figure(cetz.canvas(length: 15pt, {
            import cetz.draw: *

            content((0, 3), [#set text(1.5em); $("R U")^3$])
            cube("yggywwbbw rrgggwggo rgyrrobbo", offset: (-2.5, 0), name: "f")
            cube("rrbwbbwbb gyywyywyy oorooroow", offset: (2.5, 0), back: true)

            circle("f.center", radius: 1)
        }))

        #figure(cetz.canvas(length: 15pt, {
            import cetz.draw: *

            content((0, 3), [#set text(1.5em); $("R U")^7$])
            cube("rwgwwwrwg bgrgggggr wrwrrrwrw", offset: (-2.5, 0))
            cube("obgbbbobb byyyyybyy ooyoooooy", offset: (2.5, 0), back: true)
        }))

        #figure(cetz.canvas(length: 15pt, {
            import cetz.draw: *

            content((0, 3), [#set text(1.5em); $("R U")^5$])
            cube("obbwwygwr obwggwggr grworrygb", offset: (-2.5, 0))
            cube("rrwgbbybb ryywyygyy oobooroow", offset: (2.5, 0), back: true)
        }))

        #figure(cetz.canvas(length: 15pt, {
            import cetz.draw: *

            content((0, 3), [#set text(1.5em); $("R U")^15$])
            cube("wwwwwwwgw grgggyggg rbrrrrrrr", offset: (-2.5, 0))
            cube("bobwbbbbb yyybyyyyy ooooogooo", offset: (2.5, 0), back: true)
        }))
    ],
    [
        $
            lcm(3, 7, 15) = 105
        $

        #v(5em)

        ```
        .registers {
          A ← 3x3 (R U)
        }

        label:
        solved-goto A label

        -----

        Puzzles
        A: 3x3

        1 | solved-goto FDR FR 1
        ```
    ],
))

#pagebreak()

#thing(sc: 180%)[
    #figure(cetz.canvas(length: 15pt, {
        import cetz.draw: *

        content(((-9.9 - 4.9) / 2, 3.1), [#set text(1.2em); A = R' F' L U' L U L F U' R])
        cube("obwywwwgw bwggggggg rrbrrbrrr", offset: (-9.9, 0))
        cube("orgobwbbo yybyywyyy yowboooor", offset: (-4.9, 0), back: true)
        content(((2.5 + 7.1) / 2, 3.1), [#set text(1.2em); B = U F R' D' R2 F R' U' D])
        cube("wwwwwywwg ggrwgyggb wgrbrryyy", offset: (2.1, 0))
        cube("bbbbbbooy ggoryyrrr boooooyro", offset: (7.1, 0), back: true)
    }))

    $
              & "+2"    && "+1"   && "+1"   && "+0"   && "+0"    && "+0"  && "+1"  && "+0"  && "+0"  && "+0" \
        A = ( & "DBL")( && "UF")( && "UFL", && "UBL", && "UBR")( && "UL", && "LB", && "RB", && "UB", && "LD") \
    $

    $
              & "+1"    && "+1"    && "+1"   && "+1"   && "+2"    && "+1"   && "+0"  && "+0"  && "+0"  && "+1"  && "+0" \
        B = ( & "DBL")( && "UFR")( && "DFR", && "DFL", && "DBR")( && "RD")( && "UR", && "FL", && "DB", && "FR", && "FD")
    $
]
*/

#let cubenode(faces) = {
    cetz.canvas({
        cube(faces, scale-amt: 0.5)
    })
}

#thing(sc: 150%, cetz.canvas({
    import cetz.draw: *

    stroke(2pt + black)
    let data = (
        cubenode("rrrrrrrrr wwwwwwwww ggggggggg"),
        ([#text(size: 33pt)[\...]],),
        ([#cubenode("rrrrrrrrr gggwwwwww yyygggggg")],),
        ([#cubenode("rrrrrrrrr yyywwwwww bbbgggggg")],),
        (
            cubenode("rrrrrrrrr bbbwwwwww wwwgggggg"),
            [
                #text(size: 33pt)[\...]
                #h(10pt)
                $cubenode("rrbrrwrrw bbowwowwo ggwggwggw")$
                #h(10pt)
                $cubenode("rrorrorro bbywwywwg ggggggwww")$
                #h(10pt)
                $cubenode("rrrrrrwgg bwwbwwbww owwoggogg")$
                #h(10pt)
                #text(size: 33pt)[\...]
            ],
        ),
        ([#cubenode("rrwrrwrrw wwowwowwo ggggggggg")],),
        ([#text(size: 33pt)[\...]],),
    )
    cetz.tree.tree(
        data,
        spread: 1.25,
        grow: 1.5,
        direction: "down",
        draw-node: (node, ..) => {
            content((), [#node.content])
        },
        draw-edge: (from, to, ..) => {
            let (a, b) = (from + ".center", to + ".center")
            line((a, 1.3, b), (b, 1.5, a))
        },
        name: "tree",
    )

    line((to: "tree.g0-3", rel: (-0.9, -1)), (to: "tree.0-3-0", rel: (-4.5, 1.2)))
    line((to: "tree.g0-3", rel: (-0.6, -1.15)), (to: "tree.0-3-0", rel: (-2, 1.2)))
    line((to: "tree.g0-3", rel: (0.6, -1.15)), (to: "tree.0-3-0", rel: (2, 1.2)))
    line((to: "tree.g0-3", rel: (0.9, -1)), (to: "tree.0-3-0", rel: (4.5, 1.2)))
}))

#pagebreak()

#thing(sc: 110%, [#grid(
        columns: 2,
        column-gutter: 1em,
        [#grid(
                columns: (1fr, 1fr),
                figure(
                    cetz.canvas(length: 15pt, {
                        import cetz.draw: *

                        cube("wwwwwwoob ggwrggwgg rggrrrrrr", offset: (-2.5, 0))
                        cube("rbbbbbbbb yyyyyyyyg ooooooowy", offset: (2.5, 0), back: true)
                    }),
                    caption: figure.caption(position: top, text(1.2em)[A = F U F' U']),
                    supplement: none,
                ),
                figure(
                    cetz.canvas(length: 15pt, {
                        import cetz.draw: *

                        cube("wwowwgwwg ggyggwggg rrwbrrwrr", offset: (-2.5, 0))
                        cube("brrbbbbbb yyyyyyryy ooboooooo", offset: (2.5, 0), back: true)
                    }),
                    caption: figure.caption(position: top, text(1.2em)[B = R U R' U']),
                    supplement: none,
                ),
            )

            #figure(cetz.canvas(length: 15pt, {
                import cetz.draw: *

                set-style(content: (
                    padding: (0, 0, 7pt, 0),
                ))

                cube("wwwwwwwww ggggggggg rrrrrrrrr", offset: (0, 7.5), name: "one")
                content("one.north", text(1.2em)[#align(center + bottom)[Solved\ (reference frame)]], anchor: "south")

                cube("wwwwwwwww rrrrrrrrr bbbbbbbbb", offset: (-9, 0), name: "two")
                content("two.north", text(1.2em)[#align(center + bottom)[$S$\ Rotate $90degree$]], anchor: "south")
                cube("wwwwwwggo rrwbrrwrr brrbbbbbb", offset: (-3, 0), name: "three")
                content("three.north", text(1.2em)[#align(center + bottom)[$A$\ Apply $A$]], anchor: "south")
                cube("wwowwgwwg ggyggwggg rrwbrrwrr", offset: (3, 0), name: "four")
                content(
                    "four.north",
                    text(1.2em)[#align(center + bottom)[$S^(-1)$\ Rotate $-90degree$]],
                    anchor: "south",
                )
                content("four.east", text(2em)[$=$], anchor: "west", padding: (0, 0, 0, 5pt))
                cube("wwowwgwwg ggyggwggg rrwbrrwrr", offset: (9, 0), name: "five")
                content("five.north", text(1.2em)[#align(center + bottom)[$B$\ Resultant $B$]], anchor: "south")
            }))
        ],
        [
            #figure(
                cetz.canvas(length: 130pt, {
                    import cetz.draw: *

                    ortho(
                        x: 11deg,
                        y: 28deg,
                        {
                            let fillc(p) = gray.transparentize(p)
                            on-xy(z: 0, rect((0, 0), (1, 1), stroke: 1.25pt, fill: fillc(90%)))
                            on-xy(z: 1, rect((0, 0), (1, 1), stroke: 1.25pt, fill: fillc(90%)))
                            on-xz(y: 0, rect((0, 0), (1, 1), stroke: 1.25pt, fill: fillc(60%)))
                            on-xz(y: 1, rect((0, 0), (1, 1), stroke: 1.25pt, fill: fillc(60%)))
                            on-yz(x: 0, rect((0, 0), (1, 1), stroke: 1.25pt, fill: fillc(70%)))
                            on-yz(x: 1, rect((0, 0), (1, 1), stroke: 1.25pt, fill: fillc(80%)))

                            set-style(
                                mark: (
                                    fill: black,
                                    width: 0.1,
                                    length: 0.1,
                                    stroke: (dash: none),
                                ),
                                paint: black,
                            )

                            let x_len = 0.05
                            let x_thickness = 1.5pt

                            line(
                                (-0.13, -0.13, 1.13),
                                (1.3, 1.3, -0.3),
                                stroke: (dash: "dashed"),
                                mark: (end: ">"),
                                name: "first-line",
                            )
                            let c = x_len * calc.sqrt(2.0) / 2
                            line((-c, c, 1 + c), (c, -c, 1 - c), stroke: x_thickness)
                            line((-c, c, 1 - c), (c, -c, 1 + c), stroke: x_thickness)
                            line((1 + c, 1 - c, c), (1 - c, 1 + c, -c), stroke: x_thickness)
                            line((1 + c, 1 - c, -c), (1 - c, 1 + c, c), stroke: x_thickness)
                            content((1.06, 1.31, -0.1), text(size: 13pt)[$3$x])
                            arc((1.28, 1.11, -0.1), start: -33deg, stop: 290deg, radius: (1.5 / 13, 1.2 / 13), mark: (
                                start: ">",
                                scale: 0.4,
                            ))
                            content("first-line.end", move(dx: 5pt, dy: -15pt)[#text(size: 13pt)[$S_(U\R\B3)$]])

                            line(
                                (0.5, -0.3, 0.5),
                                (0.5, 1.61, 0.5),
                                stroke: (dash: "dashed"),
                                mark: (end: ">"),
                                name: "second-line",
                            )
                            let a = 0.5 + x_len
                            let b = 0.5 - x_len
                            line((a, 0, a), (b, 0, b), stroke: x_thickness)
                            line((a, 0, b), (b, 0, a), stroke: x_thickness)
                            line((a, 1, a), (b, 1, b), stroke: x_thickness)
                            line((a, 1, b), (b, 1, a), stroke: x_thickness)
                            content((0.74, 1.46, 0.55), text(size: 13pt)[$4$x])
                            arc((0.63, 1.35, 0.5), start: 10deg, stop: 325deg, radius: (1.7 / 13, 1 / 13), mark: (
                                start: ">",
                                scale: 0.4,
                            ))
                            content("second-line.end", text(size: 13pt)[$S_(\U4)$], padding: (0, 0, 25pt, 0))

                            set-style(stroke: (paint: red), mark: (fill: red))
                            line(
                                (1.15, -0.15, 0.5),
                                (-0.4, 1.4, 0.5),
                                stroke: (dash: "dashed"),
                                mark: (end: ">"),
                                name: "third-line",
                            )
                            line((1 - c, -c, 0.5 + c), (1 + c, c, 0.5 - c), stroke: (thickness: x_thickness))
                            line((1 - c, -c, 0.5 - c), (1 + c, c, 0.5 + c), stroke: (thickness: x_thickness))
                            line((c, 1 + c, 0.5 + c), (-c, 1 - c, 0.5 - c), stroke: (thickness: x_thickness))
                            line((c, 1 + c, 0.5 - c), (-c, 1 - c, 0.5 + c), stroke: (thickness: x_thickness))
                            content((0, 1.38, 0.49), text(size: 13pt, fill: red)[$2$x])
                            line(
                                (-0.2, 1.2, 0.2),
                                (-0.2, 1.2, 0.8),
                                stroke: (thickness: 1pt),
                                mark: (start: ">", end: ">", scale: 0.5),
                            )
                            content("third-line.end", text(size: 13pt)[$S_(F\B2)$], padding: (0, 45pt, 12pt, 0))

                            set-style(stroke: (paint: black), mark: (fill: black))
                            line(
                                (-0.5, 0.5, 0.5),
                                (1.8, 0.5, 0.5),
                                stroke: (dash: "dashed"),
                                mark: (end: ">"),
                                name: "fourth-line",
                            )
                            line((0, a, a), (0, b, b), stroke: x_thickness)
                            line((0, b, a), (0, a, b), stroke: x_thickness)
                            line((1, a, a), (1, b, b), stroke: x_thickness)
                            line((1, a, b), (1, b, a), stroke: x_thickness)
                            arc((1.46, 0.61, 0.5), start: 98deg, stop: 421deg, radius: (1.2 / 13, 1.4 / 13), mark: (
                                start: ">",
                                scale: 0.4,
                            ))
                            content((1.52, 0.73, 0.5), text(size: 13pt)[$2$x])
                            content("fourth-line.end", text(size: 13pt)[#move(dx: 3pt, dy: -22pt)[$S_(\R2)$]])
                        },
                    )
                }),
                caption: text(1.2em)[The 48 symmetries of the cube],
                supplement: none,
            )
        ],
    )
    #cetz.canvas(length: 22pt, {
        import cetz.draw: *

        cube("wwowwgwwg ggyggwggg rrwbrrwrr", offset: (-5, 0), name: "f")
        cube("rgwwwwwwr gggggwggw ybbrrrrrr", offset: (5, 0), name: "s")
        content((-5, -3.5), [Has a cycle structure])
        content((5, -3.5), [Has the same cycle structure])
        content((-5, 3), [$A$])
        content((5, 3), [$A^(-1)$])

        set-style(
            mark: (
                start: (symbol: "rect", fill: none, stroke: 0pt),
                end: (symbol: ">", fill: black, width: 0.15, length: 0.2),
            ),
        )

        line(
            (to: "f", rel: (2.5, 0)),
            (to: "s", rel: (-2.5, 0)),
            stroke: (thickness: 2pt),
            mark: (end: ">", fill: black),
            name: "l",
        )
        content(
            ((to: "l.start", rel: (0, 0.5)), 50%, (to: "l.end", rel: (0, 0.5))),
            anchor: "south",
            "implies",
        )

        line(
            "f.U1",
            "f.F5",
            mark: (start: (length: 0.3)),
        )
        line(
            "f.U5",
            "f.U1",
            mark: (start: (length: 0.1)),
        )
        line(
            "f.F5",
            "f.U5",
            mark: (start: (length: 0.4)),
        )

        line(
            "f.U0",
            "f.U2",
            mark: (start: (symbol: ">", fill: black)),
        )
        line(
            "f.R0",
            "f.R6",
            mark: (start: (symbol: ">", fill: black)),
        )


        line(
            "s.F5",
            "s.U1",
            mark: (start: (length: 0.4)),
        )
        line(
            "s.U1",
            "s.U5",
            mark: (start: (length: 0.25)),
        )
        line(
            "s.U5",
            "s.F5",
            mark: (start: (length: 0.1)),
        )

        line(
            "s.U2",
            "s.U0",
            mark: (start: (symbol: ">", fill: black)),
        )
        line(
            "s.R6",
            "s.R0",
            mark: (start: (symbol: ">", fill: black)),
        )
    })
])

#pagebreak()

#thing(sc: 150%)[
    #figure(
        diagram(
            node((0, 0), [2], stroke: 0.5pt, name: <first>),
            node((rel: (11.75mm, 0mm)), [$+$ #h(0.5mm) $5 gt.not 8$]),
            node((0.75, 0.75), "5", stroke: 0.5pt, name: <second>),
            node((-0.75, 0.75), "1", stroke: 0.5pt, name: <third>),
            node(enclose: ((-0.75, 0), (0.75, 0.75)), name: <wrapper1>),
            edge(<first>, <second>),
            edge(<first>, <third>),
            edge(<wrapper1>, <wrapper2>, align(bottom)[Pathmax], "-|>"),

            node((4.5, 0), "4", stroke: 0.5pt, name: <fourth>),
            node((rel: (0mm, 0mm)), move(dx: 51pt)[#box[$+$ #h(0.5mm) $5 gt 8$ (Prune)]]),
            node((5.25, 0.75), "5", stroke: 0.5pt, name: <fifth>),
            node((3.75, 0.75), "1", stroke: 0.5pt, name: <sixth>),
            node(enclose: ((4.5, 0), (3.75, 0.75)), name: <wrapper2>),
            edge(<fourth>, <fifth>),
            edge(<fifth>, <fourth>, text(size: 10pt)[$-1$], "-|>", bend: 30deg),
            edge(<fourth>, <sixth>),
        ),
        caption: text(size: 12pt)[IDA\* pathmax at $"depth"=5, "depth limit"=8$],
        supplement: none,
    )

    #grid(
        columns: 2,
        align: center + bottom,
        column-gutter: 4em,
        grid.cell(breakable: false)[
            Rubik's Cube position counts
            #table(
                columns: (auto, auto, auto),
                table.header([*Depth*], [*Count*], [*Branching\ factor*]),
                [0], [1], [NA],
                [1], [18], [18],
                [2], [243], [13.5],
                [3], [3240], [13.333],
                [4], [43239], [13.345],
                [5], [574908], [13.296],
                [6], [7618438], [13.252],
                [7], [100803036], [13.231],
                [8], [1332343288], [13.217],
                [9], [17596479795], [13.207],
            )
        ],
        grid.cell(breakable: false)[
            Rubik's Cube position counts unique by \ symmetry $+$ antisymmetry
            #table(
                columns: (auto, auto, auto),
                table.header([*Depth*], [*Count*], [*Branching\ factor*]),
                [0], [1], [NA],
                [1], [2], [2],
                [2], [8], [4],
                [3], [48], [6],
                [4], [509], [10.604],
                [5], [6198], [12.177],
                [6], [80178], [12.936],
                [7], [1053077], [13.134],
                [8], [13890036], [13.190],
                [9], [183339529], [13.199],
            )
        ],
    )
]

#pagebreak()

#thing(sc: 130%, grid(
    columns: 2,
    column-gutter: 4em,
    cetz.canvas(length: 25pt, {
        import cetz.draw: *

        cube("wnonnnwng gnynnngng rnwnnnwnr", offset: (0, 0), name: "a")

        content(
            (0, -7),
            [
                #set text(1.5em)
                $
                    underbrace(
                        ... #table(
                            columns: 15,
                            inset: 0.4em,
                            stroke: black,
                            table.header[2][1][2][0][2][3][2][2][2][1][0][1][2][2][2],
                        ) ..., "88 million"
                    )
                $
            ],
            name: "b",
        )

        line(
            "a",
            (to: "b", rel: (-3.1, 1.45)),
            stroke: (thickness: 1.5pt),
            mark: (
                end: ">",
                fill: black,
                width: 0.3,
                height: 0.3,
            ),
        )
    }),

    grid(
        align: center + horizon,
        columns: 2,
        column-gutter: 15pt,
        row-gutter: 15pt,

        [#cetz.canvas(length: 17pt, {
            import cetz.draw: *

            cube("wwwwwwwww ggggggggg rrrrrrrrr")
        })],
        grid.cell(align: left)[Depth 0],
        [$dots.v$], [],
        [#cetz.canvas(length: 17pt, {
            import cetz.draw: *

            cube("ywrgwygwg ybwygwrrw orborbory")
        })],
        grid.cell(align: left)[Depth 5],
        [$dots.v$], [],
        [#cetz.canvas(length: 17pt, {
            import cetz.draw: *

            cube("brgbwbbrw oyggggyrb ryworbwwg")
        })],
        grid.cell(align: left)[Depth limit 12],
    ),
))


#pagebreak()

#thing(r: true, sc: 180%)[
    $
                   A^n & = () \
        (X A X^(-1))^n & = (X A X^(-1) X A X^(-1) ...) \
                       & = X A^n X^(-1) \
                       & = X X^(-1) \
                       & = () \
    $

    #v(5em)

    #cetz.canvas({
        import cetz.draw: *

        content((0, 0), $A B C D$, name: "f")
        content((5, 0), $X (A B C D) X^(-1)$, name: "s")
        content((0, -1), [Is a CCS solution])
        content((5, -1), [Is also a CCS solution])

        set-style(
            mark: (
                start: (symbol: "rect", fill: none, stroke: 0pt),
                end: (symbol: ">", fill: black, width: 0.15, length: 0.2),
            ),
        )

        line(
            (to: "f", rel: (1, 0)),
            (to: "s", rel: (-2, 0)),
            stroke: (thickness: 2pt),
            mark: (end: ">", fill: black),
            name: "l",
        )

        content(
            ((to: "l.start", rel: (0, -0.25)), 50%, (to: "l.end", rel: (0, -0.25))),
            anchor: "north",
            "implies",
        )
    })

    #v(5em)

    $
           & A^(-1) (A B C D) A && = B C D A \
        => & B^(-1) (B C D A) B && = C D A B \
        => & C^(-1) (C D A B) C && = D A B C \
        => & D^(-1) (D A B C) D && = A B C D \
    $
]

#pagebreak()

#thing()[
    #cetz.canvas({
        import cetz.draw: *

        set-style(
            content: (padding: .2),
            fill: black.lighten(20%),
            stroke: black.lighten(70%),
            mark: (
                end: (symbol: ">", fill: black, stroke: black, width: 0.15, length: 0.2),
            ),
            circle: (radius: .45, stroke: none, fill: black),
            rect: (
                fill: red.transparentize(80%),
                stroke: red.transparentize(30%),
                radius: 0.2,
            ),
        )

        content((4.4, 2), [#set text(size: 19pt); #set align(center); Sequential])
        group(
            {
                cetz.tree.tree(
                    ([], ([], [], []), ([], [], [])),
                    spread: 2,
                    grow: 1.75,
                    draw-node: (node, ..) => {
                        circle(())
                    },
                    draw-edge: (from, to, ..) => {
                        line((a: from, number: .6, b: to), (a: to, number: .6, b: from))
                    },
                    name: "tree",
                )

                rect-around(
                    (to: "tree.0-1-1", rel: (0.7, -0.7)),
                    (to: "tree.0-0-0", rel: (-0.7, 0.7)),
                )
            },
            name: "one",
        )

        // content(
        //     ((to: "l.start", rel: (0, 0.5)), 50%, (to: "l.end", rel: (0, 0.5))),
        //     anchor: "south",
        //     "implies",
        //   )

        //   cetz.tree.tree(([], [], []), spread: 2, grow: 1.75, draw-node: (node, ..) => {
        //   circle((10, 0), radius: .45, stroke: none)
        // }, draw-edge: (from, to, ..) => (), name: "tree2")
        line(
            (10, -3),
            (19, -3),
            stroke: (thickness: 2pt, paint: black),
            fill: black,
            name: "line",
        )
        content(
            ((to: "line.start", rel: (0, 1)), 50%, (to: "line.end", rel: (0, 1))),
            [#set align(center); #set text(size: 19pt); Symmetry + antisymmetry \ reduction],
        )
        group(
            {
                set-style(
                    mark: (end: "<", width: 3, length: 6, fill: gray.transparentize(70%), stroke: gray),
                )
                content((23, 2), [#set text(size: 19pt); #set align(center); Parallel])
                circle((21, 0), name: "r")
                circle((23, 0), name: "m")
                circle((25, 0), name: "l2")
                rect-around((to: "r", rel: (-0.7, -0.7)), (to: "l2", rel: (0.7, 0.7)))
                line((21, -6.64), (21, -6.65))
                line((23, -6.64), (23, -6.65))
                line((25, -6.64), (25, -6.65))
            },
            name: "two",
        )
    })
]

#pagebreak()

#thing(r: true, sc: 300%)[
    Website

    #scale(7%, image("../misc/qter-dev-qr.png"), reflow: true)

    qter.dev
]

#pagebreak()

#thing(r: true, sc: 300%)[
    = Programming Challenge

    Modify `simple.qat` to multiply `A` by 2

    #scale(70%, reflow: true)[#align(left)[
            1. Take one number as input instead of two
            2. Multiply it by 2
            3. `halt` with the product

        ]
    ]

    === Tips
    #scale(70%, reflow: true)[#align(left)[
        - Change `(4, 4)` to `(90, 90)` to represent bigger numbers
        - Addition is `add A 7` where `7` can be any positive number
        - All of the syntax is on a poster on the wall
    ]]
]

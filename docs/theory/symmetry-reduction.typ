#import "../book.typ": book-page, canvas, diagram
#import "@preview/cetz:0.4.2"
#import "../cube/cube.typ": *

#show: book-page.with(title: "Symmetry Reduction")

Symmetry reduction is the most famous way to compress pruning table entries. We thank Kociemba for his excellent explanations of symmetry reduction on his website. Take a good look at these two cube positions below:

#diagram(grid(
    columns: (20em, 20em),
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
))

They are different but they are _basically_ identical. If you replace red with blue, blue with orange, orange with green, green with red, you will have transformed $A$ into $B$. Because these two cube positions have the exact same structure of pieces, they need the same number of moves to reach a Cycle Combination Solver solution.

We call such positions _symmetrically equivalent_. If we really wanted to be serious about pruning table compression, what we can do is store a single representative of all symmetrically equivalent cubes because they would all share the same admissible heuristic value, and keeping a separate entry for each of these positions is a waste of memory.

Defining symmetrically equivalent cubes by figuring out an arbitrary way to recolor the cube is a very handwavy way to think about it, nor is it very efficient. The more mathematically precise way to define symmetrically equivalent cubes is with permutations. Two cube positions $A$ and $B$ are symmetrically equivalent if there exists a symmetry $S$ of the cube such that $S A S^(-1) = B$, where the $S$ operations are spatial manipulations the whole cube. We can prove that $A$ and $B$ are symmetrically equivalent using this model:

#canvas(length: 15pt, {
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
    content("four.north", text(1.2em)[#align(center + bottom)[$S^(-1)$\ Rotate $-90degree$]], anchor: "south")
    content("four.east", text(2em)[$=$], anchor: "west", padding: (0, 0, 0, 5pt))
    cube("wwowwgwwg ggyggwggg rrwbrrwrr", offset: (9, 0), name: "five")
    content("five.north", text(1.2em)[#align(center + bottom)[$B$\ Resultant $B$]], anchor: "south")
})

In group theory, $S A S^(-1)$ is called a _conjugation_ of $A$ by $S$—we first perform the symmetry, apply our desired permutation, and then perform the inverse of the symmetry to restore the original reference frame. The symmetries of arbitrary polyhedra themselves form a group, called a _symmetry group_, so we can guarantee an $S^(-1)$ element exists.

Symmetry reduction compresses the pruning table by the number distinct symmetries—all possible values of $S$—of the cube, so how many are there? The symmetry group of the cube $M$ consists of 24 rotational symmetries and 24 _mirror_ symmetries, for a total of 48 distinct symmetries. You can think of the mirror symmetries by imagining holding a Rubik's Cube position in a mirror to get a mirror image of that position. In this reflectional domain, we again apply the $24$ rotational symmetries. We illustrate one (of very many) ways to uniquely construct all of these symmetries, with the mirror symmetry highlighted in red.

#diagram(figure(
    cetz.canvas(length: 130pt, {
        import cetz.draw: *

        ortho(x: 11deg, y: 28deg, {
            let fillc(p) = gray.transparentize(p)
            on-xy(z: 0, rect((0, 0), (1, 1), stroke: 1.25pt, fill: fillc(90%)))
            on-xy(z: 1, rect((0, 0), (1, 1), stroke: 1.25pt, fill: fillc(90%)))
            on-xz(y: 0, rect((0, 0), (1, 1), stroke: 1.25pt, fill: fillc(60%)))
            on-xz(y: 1, rect((0, 0), (1, 1), stroke: 1.25pt, fill: fillc(60%)))
            on-yz(x: 0, rect((0, 0), (1, 1), stroke: 1.25pt, fill: fillc(70%)))
            on-yz(x: 1, rect((0, 0), (1, 1), stroke: 1.25pt, fill: fillc(80%)))

            set-style(mark: (fill: black, width: 0.1, length: 0.1, stroke: (dash: none)), paint: black)

            let x_len = 0.05
            let x_thickness = 1.5pt

            line((-0.13, -0.13, 1.13), (1.3, 1.3, -0.3), stroke: (dash: "dashed"), mark: (end: ">"), name: "first-line")
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

            line((0.5, -0.3, 0.5), (0.5, 1.61, 0.5), stroke: (dash: "dashed"), mark: (end: ">"), name: "second-line")
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
            line((1.15, -0.15, 0.5), (-0.4, 1.4, 0.5), stroke: (dash: "dashed"), mark: (end: ">"), name: "third-line")
            line((1 - c, -c, 0.5 + c), (1 + c, c, 0.5 - c), stroke: (thickness: x_thickness))
            line((1 - c, -c, 0.5 - c), (1 + c, c, 0.5 + c), stroke: (thickness: x_thickness))
            line((c, 1 + c, 0.5 + c), (-c, 1 - c, 0.5 - c), stroke: (thickness: x_thickness))
            line((c, 1 + c, 0.5 - c), (-c, 1 - c, 0.5 + c), stroke: (thickness: x_thickness))
            content((0, 1.38, 0.49), text(size: 13pt, fill: red)[$2$x])
            line((-0.2, 1.2, 0.2), (-0.2, 1.2, 0.8), stroke: (thickness: 1pt), mark: (start: ">", end: ">", scale: 0.5))
            content("third-line.end", text(size: 13pt)[$S_(F\B2)$], padding: (0, 45pt, 12pt, 0))

            set-style(stroke: (paint: black), mark: (fill: black))
            line((-0.5, 0.5, 0.5), (1.8, 0.5, 0.5), stroke: (dash: "dashed"), mark: (end: ">"), name: "fourth-line")
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
        })
    }),
    caption: text(1.2em)[The 48 symmetries of the cube],
    supplement: none,
))


#v(0.5em)

$
    M = {(S_(U\R\B3))^a dot (S_(\R2))^b dot (S_(\U4))^c dot (S_(\F\B2))^d | a in {0,1,2}, b in {0, 1}, c in {0, 1, 2, 3}, d in {0, 1}}
$

We discussed how symmetry conjugation temporarily changes a position's frame of reference before subsequently restoring it. Without any further context this would be fine, but in programming we efficiently represent a Rubik's Cube position by treating the centers as a fixed reference frame to avoid storing their states. This optimization is critical for speed because it makes position composition faster and minimizes data overhead. The ensuing caveat is that we _must_ always refer to a fixed frame of reference, so we have to rethink how symmetry conjugation works. The solution is simple, and the established theory still holds: we define the change of reference frame as a _position_ such that, when composed with the solved state, it transforms the pieces around the fixed frame of reference.

#canvas(length: 15pt, {
    import cetz.draw: *

    set-style(content: (
        padding: (0, 0, 7pt, 0),
    ))

    cube("nnnnwnnnn nnnngnnnn nnnnrnnnn", offset: (-2.5, 8))
    cube("nnnnbnnnn nnnnynnnn nnnnonnnn", offset: (2.5, 8), back: true)
    content((0, 10.25), text(1.2em)[#align(center + bottom)[Fixed frame of reference]], anchor: "south")

    set-style(content: (
        padding: (0, 0, 10pt, 0),
    ))

    cube("wwwwwwwww bbbbgbbbb rrrrrrrrr", offset: (-10, 0), name: "five")
    content(
        "five.north",
        align(center + bottom)[#text(1.2em)[$S_(F\B2)$]\ #text(fill: red)[Invalid position]],
        anchor: "south",
    )
    cube("wwwwwwwww rrrrgrrrr bbbbrbbbb", offset: (-3.33, 0), name: "two")
    content(
        "two.north",
        align(center + bottom)[#text(1.2em)[$S_(\U4)$]\ #text(fill: red)[Invalid position]],
        anchor: "south",
    )
    cube("rrrrwrrrr yyyygyyyy bbbbrbbbb", offset: (3.33, 0), name: "three")
    content("three.north", align(center + bottom)[#text(1.2em)[$S_(\UR\B3)$]\ Valid position], anchor: "south")
    cube("yyyywyyyy bbbbgbbbb rrrrrrrrr", offset: (10, 0), name: "four")
    content("four.north", align(center + bottom)[#text(1.2em)[$S_(\R2)$]\ Valid position], anchor: "south")

    cube("ggggbgggg wwwwwwwww ooooooooo", offset: (-10, -5), back: true)
    cube("ooooboooo yyyyyyyyy ggggogggg", offset: (-3.33, -5), back: true)
    cube("wwwwbwwww ooooyoooo ggggogggg", offset: (3.33, -5), back: true)
    cube("ggggbgggg wwwwywwww ooooooooo", offset: (10, -5), back: true)
})

The takeaway is in the observation that every symmetry position has the centers in the same spatial orientation.

Notice that the $S_(F\B2)$ and $S_(\U4)$ symmetries are invalid positions with this fixed reference frame—the latter because of the parity constraint, and the former because the mirror image produces a reflectional coloring. _This does not matter_ because the inconsistencies are un-done when $S^(-1)$ is applied; thus the conjugation $S A S^(-1)$ always results in a valid position.

$48$ symmetries is already quite a lot, but we can still do better. If we can show that both an arbitrary Rubik's Cube position and its inverse position require the same number of moves to reach a Cycle Combination Solver solution, we can once again store a single representative of the two positions and further compress the table by another factor of $2$. We call this _antisymmetry_.

Let us prove that our presumption is true.

+ Let $P$ and $S$ be defined as sequences such that $P$ $S$ is an optimal solution to the Cycle Combination Solver.

+ We take the inverse of $P$ $S$ to get $S^(-1) P^(-1)$ of the same sequence length, which is still an optimal solution to the Cycle Combination Solver. Taking the inverse of the "add 1" operation (which is $P$ $S$) is the "sub 1" operation; changing your frame of reference to think of "sub 1" as "add 1" yields another way to construct the exact same register.

+ We conjugate $S^(-1) P^(-1)$ with $S$ to get $S (S^(-1) P^(-1)) S^(-1) = P^(-1) S^(-1)$ of the same sequence length. It turns out that conjugate elements in a permutation group exhibit the same cycle structure, hence this is also an optimal solution to the Cycle Combination Solver. To understand why, we simplify the problem and examine the general case of two conjugate elements in a permutation group $A$ and $A B A^(-1)$. If permutation $B$ takes element $x$ to $y$, then $A B A^(-1)$ takes element $A(x)$ to $A(y)$. Indeed,

    $ (A B A^(-1))(A(x)) = A(B(A^(-1)(A(x)))) = A(B(x)) = A(y) $

    So every cycle $(x_1, x_2, dots, x_n)$ of $B$ is taken to the cycle $(A(x_1), A(x_2), dots, A(x_n))$ of $A B A^(-1)$. Viewing permutations as bijective maps of its elements, conjugation only relabels the elements moved by $B$. It does not change the cycle lengths nor how many cycles there are. We apply this corollary with $A = S$ and $B = S^(-1)P^(-1)$.

+ We have shown that if $P$ $S$ is an optimal solution to the Cycle Combination Solver then so is $P^(-1) S^(-1)$. $S$ and $S^(-1)$ are the same sequence length; thus, the positions reached by any arbitrary $P$ and by $P^(-1)$ starting from the solved state require the same number of moves to reach an optimal Cycle Combination Solver solution. This completes our proof.

Symmetry and antisymmetry reduction comes with a cost. During IDA\* search, every position must be transformed to its "symmetry and antisymmetry" representative before using it to query the pruning table. To do so we conjugate the position by the $48$ symmetries and the inverse by the $48$ antisymmetries to explore all the possible representatives. To identify the representative position after each conjugation, we look at its raw binary state representation and choose the lexicographic minimum (i.e. the minimum comparing byte-by-byte). Multiple symmetries may produce the representative position, however that is okay because at no point do we actually care about which symmetry conjugation did so; the result is still the same.

The symmetry and antisymmetry reduction algorithm as described so far would be slow—we need to perform 96 symmetry conjugations, and each is about as expensive as two moves. We use the following trick described by Rokicki: instead of computing the full conjugation for every symmetry conjugation, we compute the elements one-at-a-time. We take the least possible value for the first element of all the symmetry conjugations and filter for the ones that give us that value. Then, we compute all the second symmetry conjugation elements, find the least possible value for that, and so on. This optimization usually only ends up performing a single full symmetry conjugation.

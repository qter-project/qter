#import "../book.typ": book-page, canvas
#import "@preview/cetz:0.4.2"
#import "../cube/cube.typ": *

#show: book-page.with(title: "Parity and Orientation Sum")

Now, we need to show some properties of how the Rubik's Cube group works. First, we would ideally like a way to take pieces into account in our representation of the Rubik's Cube group. After all, we showed in the introduction how important they are to the mechanics of the cube. What we could do is instead of having a permutation group over all of the stickers, we could have a permutation group over all of the _pieces_. There are $12$ edges + $8$ corners = $20$ pieces on a Rubik's Cube, so we need a subgroup of the permutations on 20 elements. That's fine and dandy, but actually not sufficient to encode the full cube state. The reason is that pieces can rotate in place:

#canvas(length: 15pt, {
    import cetz.draw: *

    content((-9.9, 3.1), [#set text(1.5em); ()])
    cube("wwwwwwwww ggggggggg rrrrrrrrr", offset: (-9.9, 0))
    content((-3.9, 3.1), [#set text(1.5em); R U])
    cube("wwwwwwggg rrrggyggy wbbrrrrrr", offset: (-3.9, 0), name: "ufr")
    content((2.1, 3.1), [#set text(1.5em); R U F])
    cube("wwwwwwooy ggrggryyr gbbgrrgrr", offset: (2.1, 0), name: "fr")

    circle("ufr.center", radius: 1)
    circle("fr.fr", radius: 1)
})

You can see that happening here, where the UFR corner is _twisted_ in place in the first example and the FR edge is _flipped_ in place in the second example. This shows that _just_ encoding the positions of the pieces under-specifies the entire cube state, so we need to take orientation into account.

In general, any edge or corner can exist in any other edge or corner position in any orientation. So how can we encode this orientation in full generality? It's easy to tell that the UFR corner and FR edge are twisted and flipped respectively in the above examples because the pieces can be solved by simply rotating them in place. However, when the pieces are not in their solved positions, there is no way to solve them just by rotating them in place. We need some kind of reference frame to decide how to label a piece's orientation regardless of where it is on the cube. How can we define this reference frame?

Since the problem is that pieces can be unsolved, what we can do is imagine a special recoloring of the cube such that all pieces are indistinguishable but still show orientation. If the pieces aren't distinguishable, then they're _always_ in their "solved positions" since you can't tell them apart. Then it's easy to define orientation in full generality. Here is a recoloring that does that:

#canvas(length: 15pt, {
    import cetz.draw: *

    cube("wwwwwwwww ggggggggg rrrrrrrrr", offset: (-2.5, 0))
    cube("bbbbbbbbb yyyyyyyyy ooooooooo", offset: (2.5, 0), back: true)

    line((5, 0), (8, 0), mark: (end: "straight"))

    translate((13, 0))

    cube("bbbbbbbbb nnnbbbnnn nnnnnnnnn", offset: (-2.5, 0))
    cube("nnnbbbnnn bbbbbbbbb nnnnnnnnn", offset: (2.5, 0), back: true)
})

You can imagine that we are taking a Rubik's cube and replacing all of the stickers with new stickers of the respective colors. The reason that we can do this is that we already know how to represent the locations of pieces using a permutation group, so it is valid to throw out the knowledge of a piece's location while figuring out how to represent orientation. To determine the orientation of a piece on a normally colored Rubik's Cube, you can take the algorithm to get to that cube state and apply it to our specially recolored cube:

#canvas(length: 15pt, {
    import cetz.draw: *

    cube("owoywoowb bgybgwybg oggorgwrw", offset: (-2.3, 0))
    cube("wbyrbybwr rryyyyrog bbgoogrrw", offset: (2.3, 0), back: true)

    line((0, -2), (0, -4.2), mark: (end: "straight"))

    translate((0, -6))

    cube("nbnbbnnbn nnbbbbbbn nbnnnbbnb", offset: (-2.3, 0))
    cube("bnbnbbnbn nnbbbbnnn nnnnnnnnb", offset: (2.3, 0), back: true)
})

Even though the UFR corner isn't in its solved position, we can still say that the piece in the UFR position is twisted because the blue sticker isn't facing up, like it is in the recolored solved state. You would be able to "solve" that piece—make it look like the respective position in the recolored solved state—by simply rotating it in place. This gives us a reference frame to define orientation for a piece regardless of where it is located on the cube.

Note that this recoloring is entirely arbitrary and it's possible to consider _any_ recoloring of the solved state such that all pieces are indistinguishable but still exhibit orientation, as long as you are consistent with your choice. However, this recoloring is standard due to its nice symmetries as well as properties we will describe in the next paragraph.

Based on this recoloring, you can see that the move set $⟨U, D, R 2, F 2, L 2, B 2⟩$ preserves orientation of all of the pieces, and on top of that, $R$ and $L$ preserve orientation of the edges but not of the corners. The moves $F$ and $B$ flip four edges, while $R$, $F$, $L$, and $B$ twist four corners.


#canvas(length: 15pt, {
    import cetz.draw: *

    content((0, 3.1), [#set text(1.5em); R])
    cube("bbnbbbbbn nnbbbbnnb nnnnnnnnn", offset: (-2.5, 0))
    cube("bnnbbbbnn nbbbbbnbb nnnnnnnnn", offset: (2.5, 0), back: true)

    translate(x: 13, y: 0)

    content((0, 3.1), [#set text(1.5em); F])
    cube("bbbbbbnnn nbnnbnnbn bnnbnnbnn", offset: (-2.5, 0))
    cube("nnnbbbnnn bbbbbbnnn nnnnnnbbb", offset: (2.5, 0), back: true)
})

Note that corners actually have _two_ ways of being misoriented. If the corner is twisted clockwise, we say that its orientation is one, and if it's counter-clockwise, we say that its orientation is two. Otherwise, it is zero.

#canvas(length: 15pt, {
    import cetz.draw: *

    cube("bbbbbbbbn nnnbbbnnn bnnnnnnnn", offset: (-2.5, 0), name: "cl")
    cube("bbbbbbbbn nnbbbbnnn nnnnnnnnn", offset: (2.5, 0), name: "ccl")

    content((-2.5, 3.1), [#set text(2em); $1$])
    content((2.5, 3.1), [#set text(2em); $2$])
    circle("cl.center", radius: 1)
    circle("ccl.center", radius: 1)
})

We know that $F$ and $B$ flip four edges, but what do $R$, $F$, $L$, and $B$ do to corners? Well whatever it is, those four do the same thing because all four of those moves are symmetric to each other with respect to corners in our recoloring. Therefore, we can track what happens to the corners for just one of them.

#canvas(length: 15pt, {
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

This should make logical sense. We already know that if you apply $R$ twice, the corners don't get twisted, and that can be seen in the figure as well. If you perform $R$ twice, each corner will get a $+1$ twist and a $+2$ twist, which sums to three, except that three wraps around to zero.

From here, we can prove that for _any_ cube position, if you sum the orientations of all of the corners, you get zero. Any quarter turn about $R$, $F$, $L$, and $B$ adds a total of $1 + 2 + 1 + 2 = 6$ twists to the corners, which wraps around to zero. Therefore, moves cannot change the total orientation sum so it always remains zero. This shows why a single corner twist is unsolvable on the Rubik's Cube:

#canvas(length: 15pt, {
    import cetz.draw: *

    content((0, 3.1), [#set text(1.5em); $emptyset$])
    cube("wwwwwwwwg ggrgggggg wrrrrrrrr", offset: (-2.5, 0))
    cube("bbbbbbbbb yyyyyyyyy ooooooooo", offset: (2.5, 0), back: true)
})

The orientation sum for the corners in this position is one (one for the twisted corner plus zero for the rest), however it's impossible to apply just one twist using moves, and the corner orientation sum will always be one regardless of the moves that you do.

Similarly, we can show that the orientation sum of _edges_ is also always zero. If we call the non-flipped state "zero" and the flipped state "one", then the $F$ and $B$ turns both flip four edges, adding $+4$ to the edge orientation sum of the cube, which wraps around to zero. Therefore, a single edge flip is unsolvable too:

#canvas(length: 15pt, {
    import cetz.draw: *

    content((0, 3.1), [#set text(1.5em); $emptyset$])
    cube("wwwwwwwgw gwggggggg rrrrrrrrr", offset: (-2.5, 0))
    cube("bbbbbbbbb yyyyyyyyy ooooooooo", offset: (2.5, 0), back: true)
})

Is there anything else that's unsolvable? Actually, yes! For this to make sense, we have to think of permutations as a composition of various swaps. For example, a four-cycle can be composed out of three swaps:

$
    (1, 2) · (1, 3) · (1, 4) = (1, 2, 3) · (1, 4) = (1, 2, 3, 4)
$

In general, any permutation can be expressed as a composition of swaps. So what does this have to do with Rubik's Cubes? Well a funny thing with swaps is that permutations can _only_ either be expressed as a combination of an even or an odd number of swaps. This is called the _parity_ of a permutation. You can see that a four-cycle has odd parity because creating it requires an odd number of swaps. Any quarter turn of a Rubik's Cube can be expressed as a four cycle of corners and a four cycle of edges, which is $3 + 3 = 6$ swaps. Overall, the permutation is even.

Therefore, a two-swap of Rubik's Cube pieces is unsolvable because creating it requires a single swap, and doing turns only does even permutations, meaning the permutation of pieces will always remain odd.


#canvas(length: 15pt, {
    import cetz.draw: *

    content((0, 3.1), [#set text(1.5em); $emptyset$])
    cube("wwwwwwwww grggggggg rgrrrrrrr", offset: (-2.5, 0))
    cube("bbbbbbbbb yyyyyyyyy ooooooooo", offset: (2.5, 0), back: true)
})

Is there any other arrangement of pieces that is unsolvable? Actually no! You can show this by counting the number of ways that you can take apart and randomly put together a Rubik's Cube, then dividing that by three because two thirds of those positions will be unsolvable due to the corner orientation sum being non-zero. Then divide by two for edge orientation sum, and then divide by two again for parity. You will see that the number you get is $4.3·10^19$ which is exactly the size of the Rubik's Cube group.


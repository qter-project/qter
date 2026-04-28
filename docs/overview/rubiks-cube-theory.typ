#import "../book.typ": book-page, diagram, canvas
#import "../cube/cube.typ": *

#show: book-page.with(title: "Rubik's Cube Theory")

Before we can explain how to turn a Rubik's Cube into a computer, we have to explain what a Rubik's Cube _is_ and the fundamental mathematics behind how it works. First, a Rubik's Cube is made out of three kinds of pieces: _Corners_, _Edges_, and _Centers_.

#diagram(grid(
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

    image("../../media/paper/corner.jpg", height: 10em, width: 10em),
    image("../../media/paper/edge.jpg", height: 10em, width: 10em),
    image("../../media/paper/core.jpg", height: 10em, width: 10em),
))


You can see that the centers are attached to each other by the _core_ and are only able to rotate in place. This allows us to treat the centers as a fixed reference frame to tell whether or not a sticker is on the correct side. For example, if we have the following scramble,

#canvas(length: 22pt, {
    cube("bbbbwbbbb oooogooooo wwwwrwwwww")
})

it may look as if the centers are the only thing unsolved, but in fact we would actually consider _everything else_ to be unsolved. The reason is that all of the stickers are different from the center on the same side as it. Next, people who are beginners at solving Rubik's Cubes often make the mistake of solving individual stickers instead of whole pieces.

#canvas(length: 22pt, {
    cube("wooywwoow ggggggggg rrbwrborw")
})

If someone does this, then they haven't actually made progress towards a solution because the stickers on the pieces move together, which means that all of the pieces on the green face in the example given will have to be reshuffled to bring the rest of the stickers to their correct faces. Instead, it's better to solve a full "layer" (3x3x1 block), because all of the pieces are in their correct spots and won't need to be moved for the entire rest of the solve. The takeaway being that in general, _we need to think about the cube in terms of pieces rather than in terms of stickers_.

#canvas(length: 22pt, {
    cube("yyrbwowww ggggggggg rbbrrrrry")
})

Now, we need some way to notate scrambles and solutions on a Rubik's Cube. We will use the conventional "Singmaster Notation" which is standard in the Rubik's Cube solving community. First, we will name the six sides of a Rubik's Cube _Up_ (U), _Down_ (D), _Right_ (R), _Left_ (L), _Front_ (F), and _Back_ (B). Then, we will let the letter representing each face represent a clockwise turn about that face.

#canvas(length: 15pt, {
    import cetz.draw: *

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
})

To represent double turns or counterclockwise turns, we append a `2` or a `'` respectively to the letter representing the face.

#canvas(length: 14pt, {
    import cetz.draw: *

    content((-9.9, 3.1), [#set text(1.5em); U])
    cube("wwwwwwwww rrrgggggg bbbrrrrrr", offset: (-9.9, 0))
    content((-4.9, 3.1), [#set text(1.5em); U2])
    cube("wwwwwwwww bbbgggggg ooorrrrrr", offset: (-4.9, 0))
    content((0, 3.1), [#set text(1.5em); U'])
    cube("wwwwwwwww ooogggggg gggrrrrrr", offset: (0, 0))
})

Here is a full table of all 18 moves for reference:


#canvas(length: 15pt, {
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
})

It may look like we're forgetting some moves. After all, there are _three_ layers that you can turn, not just two, and we haven't given names to turns of the three middle slices. However, we don't actually need to consider them because "slice" turns can be written in terms of the 18 "face" turns.

#canvas(length: 15pt, {
    import cetz.draw: *

    content((-9.9, 3.1), [#set text(1.5em); ??])
    cube("wgwwgwwgw gyggyggyg rrrrrrrrr", offset: (-9.9, 0))
    content((-4.9, 3.1), [#set text(1.5em); R' L])
    cube("bwbbwbbwb wgwwgwwgw rrrrrrrrr", offset: (-4.9, 0))
})

Those two cube states are actually the same because if you take the first cube and rotate it so that the green center is in front and the white center is on top again, we would see that it is exactly the same as the second cube. Since we're using the centers as a reference point, we can consider these two cube states to be exactly the same. Slice turns do have names, but we don't need to care about them for the purpose of this paper.

Another thing that we will need to name are the pieces of a Rubik's Cube. To do this, we can simply list the sides that the piece has stickers on. For example, we can talk about the "Up, Front, Right" or _UFR_ corner, or the "Front, Left" — _FL_ — edge.

#canvas(length: 15pt, {
    import cetz.draw: *

    cube("wwwwwwwww ggggggggg rrrrrrrrr", name: "cube")

    line((rel: (2, 2), to: "cube.center"), "cube.center", mark: (end: "straight"), name: "ufr")
    line((rel: (-2, 0), to: "cube.F3"), "cube.F3", mark: (end: "straight"), name: "fl")

    content((rel: (0.8, 0.4), to: "ufr.start"), [#set text(1.5em); UFR])
    content((rel: (-0.8, 0), to: "fl.start"), [#set text(1.5em); FL])
})

This system is able to uniquely identify all of the pieces. Finally, a sequence of moves to apply to a Rubik's Cube is called an _algorithm_. For example, (L2 D2 L' U' L D2 L' U L') is an algorithm that speed cubers memorize to help them at the very end of a solution when almost every piece is solved. It performs a three-cycle of the UBL, DBL, and DBR corners:

#canvas(length: 15pt, {
    import cetz.draw: *

    content((0, 3.1), [#set text(1.5em); L2 D2 L' U' L D2 L' U L'])
    cube("ywwwwwwww ggggggggg rrrrrrrry", offset: (-2.5, 0))
    cube("bbbbbbbbb oyoyyyyyy woroooooo", offset: (2.5, 0), back: true, name: "b")

    line("b.B2", "b.B8", mark: (end: "straight"))
    line("b.B8", "b.B6", mark: (end: "straight"))
    line("b.B6", "b.B2", mark: (end: "straight"))
})

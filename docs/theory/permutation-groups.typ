#import "../book.typ": book-page, canvas, diagram
#import "@preview/cetz:0.4.2"
#import "../cube/cube.typ": *

#show: book-page.with(title: "Permutation Groups")

There are lots of things that can form groups, but the things that we'll use to represent the Rubik's cube are _permutations_, which are re-arrangements of items in a set. For example, we could notate a permutation like

$
    & 0 #h(1em) && 1 #h(1em) && 2 #h(1em) && 3 #h(1em) && 4 \
    & ↓         && ↓         && ↓         && ↓         && ↓ \
    & 2         && 1         && 4         && 3         && 0 \
$

where the arrows define the rearrangement. Note that we can have permutations of any number of items rather than just five. We can leave out the top row of the mapping because it will always be the numbers in order, so we could notate it $2, 1, 4, 3, 0$. We can see that this permutation can also be thought of as an invertible, or _bijective_, function between the numbers ${0, 1, 2, 3, 4}$ and themselves.

So now, lets construct a group. The set of all permutations of a particular size, five in this example, will be the set representing our group. Then, we need an operation. Since permutations are basically functions, permutation composition can simply be function composition!

#let y(x) = text(fill: red, $#x$)

$
          a & =    && 2,         && 1,         && 4,         && 3,         && 0 \
          b & =    && #y(4),     && #y(3),     && #y(0),     && #y(2),     && #y(1) \
            &      && arrow.b    && arrow.b    && arrow.b    && arrow.b    && arrow.b \
    a dot b & = a\( && #y(4)), a\( && #y(3)), a\( && #y(0)), a\( && #y(2)), a\( && #y(1)) \
            & =    && 0,         && 3,         && 2,         && 4,         && 1 \
$

From here, the group axioms are trivial. Our identity $e$ is the do-nothing permutation, $0, 1, 2, 3, 4$. We know that associativity holds because permutation composition is identical to function composition which is known to be associative. We know that there is always an inverse because permutations are _bijective_ mappings and you can simply reverse the arrows to form the inverse:

$
    &0 #h(1em) && 1 #h(1em) && 2 #h(1em) && 3 #h(1em) && 4 &&&&0 #h(1em) && 1 #h(1em) && 2 #h(1em) && 3 #h(1em) && 4 \
    a^(-1) = #h(0.5em) &↑ && ↑ && ↑ && ↑ && ↑ #h(1em) && → #h(1em) && ↓ && ↓ && ↓ && ↓ && ↓ \
    &2 && 1 && 4 && 3 && 0 &&&& 4 && 1 && 0 && 3 && 2 \
$

Therefore, permutation composition satisfies all of the group axioms, so it is a group. Next, there also exists a much cleaner way to notate permutations, called _cycle notation_. The way you would write $a$ in cycle notation is as $(0, 2, 4)(1)(3)$. Each item maps to the next item in the list, wrapping around at a closing parenthesis. The notation is saying that $0$ maps to $2$, $2$ maps to $4$, $4$ maps to $0$ (because of the wraparound), $1$ maps to itself, and $3$ also maps to itself. This is called "cycle notation" because it shows clearly the underlying cycle structure of the permutation. $0$, $2$, and $4$ form a three-cycle and $1$ and $3$ both form one-cycles. It is also conventional to leave out the one-cycles and to just write down $(0, 2, 4)$.

This notation also provides a simple way to determine exactly how many times one has to compose a permutation with itself for it to equal identity. Since a three-cycle takes three iterations for its elements to return to their initial spots, you can compose a three-cycle with itself three times to give identity. In full generality, we have to take the _least common multiple_ of all of the cycle lengths to give that number of repetitions. For example, the permutation $(0, 1, 2)(3, 4, 5, 6)$ has a three-cycle and a four-cycle, and the LCM of three and four is $12$, therefore exponentiating it to the twelfth power gives identity.

A permutation is something that we can easily represent in a computer, but how can we represent a Rubik's Cube in terms of permutations? It is quite simple actually...

#diagram(scale(26%, reflow: true, image("../../media/paper/Stickered Cube.png")))

A Rubik's Cube forms a permutation of the stickers! We don't actually have to consider the centers because they don't move, so we would have a permutation of $(9 - 1) · 6 = 48$ stickers. We can define the turns on a Rubik's Cube in terms of permutations like so:

$
    U & = ( 1, 3, 8, 6)( 2, 5, 7, 4)( 9,33,25,17)(10,34,26,18)(11,35,27,19) \
    D & = (41,43,48,46)(42,45,47,44)(14,22,30,38)(15,23,31,39)(16,24,32,40) \
    R & = (25,27,32,30)(26,29,31,28)( 3,38,43,19)( 5,36,45,21)( 8,33,48,24) \
    L & = ( 9,11,16,14)(10,13,15,12)( 1,17,41,40)( 4,20,44,37)( 6,22,46,35) \
    F & = (17,19,24,22)(18,21,23,20)( 6,25,43,16)( 7,28,42,13)( 8,30,41,11) \
    B & = (33,35,40,38)(34,37,39,36)( 3, 9,46,32)( 2,12,47,29)( 1,14,48,27) \
$

The exact numbers aren't actually relevant for understanding, but you can sanity-check that exponentiating all of them to the fourth gives identity, due to all of the cycles having length four. This matches our expectation of how Rubik's Cube moves should work.

Now, if we restrict our set of permutations to only contain the permutations that are reachable through combinations of $⟨U, D, R, L, F, B⟩$ moves (after all, we can't arbitrarily re-sticker the cube), then this structure is mathematically identical — _isomorphic_ — to the Rubik's Cube group. This is called a _subgroup_ of the permutation group of 48 elements because the Rubik's Cube group is like its own little group hidden inside that bigger group of all permutations.

It may appear as if our definition of the Rubik's cube group includes too many elements: after all, each sticker on a Rubik's cube has seven identical twins, but we're giving them different numbers and treating them as if they were unique. If there existed an algorithm that could swap two stickers of the same color, then our definition would count those as different states whereas they would really be the same state. However, we don't have to worry about this because all of the _pieces_ on a cube are unique. The only way to swap two stickers would be to swap two pieces, and that would definitely produce a different cube state. Note that we don't get to make that assumption for puzzles like the 4x4x4 which have identical center pieces, however we are conveniently not writing about the 4x4x4 because our code doesn't even work for that yet #emoji.face.shush;.

One final term to define is an _orbit_. An orbit is a collection of stickers (or whatever elements are being permuted, in full generality) such that if there exists a sequence of moves that moves one sticker in the orbit to another sticker's place, then that other sticker must be in the same orbit as the first. On a Rubik's Cube, there are two orbits: the corners and the edges. There obviously doesn't exist an algorithm that can move a corner sticker to an edge sticker's place or vice versa, therefore the corners and edges form separate orbits. Intuitively, you can find orbits of any permutation subgroup by coloring the stickers using the most colors possible such that the colors don't change when applying moves.

#canvas(length: 15pt, {
    import cetz.draw: *

    content((-9.9, 3.1), [#set text(1.5em); ()])
    cube("roronoror roronoror roronoror", offset: (-9.9, 0))
    line((-7.5, 0), (-5.3, 0), mark: (end: "straight"))
    content((-2.9, 3.1), [#set text(1.5em); R])
    cube("roronoror roronoror roronoror", offset: (-2.9, 0))
})

Excluding centers, the best we can do is two colors, and those two colors highlight the corner and edge orbits.

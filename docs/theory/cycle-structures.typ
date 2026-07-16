#import "../book.typ": book-page, canvas
#import "@preview/cetz:0.4.2"
#import "../cube/cube.typ": *

#show: book-page.with(title: "Cycle Structures")

Now that we understand orientation, we can notate cube states in terms of permutation and orientation of pieces rather than just permutation of stickers. This will make the way in which the Qter Architecture Solver works easier to think about. Lets see how we can represent the $R U$ algorithm.

#canvas(length: 15pt, {
    import cetz.draw: *

    content((0, 3), [#set text(1.5em); R U])
    cube("wwwwwwggg rrrggyggy wbbrrrrrr", offset: (-2.5, 0))
    cube("ooowbbwbb byybyybyy oogoogooy", offset: (2.5, 0), back: true)
})

Next, lets trace where the pieces go. Instead of using numbers to represent the pieces in the cycle notation, we can simply use their names.

$
    ("UFR") ("FDR", "UFL", "UBL", "UBR", "DBR") ("FR", "UF", "UL", "UB", "UR", "BR", "DR")
$

Note that I'm writing down the one-cycle of the UFR corner because we will see that it twists in place. If you would like, you can manually verify the tracing of the pieces. Next, we need to examine changes of orientation.

#canvas(length: 15pt, {
    import cetz.draw: *

    content((0, 3), [#set text(1.5em); R U])
    cube("bbbbbbnbn nnnbbbnnb bnnnnnnnn", offset: (-2.5, 0))
    cube("nnnbbbbnn nbbbbbnbb nnnnnnnnb", offset: (2.5, 0), back: true)
})

I'm going to notate orientation by writing the amount of orientation that a piece acquires above it.

$
    &"+1" &&"+2" &&"+0" &&"+0" &&"+2" &&"+1" &&"+0" &&"+0" &&"+0" &&"+0" &&"+0" &&"+0" &&"+0"\
    \(&"UFR"\) \(&&"FDR", &&"UFL", &&"UBL", &&"UBR", &&"DBR"\) \(&&"FR", &&"UF", &&"UL", &&"UB", &&"UR", &&"BR", &&"DR"\)
$

The process of translating a cube state into cycles of pieces including orientation is known as _blind tracing_ because when blind solvers memorize a puzzle, they memorize this representation. Using this representation, we can actually calculate the order of the algorithm. In the intro, we claimed that the $R U$ algorithm repeats after performing it $105$ times, but now we can prove it.

First, we have to consider how many iterations it takes for each cycle to return to solved. To find this, we have to consider both the length of the cycle and the overall orientation accrued by each piece over the length of the cycle. Lets consider the first cycle first. It has length one, meaning the piece stays in its solved location, however the piece returns with some orientation added, so it takes three iterations overall for that piece to return to solved.

#canvas(length: 15pt, {
    import cetz.draw: *

    content((0, 3), [#set text(1.5em); $("R U")^3$])
    cube("yggywwbbw rrgggwggo rgyrrobbo", offset: (-2.5, 0), name: "f")
    cube("rrbwbbwbb gyywyywyy oorooroow", offset: (2.5, 0), back: true)

    circle("f.center", radius: 1)
})

Next, let's consider the cycle of edges. They have a cycle of seven and don't accrue orientation at all, so it simply takes 7 iterations for the edges to return to solved.

#canvas(length: 15pt, {
    import cetz.draw: *

    content((0, 3), [#set text(1.5em); $("R U")^7$])
    cube("rwgwwwrwg bgrgggggr wrwrrrwrw", offset: (-2.5, 0))
    cube("obgbbbobb byyyyybyy ooyoooooy", offset: (2.5, 0), back: true)
})

Finally, let's consider the cycle of corners. It has length 5, so all pieces return to their solved locations after 5 iterations, but you can see that they accrue some amount of orientation.

#canvas(length: 15pt, {
    import cetz.draw: *

    content((0, 3), [#set text(1.5em); $("R U")^5$])
    cube("obbwwygwr obwggwggr grworrygb", offset: (-2.5, 0))
    cube("rrwgbbybb ryywyygyy oobooroow", offset: (2.5, 0), back: true)
})

How can we calculate how much orientation? Since each piece will move through each location in the cycle, it will move through each addition of orientation, meaning that all pieces will accrue the _same_ orientation, and that orientation will be the sum of all orientation changes, looping around after three. The cycle has three orientation changes, $+2$, $+2$, and $+1$, and summing them gives $+5$ which loops around to $+2$. You can see in the above example that all corners in the cycle have $+2$ orientation.

It will take three traversals through the cycle for the orientation of the pieces to return to zero, so the cycle resolves itself after 15 iterations.

#canvas(length: 15pt, {
    import cetz.draw: *

    content((0, 3), [#set text(1.5em); $("R U")^15$])
    cube("wwwwwwwgw grgggyggg rbrrrrrrr", offset: (-2.5, 0))
    cube("bobwbbbbb yyybyyyyy ooooogooo", offset: (2.5, 0), back: true)
})

Now, the _entire_ cycle resolves itself once all individual cycles resolve themselves. To calculate when, we can simply take the LCM:

$
    lcm(3, 7, 15) = 105
$

This also clarifies what pieces we have to select as parameters for "solved-goto". We need a representative piece from every cycle that isn't redundant. We don't need to care about the 3 cycle because it is always solved whenever the 15 cycle is. We can pick any representatives from the 7 and 15 cycles, for example FDR and FR. Using those, the QAT program

```janet
.registers {
  A ← 3x3 (R U)
}

label:
solved-goto A label
```

...compiles to the Q program

```l
Puzzles
A: 3x3

1 | solved-goto FDR FR 1
```

#import "../book.typ": book-page, canvas
#import "@preview/cetz:0.4.2"
#import "../cube/cube.typ": *

#show: book-page.with(title: "Orientation and Parity Sharing")

Lets examine a real Qter architecture, for example the 90/90 one:

#canvas(length: 15pt, {
    import cetz.draw: *

    content(((-9.9 - 4.9) / 2, 3.1), [#set text(1.2em); A = R' F' L U' L U L F U' R])
    cube("obwywwwgw bwggggggg rrbrrbrrr", offset: (-9.9, 0))
    cube("orgobwbbo yybyywyyy yowboooor", offset: (-4.9, 0), back: true)
    content(((2.5 + 7.1) / 2, 3.1), [#set text(1.2em); B = U F R' D' R2 F R' U' D])
    cube("wwwwwywwg ggrwgyggb wgrbrryyy", offset: (2.1, 0))
    cube("bbbbbbooy ggoryyrrr boooooyro", offset: (7.1, 0), back: true)
})

Now let's blind-trace the cube positions:

$
          & "+2"    && "+1"   && "+1"   && "+0"   && "+0"    && "+0"  && "+1"  && "+0"  && "+0"  && "+0" \
    A = \( & "DBL"\)\( && "UF"\)\( && "UFL", && "UBL", && "UBR"\)\( && "UL", && "LB", && "RB", && "UB", && "LD"\) \
$

$
          & "+1"    && "+1"    && "+1"   && "+1"   && "+2"    && "+1"   && "+0"  && "+0"  && "+0"  && "+1"  && "+0" \
    B = \( & "DBL"\)\( && "UFR"\)\( && "DFR", && "DFL", && "DBR"\)\( && "RD"\)\( && "UR", && "FL", && "DB", && "FR", && "FD"\)
$

From here, we can calculate the orders of each register. $A$ has cycles of length $3, 2, 9, 10$ with LCM $90$, and $B$ has cycles $3, 3, 9, 2, 10$ with LCM $90$. However, we can see that both cycles twist the DBL corner! This is not good for the cycles being independently decodable. However, what we can do is ignore that one piece when calculating cycle lengths and performing "solved-goto" instructions. Without that shared piece, we get that $A$ has cycles $2, 9, 10$ still with LCM $90$ and $B$ has cycles $3, 9, 2, 10$ still with LCM $90$.

Why would we need to share pieces? The fundamental reason is due to the orientation and parity constraints described previously. You've seen that having a non-zero orientation sum allows the lengths of cycles to be extended beyond what they might otherwise be, however that net orientation needs to be cancelled out elsewhere to ensure that the orientation sum of the whole puzzle remains zero. For example, for the register $A$, the $+2$ on DBL cancels out the $+1$ on that 15 cycle.

It's possible for us to use the same piece across different registers to cancel out orientation, allowing more pieces to be used for storing data. We call this _orientation sharing_, and the pieces that are shared are called _shared pieces_. We can also use sharing to cancel out parity. For both $A$ and $B$, all of the cycles that contribute to the order have even parity, meaning that parity doesn't need to be cancelled out. However if they had odd parity, then we could share two pieces that can be swapped to cancel out parity. We call that _parity sharing_.

Note that it would actually be possible for all of the DBL, UFR, UF, and RD pieces to be shared and the cycles would still work; it just happens that they aren't. If they were shared, then there could be the possibility of a shorter algorithm to produce a cycle, but at the cost of the ability to use those pieces to detect whether the register is divisible by two or three.


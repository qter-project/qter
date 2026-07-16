#import "../book.typ": book-page, canvas
#import "@preview/cetz:0.4.2"
#import "../cube/cube.typ": *

#show: book-page.with(title: "Group Theory")

First, we have to build a foundation of how we can represent Rubik's Cubes in the language of mathematics. That foundation is called _group theory_. A _group_ is defined to be a _set_ equipped with an _operation_ (denoted $a b$ or $a · b$) that follows the following _group axioms_:

- There exists an _identity_ element $e$ such that for any element of the group $a$, $a · e = a$.
- For all elements $a$, $b$, $c$, $(a · b) · c = a · (b · c)$. In other words, the operation is _associative_.
- For each $a$ in the group, there exists $a^(-1)$ such that $a · a^(-1) = e$. In other words, every element has an _inverse_ with respect to the group operation.

Importantly, commutativity is _not_ required. So let's see how this definition applies to the Rubik's Cube. To form a group, we need a _set_, and for the Rubik's Cube, this set is the infinite set of all _move sequences_ that you can apply to a puzzle. For example, doing nothing is an element of the set. If you turn the top face then that's an element of the set. If you just scramble your cube randomly, then even that sequence of moves is part of the set.

Next, we need an _operation_. For the Rubik's Cube, this will be jamming together the sequence of moves. We will call this operation _composition_ because it is very similar to function composition.

#canvas(length: 15pt, {
    import cetz.draw: *

    content((-9.9, 3.1), [#set text(1.5em); R U R' U'])
    cube("wwowwgwwg ggyggwggg rrwbrrwrr", offset: (-9.9, 0))
    content((-3.9, 3.1), [#set text(1.5em); F L])
    cube("bwwbwwboo wggwggogg wrrwrrwrr", offset: (-3.9, 0))
    content((2.1, 3.1), [#set text(1.5em); (R U R' U') (F L)])
    cube("bwobwgroo wggwggowy wrwwrrgrr", offset: (2.1, 0))

    circle((-7, 0), fill: black, radius: 0.15)
    content((-0.9, 0), [#set text(2em); $=$])
})

Now, let's verify that all of the group axioms hold. First, we need an identity element. This identity is simply the "do nothing" sequence! Lets verify this, and let $A$ be an arbitrary scramble:

#canvas(length: 15pt, {
    import cetz.draw: *

    content((-9.9, 3.1), [#set text(1.5em); A])
    cube("wwowwgwwg ggyggwggg rrwbrrwrr", offset: (-9.9, 0))
    content((-3.9, 3.1), [#set text(1.5em); ()])
    cube("wwwwwwwww ggggggggg rrrrrrrrr", offset: (-3.9, 0))
    content((2.1, 3.1), [#set text(1.5em); (A) () = A])
    cube("wwowwgwwg ggyggwggg rrwbrrwrr", offset: (2.1, 0))

    circle((-7, 0), fill: black, radius: 0.15)
    content((-0.9, 0), [#set text(2em); $=$])
})

Regardless of what the first move sequence is, appending the "do nothing" algorithm will lead to the same sequence. Next, lets verify associativity, letting $A$, $B$, and $C$ be arbitrary scrambles.

#canvas(length: 15pt, {
    import cetz.draw: *

    content((-9.9, 3.1), [#set text(1.5em); A B])
    cube("wwrwwgwwg rryggyggy rrbrrbrrb", offset: (-9.9, 0))
    content((-3.9, 3.1), [#set text(1.5em); C])
    cube("wwwwwwooo ggggggggg wrrwrrwrr", offset: (-3.9, 0))
    content((2.1, 3.1), [#set text(1.5em); (A B) (C) = A B C])
    cube("wwrwwgoog ggrggryyy wrbwrbgrb", offset: (2.1, 0))

    circle((-7, 0), fill: black, radius: 0.15)
    content((-0.9, 0), [#set text(2em); $=$])

    translate((0, -8))

    content((-9.9, 3.1), [#set text(1.5em); A])
    cube("wwwwwwwww rrrgggggg bbbrrrrrr", offset: (-9.9, 0))
    content((-3.9, 3.1), [#set text(1.5em); B C])
    cube("wwgwwgooo ggggggyyy wrrwrrgrr", offset: (-3.9, 0))
    content((2.1, 3.1), [#set text(1.5em); (A) (B C) = A B C])
    cube("wwrwwgoog ggrggryyy wrbwrbgrb", offset: (2.1, 0))

    circle((-7, 0), fill: black, radius: 0.15)
    content((-0.9, 0), [#set text(2em); $=$])
})

Because of the nature of how jamming together algorithms works, parentheses can essentially be ignored. Therefore, the composition operation is associative. Finally we must show that every sequence of moves has an inverse. In our case, an inverse exists simply because we can undo the entire move sequence. Here is an algorithm to find that inverse:

```ts
function inverse(moves: List<Move>): List<Move> {
  reverse(moves)

  for (move in moves) {
    if move.ends_with("'") {
      remove(`'` from move)
    } else if move.ends_with("2") {
      // Leave it
    } else {
      append(`'` to move)
    }
  }

  return moves
}
```

This works because any clockwise base move X cancels with it's counterclockwise pair X' and vice versa, and any double turn X2 cancels with itself.

$
    "R'" "U2" "F" " " "L" · "inverse"("R'" "U2" "F" " " "L") & = ("R'" "U2" "F" " " "L") ("L'" "F'" "U2" "R") \
                                                             & = "R'" "U2" "F" "F'" "U2" "R" \
                                                             & = "R'" "U2" "U2" "R" \
                                                             & = "R'" "R" \
                                                             & = () \
$

Next, it is important to distinguish a _cube state_ from an _algorithm to reach that cube state_. We just described a group of Rubik's cube _algorithms_ but not a group of Rubik's cube _states_. We can say that the group of Rubik's cube _algorithms_ is an _action_ on the group of Rubik's cube _states_. It turns out that Rubik's cube states can actually form a group by themselves without having to think about algorithms. We will explore this group of Rubik's cube states next, because it turns out that it is much more amenable to mathematical analysis and representation inside of a computer. After all, move sequences alone don't give us insight into the structure of the puzzle itself.

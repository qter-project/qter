#import "../book.typ": book-page, canvas
#import "@preview/cetz:0.4.2"
#import "../cube/cube.typ": *

#show: book-page.with(title: "Qter Architecture Solver")

You now have all of the background knowledge required to understand what the Qter Architecture Solver does. It is split into two phases:

The Cycle Combination Finder calculates what registers are possible on a Rubik's Cube by determining how cycles can be constructed and how pieces would have to be shared. One of the outputs of Cycle Combination Finder for the 90/90 architecture shown before would be something like:

```
Shared: Two corners, Two edges
A:
  - Cycle of three corners with non-zero net orientation
  - Cycle of five edges with non-zero net orientation
B:
  - Cycle of three corners with non-zero net orientation
  - Cycle of five edges with non-zero net orientation
```

Then the Cycle Combination Solver would take that as input and output the shortest possible algorithms that produce the given cycle structures.

Oh, and most of the theory that we just covered is generalizable to arbitrary twisty puzzles, and the Qter Architecture Solver is programmed to work for all of them. However, we will stick to the familiar Rubik's Cube for our explanation.


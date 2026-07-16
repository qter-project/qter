#import "../book.typ": book-page, canvas, diagram
#import "@preview/cetz:0.4.2"
#import "../cube/cube.typ": *

#show: book-page.with(title: "Larger Puzzles")

The overwhelming majority of our research has been within the realm of the Rubik's Cube, and so far, we have yet to run the Cycle Combination Solver on non-Rubik's Cube twisty puzzles. While we are confident all of our theory generalizes to larger twisty puzzles (with minor implementation detail differences), there is a practical concern we expect to run into.

Optimally solving the 4x4x4 Rubik's Cube has been theorized to take roughly as much time as computing the minimum number of moves to solve any 3x3x3 Rubik's Cube, which took around 35 CPU-years. It may very well be the case that the Cycle Combination Solver, even with all its optimization tricks, will never be able to find a solution to a Cycle Combination Finder cycle structure for larger twisty puzzles. Thus, we are forced to sacrifice optimality in one of three ways:

- We can write _multiphase_ solvers for these larger puzzles. Multiphase solvers are specialized to the specific puzzle, and they work by incrementally bringing the twisty puzzle to a "closer to solved" state in a reasonable number of moves. However, designing a multiphase solver is profoundly more involved compared to designing an optimal solver. This approach is unsustainable because it is impractical and difficult to write a multiphase solver for every possible twisty puzzle.
- We can resort to methods to solve arbitrary permutation groups. We speculate that the most promising method of which may be to utilize something called a strong generating set . The GAP computer algebra system implements this method in the `PreImagesRepresentative` function as illustrated in <gap>. The algorithms produced by the strong generating sets can quickly become large. In the future, we plan to investigate the work of Egner and apply his techniques to keep the algorithm lengths in check.
- When all other options have been exhausted, we must resort to designing our cycle structure algorithms by hand. This approach would likely follow the blindfolded twisty puzzle solving method of permuting a three or five pieces at a time. Contrary to popular belief, the blindfolded solving method is simple, and it is generalizable to arbitrary twisty puzzles.


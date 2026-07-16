#import "../book.typ": book-page, canvas
#import "@preview/cetz:0.4.2"
#import "../cube/cube.typ": *

#show: book-page.with(title: "Cycle Combination Solver")

The Cycle Combination Finder of the Qter Architecture Solver finds the non-redundant cycle structures of each register in a Qter architecture. We are not done yet—for every cycle structure, we need to find an algorithm that, when applied to the solved state, yields a state with that cycle structure. That is, we need to solve for the register's "add 1" operation. Once we have that, all other "add N"s can be derived by repeating the "add 1" operation $N$ times and then shortening the algorithm using an external Rubik's Cube solver.

The Cycle Combination Solver adds two additional requirements to this task. First, it solves for the _shortest_, or the _optimal_ algorithm that generates this cycle structure. This is technically not necessary, but considering that "add 1" is observationally the most executed instruction, it greatly reduces the overall number of moves needed to execute a _Q_ program. Second, of all solutions of optimal length, it chooses the algorithm easiest to physically perform by hand, which we will discuss in a later section that follows.

In order to understand how to optimally solve for a cycle structure, we briefly turn our attention to an adjacent problem: optimally solving the Rubik's Cube.

=== Optimal solving background

First, what do we mean by "optimal" or "shortest"? We need to choose a _metric_ for counting the number of moves in an algorithm, and there are a variety of ways to do so. In this paper, we will use what is known as the _half turn metric_, which means that we consider U2 to be a single move. An alternative choice would be the _quarter turn metric_ which would consider U2 to be two moves, however that is less common in the literature and we won't use it in this paper.

In an optimal Rubik's Cube solver, we are given a random position, and we must find the shortest algorithm that brings the Rubik's Cube to the solved state. Analogously, the Cycle Combination Solver starts from the solved state and finds the shortest algorithm that brings the puzzle to a position with our specified cycle structure. The only thing that's fundamentally changed is something trivial — the goal condition. We bring up optimal _solving_ because this allows us to reuse its techniques which have been studied for the past 30 years.

It would be reasonable to expect there to be a known structural property of the Rubik's Cube that makes optimal solving easy. Indeed, to find a _good_ solution to the Rubik's Cube, the technique of Kociemba's algorithm cleverly utilizes a specific subgroup to solve up to 3900 individual position per second _near_ optimally. However, we want to do better than that.

Unfortunately, to find an _optimal_ solution, the only known approach is to brute force all combinations of move sequences until the Rubik's Cube is solved. To add some insult to injury, Demaine proved that optimal $N times N times N$ cube solving is NP-complete. However, this doesn't mean we can't optimize the brute force approach. We will discuss a variety of improvements that can be made, some specific to the Cycle Combination Solver only, but unless there is a significant advancement in group theory relating to the problem it is solving, the runtime is necessarily going to be exponential.


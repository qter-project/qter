Tentative "who's saying what?"
- Intro: Arhan
- How Qter Works: Henry
- Cycles & CCF: Asher
- CCS: Arhan
- Conclusion: Henry
(of course, everyone has permission to change the writing of their section)

= Intro / Hook

Hello Hack from Scratch! Check out this Rubik's Cube. It's running a program. These aren't just random moves - the cube is turning according to this code. It began with an input of 6, and it finishes with an output of 13, because 13 is the 6th Fibonacci number.

This program uses the cube to calculate Fibonacci numbers, and it is part of our project Qter - the cube computer.


My name is Arhan... intro
my name is Henry... intro
and my name is Asher, also known online as TheGrayCuber where I make content about math and cubing.
And we'd also like to mention collaboratos Daniel and Neel.

We'll explain how Qter works in four stages: representing numbers, writing programs, finding structures, and implementing efficiently.

= How Qter works

== Numbers

First, we need some way to represent numbers on a Rubik's cube. To do this, we will suppose that the solved cube represents the number zero. Next, we know that the most basic operation you can do to a Rubik's cube is to make turns, so perhaps we can suppose that doing turns performs addition. For example, lets say that turning the up face represents addition by one. Then this represents one, then two, then three, then it loops back around to zero. It's analagous to how the numbers on a clock loop around after reaching twelve.

But what if we want to represent bigger numbers than just three? The trick is to use a more complicated sequence of moves to represent addition by one. For example, adding one can be the algorithm R U, which means we turn the right face then the up face. This algorithm makes a 105 cycle, meaning we can represent numbers up to 104.

Now it's not super useful to be able to represent only a single number on the Rubik's cube. Can we represent more than one? Sure! You can see that there's a bunch of unused pieces in this block here (point to 2x2x3 block on DL). We can also define adding one as this algorithm, which means that we turn the down face twice, then the left face twice, and then repeat those.

Notice that this only affected pieces in the lower left. These two algorithms are disjoint, so they can store separate values. We'll say this is Register A and this is Register B.


== Branching & Q

So, we now know how to represent numbers, but we promised you a _computer_. How can you do computations using our setup? First, a program is represented as an external sequence of instructions that a person or a robot would follow.

We have a special notation for a sequence of instructions, called "Q" code, and since this is Hack from Scratch, we'll show the equivalent statements in Scratch.

First, the simplest instruction is to perform a sequence of moves on a cube. This is logically equivalent to performing an addition on a register.

```q
0 | U R U' D2 B
```

```
Scratch:
- change `A` by `1`
```

You may notice that the instruction starts with the number zero. This is because all of the instructions get numbers as labels, and we'll explain why later. After the bar character, each of the symbols starts with a letter representing which face to turn. By default, you would turn a face clockwise, but if the letter is followed by a `2` or a `'`, then you would turn it twice or counterclockwise respectively.

A fun fact about Rubik's cubes is that any position can be solved in 20 or fewer moves. What that means is that if we perform a long sequence of additions, we can take that final cube state, solve it in 20 or fewer moves, and then undo that solution. The process of undoing that solution has the same effect as performing all of those additions, meaning we can replace them with the much shorter sequence of moves, and effectively merge all of them into one instruction.

```q
0 | U R U' D2 B
1 | U R U' D2 B
2 | U R U' D2 B
3 | U R U' D2 B
4 | U R U' D2 B
5 | B U2 B' L' U2 B U L' B L B2 L
6 | B U2 B' L' U2 B U L' B L B2 L
7 | B U2 B' L' U2 B U L' B L B2 L
```

```q
0 | B D R' B' D F R2 L' B' L' D2 F' U L' D2 F2 B
```

```
Scratch:
- change `A` by `5`
- change `B` by `3`
```

Next, the first instruction that you will typically actually see in a program is the `input` instruction.

```q
0  | input "Which Fibonacci number to calculate:"
           B2 U2 L F' R B L2 D2 B R' F L
           max-input 8
```
```
Scratch:
- Ask "Which Fibonacci number to calculate:" and wait
- Change `A` by `Answer`
```

What you would do is decide what number you want to input, and then repeat the given sequence of moves that number of times. If we wanted to input `5`, we would repeat the sequence of moves 5 times. The `max-input` statement tells you the biggest number you can input right before the register wraps around to zero.

Now, here's where things get fun: let me tell you about our _control_ instructions. First is the `goto` instruction. When you reach one, it tells you to jump to a particular line number.

```q
0 | U R U' D2 B
1 | goto 0
```
```
Scratch:
- forever
    - change `A` by 1
```

In this example, we're being told to repeat `U R U' D2 B` forever! As soon as we finish performing the move sequence, we reach the `goto` instruction which tells us to jump back to line number zero, which is where we just came from.

Next is our most powerful control instruction, called `solved-goto`. What it does, is it tells us to jump to a different part of the program only if a particular part of the cube is solved.

```q
0 | solved-goto UBL BL 2
1 | U R U' D2 B
2 | (rest of program)
```
```
Scratch:
- if <not <`A` = 0>> then
    - change `A` by 1
```

Here, `UBL` and `BL` refer to physical pieces of the Rubik's cube, where each letter represents a side that the piece is on. Here, `UBL` refers to the "Up Back Left" corner piece here, and "BL" refers to the "Back Left" edge piece here. If _both_ of these pieces are in their solved positions, then the `solved-goto` instruction tells us to jump to the line number provided. Otherwise, we do nothing and continue to the next instruction. Even if some but not all pieces are solved, we would still go to the next instruction.

The pieces in a `solved-goto` instruction are chosen such that a register is zero if and only if all of those pieces are solved. That makes this effectively a "jump if zero" operation. This is actually sufficient for us to be able to perform any calculation using the cube.

For example, what if we want to add together two registers? The code for that would look like this:

```q
0 | solved-goto UBL BL 3
1 | F' B' L D' L B' U B2 D B' U' F U2 B D2 R'
    (subtract 1 from register A, add one to register B)
2 | goto 0
3 | (rest of program)
```
```
Scratch
- repeat until <a = 0>
    - change `A` by -1
    - change `B` by 1
```

What this does is repeatedly subtract one from register A and add one to register B. Once `A` is zero, the `solved-goto` makes us exit the loop, and `B` has been set to `B + A`. This pattern of "do something until a register is zero" is so common that we have our own instruction for it, called `repeat-until`.

```q
0 | repeat until UBL BL solved
           F' B' L D' L B' U B2
           D B' U' F U2 B D2 R'
```

This tells you to repeat the given sequence of moves until the given pieces are solved, and is identical to the previous Q code.

The final instruction that you will see is the `halt` instruction. It tells you that the program is finished and how to decode the final result.

```q
14 | halt "The number is"
          U' R'
          counting-until UFL UF
```
```
Scratch:
- say (join "The number is " `A`) for 2 seconds
- stop all
```

The `halt` instruction contains a sequence of moves that represents a "subtract 1" operation on a particular register. What you do is you repeat the given sequence of moves until the given list of pieces are all solved, and the final output is the number of times that you had to repeat the move sequence. This effectively decodes the value of that register. It's essentially the same as the `repeat until` instruction, but you count in your head the number of times that you repeat.

For example, executing the given halt instruction on this cube state would give one, two, three, four, and five. You can see that the "Up Front Left" and "Up Front" pieces are both in their solved positions, so we finished doing the halt instruction.

Now that's all of the instructions! You can put these together to compute fibonacci or multiplication on a regular Rubik's cube. But in order to run more complicated programs, we need to find a good structure within the cube.

= QAS

== CCF

Each register is defined by some cycle. In the earlier example, where we add 1 by turning the up face, we are cycling through 4 different positions. This is a 4 cycle. The length of the cycle determines how high our numbers can go.

Let's say we have 8 pieces. We could move them in a big loop to get an 8 cycle, but there's a better option. Instead we can split them into a 3 cycle and a 5 cycle. The 3 cycle will repeat with period 3, and the 5 cycle with period 5, so together they make a 15 cycle!

And there's something special about this 3 and 5. If we instead split the pieces into 4 cycles, then they both repeat with period 4, so they're in sync. One of them is redundant. 3 and 5 are distinct prime powers, so there is no redundancy. We find optimal solutions by combining prime powers.

Let's apply this idea to the cube! There are 8 corner pieces and 12 edge pieces. So we can make a 3 cycle and 5 cycle on the corners, and a 4 cycle and 7 cycle on the edges. These all multiply together to make a 420 cycle.

But this setup only allows for one register. For most programs we'll need at least two. So, instead we could split the pieces like this, so that registers A and B each have a 2 cycle, 3 cycle, and 5 cycle, which combine to make a 30 cycle.

This is basic idea of of our Cycle Combination Finder. An algorithm that searches for the best sets of prime powers that fit within the cube.

Although, it's a little more complicated than just cycles. Let's say we have a 2 cycle of edges. Each edge has two stickers, so we can add more complexity by flipping one of them and then doing the cycle.

After two repetitions the edges are back in their locations, but they're both flipped. It takes another two repetitions to solve them. So flipping has made this into a 4 cycle. It doubled the length!

By adding this flipping into our Cycle Combination Finder, we can get a 1260 cycle for one register, or 90 cycles for two registers, or 30 cycles for 3 registers.

And this algorithm can easily generalize to larger puzzles, like a 4x4, 5x5, 6x6 and so forth. If we apply it to the 11x11, we can get *not sure yet, need to run the numbers*

But this is just the theoretical step. Once we find a cycle combination, we must then find a way to actually implement it on the cube.

== CCS

The Cycle Combination Finder finds the non-redundant cycle structures of each register in a Qter architecture. But we aren't done yet—the Cycle Combination Solver solves for the actual sequence of moves that creates that cycle structure on the Rubik's Cube. In other words, it solves for the register’s “add 1” operation. Once we have that, all other “add N”s can be derived by repeating the “add 1” operation N times and then shortening the algorithm using a Rubik’s Cube solver like Henry how explained.

The Cycle Combination Solver additionally solves for optimal algorithm. The solution it returns will simultaneously be the shortest algorithm as well as the easiest to physically perform by hand. In order to understand how to optimally solve for a cycle structure, we briefly turn our attention to an adjacent problem: optimally solving the Rubik’s Cube.

In an optimal Rubik’s Cube solver, we are given a random position, and we must find the shortest algorithm that brings the Rubik’s Cube to the solved state.

Analogously, the Cycle Combination Solver starts from the solved state and finds the shortest algorithm that brings the puzzle to a position with our specified cycle structure. The only things that have fundamentally changed are both trivial — the starting position and the goal condition. We bring up optimal solving because this allows us to reuse its techniques which have been studied for the past 30 years.

You might expect there to be a known structural property of the Rubik’s Cube that makes optimally solving for a cycle structure easy. As it turns out, the only known way is to brute force all combinations of move sequences until the Rubik’s Cube is solved. Fortunately, we can significantly optimize the brute force approach. We will discuss a variety of improvements that can be made.

A more formal way to think about the Cycle Combination Solver is to think of the state space as a tree of Rubik’s Cube positions joined by the 18 moves. The number of moves that have been applied to any given position is simply that position’s corresponding level in the tree. We will refer to these positions as nodes

Our goal is now to find a node with the specified cycle structure at the topmost level of the tree, a solution of the optimal move length. Those familiar with data structures and algorithms will think of the most obvious approach to this form of tree searching: breadth-first search (BFS). BFS is an algorithm that explores all nodes in a level before moving on to the next one. Indeed, BFS guarantees optimality, and works in theory, but not in practice: extra memory is needed to keep track of child nodes that are yet to be explored. At every level, the number of nodes scales by a factor 18, and so does the extra memory needed. At a depth level i.e. sequence length of just 8, BFS would require storing $18^9$ depth-9 nodes or roughly 200 billion Rubik’s Cube states in memory. This is clearly not practical; we need to do better. We now consider a sibling algorithm to BFS: depth-first search (DFS). DFS is an algorithm that explores all nodes as deep as possible before backtracking. It strikes our interest because the memory overhead is minimal; all you need to keep track of is the path taken to reach a node, something that can be easily managed during the search. However, because we explore nodes depth-first, it offers no guarantee about optimality, so we still have a problem.

We now consider a sibling algorithm to BFS: depth-first search (DFS). DFS is an algorithm that explores all nodes as deep as possible before backtracking. It strikes our interest because the memory overhead is minimal; all you need to keep track of is the path taken to reach a node, something that can be easily managed during the search.

This alone offers no guarantee about optimality, so we still tweak the DFS implementation so that it explores up until a specified depth, testing whether each node at this depth is a solution, without exploring further. We repeatedly run this implementation at increasing depth limits until a solution is found. Put simply, you do a DFS of depth 1, then of depth 2, and so on. This idea is known as iterative-deepening depth-first search (IDDFS), a hybrid of a breadth-first and depth- first search. IDDFS does repeat some work each iteration, but the cost is always small relative to the last depth because the Rubik’s Cube search tree grows exponentially.

IDDFS solves the memory issue, but is lacking in speed because tree searching is still slow. The over- whelming majority of paths explored lead to no solution. What would be nice is if we could somehow know whether all paths that continue from a given node are dead ends without having to check by brute-force. For this, we introduce the idea of a pruning table. For any given Rubik’s Cube position, a pruning table tells you a lower bound on the number of moves needed to reach a Cycle Combination Solver solution. Suppose we are running IDDFS until depth 12, we’ve done 5 moves so far, and we have reached this node.

If we query the pruning table and it says that this position needs at least 8 moves to reach a Cycle Combination Solver solution, we know that this branch is a dead end. 5 moves done so far plus 8 left is 13, which is more than the 12 at which we plan to terminate. Hence, we can avoid having to search this position any longer.

Pruning tables must never overestimate the distance to a solution. If in the above example, the lower bound was wrong and there really was a solution in 12 moves, then the heuristic would prevent us from finding it. Combining IDDFS and an admissible heuristic is known as Iterative Deepening A\* (IDA\*).

How are we supposed to store all 43 quintillion positions of the Rubik’s Cube in memory? Well, we don’t: different types of pruning tables solve this problem by sacrificing either information or accuracy to take up less space. Hence, pruning tables give an admissible heuristic instead of the exact number of moves needed to reach a Cycle Combination Solver solution.

Loosely speaking, pruning tables can be thought of as a form of meet-in-the-middle search, more generally known as a space—time trade-off. The improvements are dramatic because the number of nodes at increasing depths grows exponentially, but there is no free lunch: we have to pay for this speedup by memory.

The larger the pruning table heuristic, the better the pruning, and the lesser the search depth. So, we need to carefully design our pruning tables to maximize:
- how much information we can store within a given memory constraint; and
- the value of the admissible heuristic

To Asher — there is significantly more I can talk about pruning tables, IDA\* optimizations, SIMD, multithreading, etc. But I feel like most of that isn't necessary because they are just implementation details. Let me know if the video script is too short and I can fill it in with more content from the qter paper.

(Insert visuals from here https://typst.app/project/rkXU6CFpEvKJvtoB6AJMWo )

== MCC

The Cycle Combination Solver finds the optimal solution to the results of the Cycle Combination Finder. They are only optimal by length, but not by easiness to perform. If you pick up a Rubik’s cube right now, you would find it much harder to perform 𝐵2 𝐿′ 𝐷2 compared to 𝑅 𝑈 𝑅′ despite being the same length because this algorithm requires you to awkwardly re-grip your fingers to make the turns. This might seem like an unimportant metric, but remember: we want Qter to be human-friendly, and the “add 1” instruction is observationally the most executed one.

Thus, the Cycle Combination Solver first finds all optimal solutions of the same length, and then utilizes our rewrite of trangium’s Movecount Coefficient Calculator to output the solution easiest to physically perform. The Movecount Coefficient Calculator simulates a human hand turning the Rubik’s Cube to score algorithms by this metric, factoring in criteria such as algorithm length, number of double turns, and re-grips.

https://www.speedsolving.com/threads/movecount-coefficient-calculator-online-tool-to-evaluate-the-speed-of-3x3-algorithms.79025/

= Q&A in chat

= Conclusion / Transition to SIGHORSE panel

Well, now we're out of time! If you want to learn more about Qter, you can read our writeup about the project at `qter.dev/paper.pdf`, or even better, you can donate at "The Lore" tier to get a copy of a book called SIGHORSE, which is faux academic journal that Purdue Hackers put together that includes papers detailing lots of other Purdue Hackers projects including ours. The next stream session will be a panel interviewing some of the people involved with producing SIGHORSE, so hold on tight for that!

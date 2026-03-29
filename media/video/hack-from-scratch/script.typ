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

Now that's all of the instructions! You can put these together to compute fibonacci or multiplication on a regular Rubik's cube. So now, lets talk about how we can find all of these magic move sequences that give us big registers.

= QAS

== Cycles

- Delegate writing to Asher

== CCF

- Delegate writing to Asher

== CCS

The Cycle Combination Finder finds the non-redundant cycle structures of each register in a Qter architecture. But we aren't done yet—the Cycle Combination Solver solves for the actual sequence of moves that creates that cycle structure on the Rubik's Cube. In other words, it solves for the register’s “add 1” operation. Once we have that, all other “add N”s can be derived by repeating the “add 1” operation N times and then shortening the algorithm using a Rubik’s Cube solver like Henry how explained.

The Cycle Combination Solver additionally solves for optimal algorithm. The solution it returns will simultaneously be the shortest algorithm as well as the easiest to physically perform by hand. In order to understand how to optimally solve for a cycle structure, we briefly turn our attention to an adjacent problem: optimally solving the Rubik’s Cube.

#cetz.canvas(length: 22pt, {
    import cetz.draw: *

    cube("rgywwrwry byowggwyr bggyrwywr", offset: (-5, 0), name: "f")
    cube("wwwwwwwww ggggggggg rrrrrrrrr", offset: (5, 0), name: "s")

    line(
      (to: "f", rel: (2.5, 0)),
      (to: "s", rel: (-2.5, 0)),
      stroke: (thickness: 2pt),
      mark: (end: ">", fill: black),
      name: "l",
    )
    content(
      ((to: "l.start", rel: (0, 0.5)), 50%, (to: "l.end", rel: (0, 0.5))),
      anchor: "south",
      "min",
    )
})

In an optimal Rubik’s Cube solver, we are given a random position, and we must find the shortest algorithm that brings the Rubik’s Cube to the solved state.



Analogously, the Cycle Combination Solver starts from the solved state and finds the shortest algorithm that brings the puzzle to a position with our specified cycle structure. The only things that have fundamentally changed are both trivial — the starting position and the goal condition. We bring up optimal solving because this allows us to reuse its techniques which have been studied for the past 30 years.

You might expect there to be a known structural property of the Rubik’s Cube that makes optimally solving for a cycle structure easy. As it turns out, the only known way is to brute force all combinations of move sequences until the Rubik’s Cube is solved. Fortunately, we can significantly optimize the brute force approach. We will discuss a variety of improvements that can be made.

(Insert visuals from here https://typst.app/project/rkXU6CFpEvKJvtoB6AJMWo )

= Q&A in chat

= Conclusion / Transition to SIGHORSE panel

Well, now we're out of time! If you want to learn more about Qter, you can read our writeup about the project at `qter.dev/paper.pdf`, or even better, you can donate at "The Lore" tier to get a copy of a book called SIGHORSE, which is faux academic journal that Purdue Hackers put together that includes papers detailing lots of other Purdue Hackers projects including ours. The next stream session will be a panel interviewing some of the people involved with producing SIGHORSE, so hold on tight for that!

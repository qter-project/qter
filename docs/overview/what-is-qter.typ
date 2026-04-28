#import "../book.typ": book-page, diagram, canvas
#import "@preview/cetz:0.4.2"
#import "../cube/cube.typ": *

#show: book-page.with(title: "What is Qter?")

Now that you understand what a Rubik's Cube is and the fundamental mechanics, we can explain the ideas of using it to perform computation. The most important thing for a computer to be able to do is represent numbers. Let's take a solved cube and call it "zero".

The fundamental unit of computation in Qter is an _algorithm_, or a sequence of moves to apply to the cube. The fundamental unit of computation on a classical computer is addition, so let's see what happens when we apply the simplest algorithm, just turning the top face, and call it addition by one. What does this buy us?

#diagram(image("../../media/paper/Light U States.png"))

We can call this new state "one". Since we want the algorithm (U) to represent addition, perhaps applying (U) _again_ could transition us from state "one" to state "two", and again to state "three", and again to state "four"?

When we apply (U) the fourth time, we find that it returns back to state "zero". This means that we can't represent every possible number with this scheme. We should have expected that, because the Rubik's Cube has a _finite_ number of states whereas there are an _infinite_ amount of numbers. This doesn't mean that we can't do math though, we just have to treat numbers as if they "wrap around" at four. This is analogous to the way that analog clocks wrap around after twelve, and this form of math is well-studied under the fancier name "modular arithmetic".

=== Addition

Can we justify this way of representing numbers? Let's consider adding "two" to "one". We reach the "two" state using the algorithm (U U), so if we apply that algorithm to the "one" state, we will find the cube in the same state as if we applied ((U) (U U)), or (U U U), which is exactly how we reach the state "three". It's easy to see that associativity of moves makes addition valid in this scheme. What if we wanted to add "three" to "two"? We would expect a result of "five", but since the numbers wrap around upon reaching four, we would actually expect to reach the state of "one". You can try on your own Rubik's Cube and see that it works.

What if we want to perform subtraction? We know that addition is performed using an algorithm, so can we find an algorithm that adds a negative number? Let's consider the state that represents "one". If we subtract one, we would expect the cube to return to state "zero". The algorithm that brings the cube from state "one" to state "zero" is (U'). This is exactly the _inverse_ of our initial (U) algorithm. If we want to subtract two, we can simply subtract one twice as before: (U' U').

You may notice that subtracting one is equivalent to adding three, because (U') is equivalent to (U U U). It may seem like this is a contradiction, but it actually isn't: Adding three to one gives four, but since four wraps around to zero, our result is actually zero, as if we subtracted one. In general, any number can be seen as either positive or negative: -1 = 3, -2 = 2, and -3 = 1. You can manually verify this yourself if you like. Interestingly, this is how signed arithmetic works in a classical computer, but that's irrelevant for our purposes.

=== Bigger numbers

If the biggest number Qter could represent was three, it would not be an effective tool for computation. Thankfully, the Rubik's Cube has 43 quintillion states, leaving us lots of room to do better than just four. Consider the algorithm (R U). What if instead of saying that (U) adds one, we say that (R U) adds one? We can play the same game using this algorithm. The solved cube represents zero, (R U) represents one, (R U R U) represents two, etc. This algorithm performs a much more complicated action on the cube, so we should be able to represent more numbers. In fact, the maximum number we can represent this way is 104, and the cube re-solves itself after 105 iterations. We would say that the algorithm has _order 105_.

#canvas(length: 15pt, {
    import cetz.draw: *

    content((-9.9, 3.1), [#set text(1.5em); "Zero"])
    cube("wwwwwwwww ggggggggg rrrrrrrrr", offset: (-9.9, 0))
    content((-4.9, 3.1), [#set text(1.5em); "One"])
    cube("wwwwwwggg rrrggyggy wbbrrrrrr", offset: (-4.9, 0))
    content((0.1, 3.1), [#set text(1.5em); "Two"])
    cube("gwwgwwyyr rrwggbggb goorrbrrb", offset: (0.1, 0))
    content((4.9, 3.1), [#set text(1.5em); ...])
    content((10.1, 3.1), [#set text(1.5em); "104"])
    cube("wwbwwbwwr oowggwggw grrgrrgrr", offset: (10.1, 0))
    content((15.1, 3.1), [#set text(1.5em); "105"])
    cube("wwwwwwwww ggggggggg rrrrrrrrr", offset: (15.1, 0))
})

There are still lots of cube states left; can we do better? Unfortunately, it's only possible to get to 1259, wrapping around on the 1260th iteration. You can try this using the algorithm `R U2 D' B D'`. This has been proven to be the maximum order possible.

=== Branching

The next thing that a computer must be able to do is _branch_: without it we can only do addition and subtraction and nothing else. If we want to perform loops or only execute code conditionally, qter must be able to change what it does based on the state of the cube. For this, we introduce a `solved-goto` instruction.

If you perform `R U` on a cube a bunch of times without counting, it's essentially impossible for you to tell how many times you did the algorithm by _just looking_ at the cube. With one exception: If you did it _zero_ times, then the cube is solved and it's completely obvious that you did it zero times. Since we want qter code to be executable by humans, the `solved-goto` instruction asks you to jump to a different location of the program _only if_ the cube is solved. Otherwise, you simply go to the next instruction. This is functionally equivalent to a "jump-if-zero" instruction which exists in most computer architectures.

#canvas(length: 15pt, {
    import cetz.draw: *

    content((-9.9, 3.1), [#set text(1.5em); (R U) × ???])
    cube("oybbwgywr grwggwggb grobrgyob", offset: (-9.9, 0))
    content((-2.9, 3.1), [#set text(1.5em); (R U) × #underline()[0]])
    cube("wwwwwwwww ggggggggg rrrrrrrrr", offset: (-2.9, 0))
})

=== Multiple numbers <multiple-numbers>

If you think about what programs you could actually execute with just a single number and a "jump if zero" instruction, it would be almost nothing. It would be impossible for `solved-goto` jumps to be taken without erasing all data stored on the cube. What would be wonderful is if we could represent _multiple_ numbers on the cube at the same time.

Something cool about Rubik's Cubes is that it's possible for a long sequence of moves to only affect a small part of the cube. For example, we showed in the introduction an algorithm (L2 D2 L' U' L D2 L' U L') which cycles three corners. Therefore, it should be possible to represent two numbers using two algorithms that affect distinct "areas" of the cube.

The simplest example of this are the algorithms (U) and (D'). You can see that (U) and (D') both allow representing numbers up to three, and since they affect different areas of the cube, we can represent _two different_ numbers on the cube at the _same time_. We call these "registers", as an analogy to the concept in classical computing.

#canvas(length: 15pt, {
    import cetz.draw: *

    content((-9.9, 3.1), [#set text(1.5em); (0, 0)])
    cube("wwwwwwwww ggggggggg rrrrrrrrr", offset: (-9.9, 0))
    content((-4.9, 3.1), [#set text(1.5em); (1, 0)])
    cube("wwwwwwwww rrrgggggg bbbrrrrrr", offset: (-4.9, 0))
    content((0.1, 3.1), [#set text(1.5em); (0, 1)])
    cube("wwwwwwwww ggggggrrr rrrrrrbbb", offset: (0.1, 0))
    content((5.1, 3.1), [#set text(1.5em); (1, 1)])
    cube("wwwwwwwww rrrgggrrr bbbrrrbbb", offset: (5.1, 0))
    content((10.1, 3.1), [#set text(1.5em); (3, 2)])
    cube("wwwwwwwww ooogggbbb gggrrrooo", offset: (10.1, 0))
    content((15.1, 3.1), [#set text(1.5em); (1, 3)])
    cube("wwwwwwwww rrrgggooo bbbrrrggg", offset: (15.1, 0))
})

As described, `solved-goto` would only branch if the entire cube is solved, however since each algorithm affects a distinct area of the cube, it's possible for a human to determine whether a _single_ register is zero, by inspecting whether a particular section of the cube is solved. Remember that "solved" means that all of the stickers are the same color as the corresponding center.

#canvas(length: 15pt, {
    import cetz.draw: *

    content((-9.9, 3.1), [#set text(1.5em); (0, ?)])
    cube("wwwwwwwww ggggggrrr rrrrrrbbb", offset: (-9.9, 0))
    content((-4.9, 3.1), [#set text(1.5em); (?, 0)])
    cube("wwwwwwwww bbbgggggg ooorrrrrr", offset: (-4.9, 0))
})

For the first cube in the above figure, it's easy to tell that the first register is zero because the entire top layer of the cube is solved. We can modify the "solved-goto" instruction to input a list of pieces, all of which must be solved for the branch to be taken, but not necessarily any more. The following illustrates a successful `solved-goto UF UFR` instruction that would require jumping to a different part of the program, as well as an unsuccessful one that would require going to the next instruction.

#diagram(scale(30%, reflow: true, image("../../media/paper/Goto States.png")))

Can we do better than two registers with four states? In fact we can! If you try out the algorithms `R' F' L U' L U L F U' R` and `U F R' D' R2 F R' U' D`, you can see that they affect different pieces and both have order ninety. You may notice that they both twist the DBL corner; this is not a problem because they are independently decodable even ignoring that corner. One of the biggest challenges in the development of qter has been finding sets of algorithms with high orders that are all independently decodable. This is the fundamental problem that the Qter Architecture Solver attempts to solve, and will be discussed in later sections.

#canvas(length: 15pt, {
    import cetz.draw: *

    content(((-9.9 + -4.9) / 2, 3.1), [#set text(1.2em); R' F' L U' L U L F U' R #h(0.4em) (1, 0)])
    cube("obwywwwgw bwggggggg rrbrrbrrr", offset: (-9.9, 0))
    cube("orgobwbbo yybyywyyy yowboooor", offset: (-4.9, 0), back: true)
    content(((2.1 + 7.1) / 2, 3.1), [#set text(1.2em); U F R' D' R2 F R' U' D #h(0.4em) (0, 1)])
    cube("wwwwwywwg ggrwgyggb wgrbrryyy", offset: (2.1, 0))
    cube("bbbbbbooy ggoryyrrr boooooyro", offset: (7.1, 0), back: true)
})

Another fun thing that tweaking the "solved-goto" instruction in this way allows us to do is test whether the current value of a register is divisible by a particular set of numbers. For example, returning to the register defined by $R U$, we can test divisibility by three by looking at the the UFR corner.

#canvas(length: 15pt, {
    import cetz.draw: *

    content((-2.5, 3.1), [#set text(1.5em); R U])
    cube("wwwwwwggg rrrggyggy wbbrrrrrr", offset: (-2.5, 0), name: "1x")
    content((2.5, 3.1), [#set text(1.5em); $("R U")^3$])
    cube("yggywwbbw rrgggwggo rgyrrobbo", offset: (2.5, 0), name: "3x")

    circle("1x.center", radius: 1)
    circle("3x.center", radius: 1)
})

You can see that that piece resolves itself _before_ the rest of the register does, allowing us to check divisibility by three. This will be further elaborated on in the #link("/theory/introduction.html")[Theory] chapter.

All of the concepts described actually apply to other so-called "twisty puzzles", for example the Pyraminx, which is essentially a pyramid shaped Rubik's Cube. Only the notation and algorithms would have to change. For the rest of the paper, we will just look at the 3x3x3 because that is what most people are familiar with.

This is in fact all that's necessary to do things like calculating Fibonacci and performing multiplication. So now, how can we represent Qter programs?

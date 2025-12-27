Ok so, here's an overview of the qter repository and compilation pipeline. Forgive me if this is TMI, feel free to dump this into chatgippity and ask for a TLDR.

# First, the packages:

`qter_core` defines a variety of common types, implementations of mathematical functions, and code to operate on twisty puzzles.
`cli` implements the command line interface for qter
`compiler` implements the compilation process from `QAT` to the intermediate representation defined in `qter_core/runtime.rs`
`interpreter` implements an interpreter of the intermediate representation

The other packages are more math heavy and you won't have to worry about them unless you want to:

`puzzle_geometry` allows a twisty puzzle to be defined as a polyhedron with cut planes. It's mostly computational geometry, and converts a geometrical representation of a puzzle into a permutation group representation of the puzzle
`cycle_combination_solver` implements the search process for algs to encode registers
`movecount_coefficient_calculator` implements a metric for how "easy" it is for a human to perform an algorithm (sequence of moves) on a rubik's cube. For example, turns on the back face are harder to do than turns on the top face, though it gets more complicated than that.

# Second, some important technical details not mentioned in the readme:

Note that when I wrote the compiler, we hadn't had the idea of memory tapes yet and instead we were planning on implementing a system where the `.registers` declaration would be allowed to be placed anywhere and execution of the declaration would push puzzles to a global stack. We decided that memory tapes were better because this system only makes qter equivalent to a pushdown automaton whereas memory tapes make it equivalent to a turing machine. The point is that many of the types as well as the language specification assume that `.registers` can appear anywhere. At some point we will have to get rid of that but if you see strange type definitions, this may be why. Thankfully the code only partially supported this so there will not be too much to remove.

Another concept not mentioned in the readme that is important to understanding the compiler is architecture switching. The concept is basically that if a register is zero, then all of its pieces are solved. What you can do is take all of those pieces and do a different algorithm to affect them such that the register has a different order. Say your program has been operating in the (30, 30, 30) architecture but at a particular point in the computation needs to represent a number greater than 30. What it can do is zero out two of the 30 order registers, and reuse those pieces for a single 180 order register. Note that the process will actually involve tranferring the information in the 30 order register to a new 30 order register since the structure of the new 30 order register needs to be different for the 180 order register to fit but that's more on the technical side. This is unimplemented for now but it prevents the compiler from making as many assumptions as you might think it could.

We also thought of new instructions since writing the readme that will help humans read the Q file. `solve-puzzle` asks the human to bring the puzzle to the solved state, and `repeat-until` asks the human to repeat a sequence of moves until particular pieces are solved.

QAT has syntax for defining procedural macros using inline Lua code that is executed by the compiler. Basically if you see lua stuff, that's what's going on. If you want to see the syntax for this, you can look in `src/qter_core/prelude.qat` which defines the standard library of macros.

For strings, we use a thing called `ArcIntern` which is a reference counting pointer that automatically deduplicates equal instances. This way, equality, hashing, and cloning can be done just on the pointer so are very efficient. The code will even regularly use the entire file contents as a proxy for the file itself.

# Third, the compilation pipeline:

The pipeline starts in the CLI when the user types in something line `qter interpret file.qat`. If you like, you can `cd` into `src/cli`, copy/paste the example in the qter readme into a .qat file, and do `cargo run -- interpret file.qat`. You should be able to run the program. Most of the CLI is unimplemented right now so it only supports interpretation of qat files. The CLI will call into the compiler package to transform your program into the intermediate representation.

The first step is to parse your code. This is done using a library called `pest`, which allows you to define a grammar for a language and it transforms that into parsing code. The QAT language is defined at `src/compiler/qat.pest`. `pest` will return an untyped syntax tree, so the next step is to transform that syntax tree into meaningful, typed data. `compiler/src/parsing.rs` performs this task. It also takes all of the inline lua and merges it into a lua program.

The next step is to perform macro expansion, which is done in `compiler/src/macro_expansion.rs`. This is for the most part unimplemented, except for the "builtin macros" which have a one-to-one mapping with Q instructions. The builtin macros are defined in `compiler/src/builtin_macros.rs`, which contains functions that each manually parse the macro arguments and turn them into instructions.

Now that we have flattened the program into a string of labels and instructions, the final step is to clean everything up and to perform optimizations. This is done in `compiler/src/strip_expanded.rs`. You may notice that the intermediate representation does not have labels, and instead jump instructions directly jump to instruction indices. This is the step that substitutes label names for instruction indices. It will also perform validation and optimizations ahead of that. Right now, the only thing it does is coalesce consecutive add instructions. On a Rubik's cube, any position can be solved in <=20 moves, therefore a long string of additions may be coalesced into a single sequence of <=20 moves. Right now it implements coalescing but not optimizing of move count, just appending the sequences together. It will eventually implement prevention of jumping to a location that assumes a different architecture, analysis of whether conditional jumps are guaranteed and removal of dead code, and searching for particular code patterns that implement the `solve-puzzle` or `repeat-until` instructions.

That concludes the compilation pipeline as is implemented right now. To complete the compilation pipeline, the last step would be assembling the instructions into a .Q file. This should be fairly easy, a simple series of `writeln!` calls. Parsing the .Q file will also be necessary for interpreting it but this is also unimplemented.

# Fourth, the interpreter:

The interpreter is comparatively simple, it just runs through the instructions as outputted by the compiler. The `step` function returns a trace of what it is doing to allow for the future implementation of a debugger in the CLI. Right now, the CLI is capable of dumping this output to allow it to be given to a cubing robot. There's a fairly mathy optimization I would like to make to the interpreter that I can explain if you would like.

I think that's a pretty good technical overview of the entire system.

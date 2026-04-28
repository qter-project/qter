#import "../book.typ": book-page

#show: book-page.with(title: "Memory Tapes")

Note that this is not implemented in the compiler yet; it won't work if you try it!

Now we're getting to the more theoretical side, as well as into a design space that we're still exploring. Things can easily change.

There are plenty of cool programs one can write using the system described above, but it's certainly not Turing complete. The fundamental reason is that we only have finite memory... For example it would be impossible to write a QAT compiler in QAT because there's simply not enough memory to even store a whole program on a Rubik's Cube. In principle, anything would be possible with infinite Rubik's Cubes, but it wouldn't be practical to give all of them names since you can't put infinite names in a program. How can we organize them instead?

The traditional solution to this problem that is used by classical computers is _pointers_. You assign every piece of memory a number and allow that number to be stored in memory itself. Each piece of memory essentially has a unique name — its number — and you can calculate which pieces of memory are needed at runtime as necessary. However, this system won't work for qter because we would like to avoid requiring the user to manually decode registers outside of halting. We allow the `print` instruction to exist because it doesn't affect what the program does and can simply be ignored at the user's discretion.

Even if we did allow pointers, it wouldn't be a foundation for the usage of infinite memory. The maximum number that a single Rubik's Cube could represent if you use the whole cube for one register is 1259. Therefore, we could only possibly assign numbers to 1260 Rubik's Cubes, which would still not be nearly enough memory to, say, compile a QAT program using Qter.

Since our language is so minimal, we can take inspiration from perhaps the most famous barely-Turing-complete language out there (sorry in advance)... Brainfuck!! Brainfuck consists of an infinite list of numbers and a single pointer (stored externally) to the "current" number that is being operated on. A Brainfuck program consists of a list of the following operations:

- `>` Move the pointer to the right
- `<` Move the pointer to the left
- `+` Increment the number at the pointer
- `-` Decrement the number at the pointer
- `.` Output the number at the pointer
- `,` Input a number and store it where the pointer is
- `[` Jump past the matching `]` if the number at the pointer is zero
- `]` Jump to the matching `[` if the number at the pointer is non-zero

The similarity to Qter is immediately striking: we can give Qter an infinite list of cubes, call it a _memory tape_, and provide instructions to move left and right. That would make it Turing-complete by making it work essentially like Brainfuck. Now Brainfuck is intentionally designed to be a "Turing tarpit" and to make writing programs as annoying as possible, but we don't want that. For the sake of our sanity, we support having multiple memory tapes and naming them, so you don't have to think about potentially messing up other pieces of data while traversing for something else. To model a tape in a hand-computation of a qter program, one could have a bunch of Rubik's Cubes on a table laid out in a row and a physical pointer like an arrow cut out of paper to model the pointer. One could also model the pointer without an arrow by setting the currently pointed-to Rubik's Cube aside.

Lets see how we can tweak Q and QAT to interact with memory tapes. First, we need a way to declare them in both languages. In Q, you can write

```l
Puzzles
tape A: 3x3
```

to mark A as a _tape_ of 3x3s rather than just one 3x3. In QAT, you can write

```janet
.registers {
    tape X ~ A, B ← 3x3 builtin (90, 90)
}
```

to declare a memory tape X of 3x3s with the 90/90 architecture. Equivalently, you can replace the `tape` keyword with the '📼' emoji in both contexts:

```l
Puzzles
📼 A: 3x3
```

```janet
.registers {
    📼 X ~ A, B ← 3x3 builtin (90, 90)
}
```

In Q, we need syntax to move the tape left and right, equivalent to `<` and `>` in Brainfuck. As with multiple Rubik's Cubes, tapes are switched between using the `switch` instruction, and any operations like moves or `solved-goto` will apply to the currently pointed-to Rubik's Cube.

- `move-left [<number>]`

Move the pointer to the left by the number of spaces given, or just one space if not specified

- `move-right [<number>]`

Move the pointer to the right by the number of spaces given, or just one space if not specified

In QAT, tapes can be operated on like...

```janet
.registers {
    📼 X ~ A, B ← 3x3 builtin (90, 90)
}

add X.A 1         -- Add one to the `A` register of the currently selected Rubik's Cube on the `X` tape

move-right X 1    -- Move to the right
print "A is" X.A  -- Prints `A is 0` because we added one to the cube on the left

move-left X 1     -- Move to the left
print "A is" X.A  -- Prints `A is 1` because this is the puzzle that we added one to before
```

We poo-pooed pointers previously, however this system is actually powerful enough to implement them using QAT's metaprogramming functionality, provided that we store the current head position in a register external to the tape. The following `deref` macro moves the head to a position specified in the `to` register, using the `current` register to track the current location of the head.

```janet
.macro deref {
    ($tape:tape $current:reg $to:reg) => {
        -- Move the head to the zero position
        while not-solved $current {
            dec $current
            move-left $tape
        }

        -- Move the head to `to`
        while not-solved $to {
            dec $to
            inc $current
            move-right $tape
        }
    }
}
```

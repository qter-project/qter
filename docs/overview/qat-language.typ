#import "../book.typ": book-page, diagram

#show: book-page.with(title: "QAT Language")

Q would be very difficult to create programs in by hand, similarly to how it is difficult to write programs in machine code directly. Therefore, we created a high-level programming language called _QAT_ (Qter Assembly Text) that is designed to make it easy to write meaningful Qter programs. 

=== Global variables

Every QAT program begins with a `.registers` statement, used to declare global variables named registers. The statement in the above average program declares two global registers of size 90 to be stored on a Rubik's Cube. That is, additions operate modulo 90: incrementing a register of value 89 resets it back to 0, and decrementing a register of value 0 sets it to 89.

The `builtin` keyword refers to the fact that valid register sizes are specified in a puzzle-specific preset. For the Rubik's Cube, all builtin register sizes are in [src/qter_core/puzzles/3x3.txt](src/qter_core/puzzles/3x3.txt). Unlike traditional computers, qter is only able to operate with small and irregular register sizes.

You can choose to use larger register sizes at the cost of requiring more puzzles. For example, 1260 is a valid builtin register size that needs an entire Rubik's Cube to declare. If your program wants access to more than one register, it would have to use multiple Rubik's Cubes for more memory.

```janet
.registers {
    A <- 3x3 builtin (1260)
    B <- 3x3 builtin (1260)
    ...
}
```

To access the remainder of a register as explained in #link("/overview/what-is-qter.html#label-Multiple numbers")[What is Qter], you can write, for example, `A%3` to access the remainder after division by three.

The `.registers` statement is also used to declare memory tapes, which help facilitate local variables, call stacks, and heap memory. This idea will be expanded upon in #link("/overview/memory-tapes.html")[Memory Tapes].

=== Basic instructions

The basic instructions of the QAT programming language mimic an assembly-like language. If you have already read #link("/overview/q-language.html")[Q Language], you should be able to notice the similarities.

- `add <variable> <number>`

Add a constant number to a variable. This is the only way to change the value of a variable.

- `goto <label>`

Jump to a label, an identifier used to mark a specific location within code. The syntax for declaring a label follows the common convention amongst assembly languages:

```janet
infinite_loop:
    goto infinite_loop
```

- `solved-goto <variable> <label>`

Jump to a label if the specified variable is zero.

- `input <message> <variable>`

Ask the user for numeric input, which will be added to the given variable.

- `print <message> [<variable>]`

Output a message, optionally followed by a variable's value.

- `halt <message> [<variable>]`

Terminate the program with a message, optionally followed by a variable's value.

=== Metaprogramming

As described, QAT is not much higher level than Q... Ideally we need some kind of framework to allow abstraction and code reuse. Due to the fact that Rubik's Cubes have extremely limited memory, we cannot maintain a call stack in the way that a classical computer would. Therefore, we cannot incorporate functions into QAT. Instead, we have a Rust-inspired macro system where invocations of a macro automatically copy/paste the macro definition into the call site.

==== Defines

The simplest form of this provided by QAT is the `.define` statement, allowing you to define a variety of global constants.

```janet
.define PI 3          -- Global Integer
.define ALSO_PI $PI   -- Reference a previous define statement
.define ALSO_A A      -- Save an identifier
.define DO_ADDITION { -- Name a whole code block
    add A 10
}

add A $PI
add $ALSO_A $ALSO_PI
$DO_ADDITION
-- `A` will store the number 16
```

However, this is most likely too simple for your use case...

==== Macros

Macros roughly have the following syntax:

```janet
.macro <name> {
    (<pattern>) => <expansion>
    (<pattern>) => <expansion>
    ...
}
```

As a simple example, consider a macro to increment a register:

```janet
.macro inc {
    ($R:reg) => add $R 1
}
```

You would invoke it like

```janet
inc A
```

and it would be transformed at compile time to

```janet
add A 1
```

In the macro definition, `$R` represents a placeholder that any register could take the place of.

Now consider a more complicated macro, one to move the value of one register into another:

```janet
.macro move {
    ($R1:reg to $R2:reg) => {
        loop:
            solved-goto $R1 finished
            dec $R1
            inc $R2
            goto loop
        finished:
    }
}
```

You would invoke it like

```janet
move A to B
```

The word `to` is simply an identifier that must be matched for the macro invocation to compile. It allows you to make your macros read like english. This invocation would be expanded to

```janet
loop:
    solved-goto A finished
    dec A
    inc B
    goto loop
finished:
```

which would be expanded again to

```janet
loop:
    solved-goto A finished
    sub A 1
    add B 1
    goto loop
finished:
```

The expansion of `sub` will depend on the size of register A, and we'll see how to define the `sub` macro later.

Labels in macros will also be unique-ified so that if you invoke `move` twice, the labels will not conflict. This will also prevent you from jumping inside the macro invocation from outside:

```janet
move A to B
goto finished // Error: the `finished` label is undefined
```

Already, we have created a powerful system for encapsulating and abstracting code, but we still have to perform control flow using manual labels and jumping. Can we extend our macro system to allow defining control flow? In fact, we can! We can define an `if` macro like

```janet
.macro if {
    (solved $R:reg $code:block) => {
            solved-goto $R do_if
            goto after_if
        do_if:
            $code
        after_if:
    }
}
```

and we can invoke it like

```janet
if solved A {
    // Do something
}
```

which would be expanded to

```janet
    solved-goto A do_if
    goto after_if
do_if:
    // Do something
after_if:
```

Here, `$code` is a placeholder for an arbitrary block of code, which allows defining custom control flow. The unique-ification of labels also covers code blocks, so the following wouldn't compile:

```janet
if solved A {
    goto do_if // Error: the `do_if` label is undefined
}
```

Let's try defining a macro that executes a code block in an infinite loop:

```janet
.macro loop {
    ($code:block) => {
        continue:
            $code
            goto continue
        break:
    }
}
```

We can invoke it like

```janet
loop {
    inc A
}
```

but how can we break out of the loop? It would clearly be desirable to be able to `goto` the `continue` and `break` labels that are in the macro definition, but we can't do that. The solution is to mark the labels public, like

```janet
.macro loop {
    ($code:block) => {
        !continue:
            $code
            goto continue
        !break:
    }
}
```

The exclamation mark tells the compiler that the label should be accessible to code blocks inside the macro definition, so the following would be allowed:

```janet
loop {
    inc A

    if solved A {
        goto break
    }
}
```

However, the labels are not public to the surroundings of the macro to preserve encapsulation.

```janet
loop {
    -- Stuff
}
goto break -- Error: the `break` label is undefined
```

==== Rhai Macros

For situations where macros as described before aren't expressive enough, you can embed programs written in #link("https://rhai.rs/")[Rhai] into your QAT code to enable compile-time code generation. Lets see how the `sub` macro can be defined:

```janet
.start-rhai
    fn subtract_order_relative(r1, n) {
        return [ "add", r1, (-n) % r1.order ];
    }
end-rhai

.macro sub {
    ($R:reg $N:int) => rhai subtract_order_relative($R, $N)
}
```

`rhai` is a special statement that allows you to call a rhai function at compile-time, and the code returned by the function will be spliced in its place. Rhai macros should return a list of instructions, each of which is itself a list containing the instruction name and arguments.

Here, invoking the `sub` macro will invoke the rhai code to calculate what the `sub` macro should actually emit. In this example, the rhai macro accesses the size of the register to calculate which addition would cause it to overflow and wrap around, having the effect of subtraction. It would be impossible to do that with simple template-replacing macros.

In general, you can write any rhai code that you need to in order to make what you need to happen, happen. There's a bit of extra functionality that QAT gives Rhai access to.

```rhai
big(number) -> bigint // Takes in a standard lua number and returns a custom bigint type that is used for register orders and instructions
reg.order -> bigint // Provides the order of the register inputted to it
```

If the Rhai code throws an error, compilation will fail.

You can also invoke Rhai code in define statements:

```janet
.start-rhai
    fn bruh() {
        return 5;
    }
end-rhai

.define FIVE rhai bruh()
```

==== Importing

Finally, it is typically desirable to separate code between multiple files. QAT provides an `import` statement that brings all defines and macros of a different QAT file into scope, and splices any code defined in that file to the call site.

```janet
-- file-a.qat

.registers {
    A <- 3x3 builtin (1260)
}

add A 1
import "./file-b.qat"
thingy A

halt A
```

```janet
-- file-b.qat

add A 12

.macro thingy {
    ($R:reg) => {
        add $R 10
    }
}
```

Compiling and executing `file-a.qat` would print `23`.

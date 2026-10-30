#import "../book.typ": book-page, diagram

#show: book-page.with(title: "QAT Language")

Q would be very difficult to create programs in by hand, similarly to how it is difficult to write programs in machine code directly. Therefore, we created a high-level programming language called _QAT_ (Qter Assembly Text) that is designed to make it easy to write meaningful Qter programs. 

=== Global variables

Every QAT program begins with a `.registers` statement, used to declare global variables named registers. For example:

```
.registers {
    A, B <- 3x3 builtin (90, 90)
}
```


This declares two global registers named `A` and `B` each of size `90` to be stored on a standard `3x3` Rubik's Cube. That is, additions operate modulo 90: incrementing a register of value 89 resets it back to 0, and decrementing a register of value 0 sets it to 89.

The `builtin` keyword tells the compiler that the register orders refer to a particular preset architecture built into the compiler. For the standard Rubik's Cube, the builtin architectures are

- `(1260)`
- `(90, 90)`
- `(210, 24)`
- `(30, 30, 30)`
- `(30, 18, 10, 9)`

The distribution reflects the fact that a Rubik's cube has only a small amount of space. If we want to fit more registers in to the same amount of space, each register must be smaller. If you want more memory, you can declare that your program requires more than one Rubik's cube:

```janet
.registers {
    A <- 3x3 builtin (1260)
    B <- 3x3 builtin (1260)
    ...
}
```

If you want to define custom non-built-in registers, you can type algs directly into the declaration. The compiler will figure out how big each register is.

```
.registers {
    A, B <- 3x3 (U, D)
}
```

The `.registers` statement is also used to declare memory tapes, which help facilitate local variables, call stacks, and heap memory. This idea will be expanded upon in #link("/overview/memory-tapes.html")[Memory Tapes].

=== Basic instructions

The basic instructions of the QAT programming language mimic an assembly-like language. If you have already read #link("/overview/q-language.html")[Q Language], you should be able to notice the similarities.

- `add <variable> <number>`

Add a constant number to a variable.

- `sub <variable> <number>`

Subtract a constant number from a variable.

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

==== Register modifiers

===== Negative Alias

QAT exposes some of the mathematical structure underlying individual registers, allowing you to write faster programs. First, it is possible to express the _negative alias_ of a register:

```
add -A 1
```

`-A` holds the _additive inverse_ of `A`. For example, if `A` was a register of order 90 and held the value 10, then `-A` would hold the value `80`. Those sum to 90, which is equal to zero since the register is order 90.

Note that the negative alias is not doing any computation; it's showing the same register to you in a different way. Since `A` + `-A` is always a fixed constant, adding one to `-A` must have the effect of subtracting one from `A`, and subtracting one from `-A` must add one to `A`. Therefore, the statement `add -A 1` will be compiled to `sub A 1`, and likewise `sub -A 1` will compile to `add A 1`.

===== Modulus

In our section describing #link("/overview/what-is-qter.html#label-Multiple%20numbers")[multiple numbers], we claimed that doing `solved-goto` with only a subset of the pieces allows you to determine divisibility by particular numbers. In QAT, this is the syntax for it:

```
// Is A zero?
solved-goto A label1

// Is A divisible by 9?
solved-goto A%9 label2
```

This might compile down to:

```
1 | solved-goto UBL UB 5
2 | solved-goto UBL 7
```

If you ask to determine divisibility by a number that doesn't work, the compiler will tell you the list of numbers that do work:

```
Error:
   ╭─[ /tmp/code.qat:8:13 ]
   │
 8 │ solved-goto A%8 label1
   │             ┬
   │             ╰── Could not find a set of pieces for solved-goto that give divisibility by 8. The available moduli are the LCMs of all combinations of: 2, 9, 10
───╯
```

Note that we can interpret `A%9` as holding the remainder after division by `9`. If we did `halt A%9`, the halt instruction would do repeated subtraction until `A` was divisible by `9`, giving you the remainder. This is why we call it the _modulus_, it gives you the register _modulo_ some constant.

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

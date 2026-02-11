#let colors = (
    bg: rgb("#2f3052"),
    white: rgb("#d8d9ff"),
    yellow: rgb("#e1e485"),
    blue: rgb("#8498f0"),
    green: rgb("#2cda9d"),
    orange: rgb("#e4b37f"),
    red: rgb("#d86f9a"),
)

#show raw: it => {
    set text(size: 0.35in)

    show regex("\/\/.*\n"): it => {
        set text(fill: colors.blue)
        it
    }

    // set text(fill: color.yellow)
    text(it.text)
}

// #set page(width: 24in, height: 36in, fill: black, margin: 0in)
#set page(width: 24in, height: 36in, fill: colors.bg, margin: 0in)

#set text(fill: colors.white, font: "Martian Mono", size: 0.35in, weight: "extrabold")
// #set text(fill: colors.white, font: "Monaspace Neon", size: 0.4in)

#let bordered-text(size, c) = {
    set text(size)

    box(height: size * 0.75)[
        #place(dx: 0pt, dy: 0pt, [
            #set text(stroke: (paint: colors.bg, thickness: 0.2in))
            #c
        ])
        #place(dx: 0pt, dy: 0pt, c)
    ]
}

#box(width: 100%, inset: (top: 0.2in, bottom: 0.2in), box(fill: colors.red, width: 100%, inset: 0.5in)[
    #bordered-text(1.3in)[
        #set text(weight: "semibold", font: "Monaspace Xenon")
        _Write Your Own Qter Program_
    ]
])

// #box(width: 100%, fill: black, stroke: (
//     top: white,
//     bottom: white,
// ), inset: 0.3in)[
//     #set text(size: 0.8in, weight: "semibold", font: "Monaspace Xenon")
//     Step Zero: Run Some Existing Programs
// ]

#box(width: 100%, fill: colors.green, inset: 0.3in)[
    #bordered-text(0.8in)[
        #set text(weight: "semibold", font: "Monaspace Xenon")
        Step One: Choose a Register Architecture
    ]
]

#box(inset: (left: 0.5in, right: 0.5in))[
    Declaring a register architecture allows you to choose how to allocate the space available on the cube.

    #grid(
        columns: 2,
        column-gutter: 1in,
    )[
        ```qat
        // Syntax to declare a register architecture
        .registers {
            A, B, C <- 3x3 builtin (30, 30, 30)
        }
        // You can also supply move sequences for each cycle
            A, B <- 3x3 (U, D')
        ```
    ][
        ```qat
        // Presets
        A          <- 3x3 builtin (1260)
        A, B       <- 3x3 builtin (90, 90)
        A, B       <- 3x3 builtin (210, 24)
        A, B, C    <- 3x3 builtin (30, 30, 30)
        A, B, C, D <- 3x3 builtin (30, 18, 10, 9)
        ```
    ]
]

#box(width: 100%, fill: colors.orange, inset: 0.3in)[
    #bordered-text(0.8in)[
        #set text(weight: "semibold", font: "Monaspace Xenon")
        Step Two: Learn the Instructions
    ]
]

#box(inset: (left: 0.3in, right: 0.3in))[

    #grid(
        columns: (1fr, 1fr, 1fr),
        column-gutter: 0in,
    )[
        Note that registers cannot be substituted for constants.

        == Fundamental
        ```qat
        // Add a constant to a register
        add A 5
        // Subtract from a register
        sub A 5
        // A label that can be jumped to
        spot:
        // Jump to a label
        goto spot
        // Jump if a register is zero
        solved-goto A spot
        // Accept input into a register
        input "Value:" A
        // Halt and output a register
        halt "The output is" A
        // Print to the interpreter output
        print "Computing XYZ next..."
        ```
        == Higher order
        ```qat
        // Increment a register
        inc A
        ```
    ][
        ```qat
        // Decrement a register
        dec A
        // Add A to B while erasing A
        move A to B
        // Erase A and set it to a constant
        set A to 10
        // Erase A and add A*N to B
        multiply A 10 at B
        ```
        == Control Flow
        ```
        if solved A {
            // Executed if A is zero
        } else {
            // Executed if A is non-zero
            // Else blocks are optional
        }

        if not-solved A { } else { }

        // Tests if a register is
        // equal to a constant
        if A equals 5 { } else { }

        if A not-equals 5 { } else { }
        ```
    ][
        ```qat
        loop {
            // This code repeats forever

            // You can jump to `break`
            // to exit the loop early
            goto break
            // You can jump to `continue`
            // to restart the loop from
            // the beginning
            goto continue
        }

        // `break` and `continue` labels are
        // available for all loops

        while solved A { }

        while not-solved A { }

        while A equals 5 { }

        while A not-equals 5 { }
        ```
    ]
]

#box(width: 100%, fill: colors.blue, inset: 0.3in)[
    #bordered-text(0.8in)[
        #set text(weight: "semibold", font: "Monaspace Xenon")
        Step Three: Write Some Macros
    ]
]

#box(inset: (left: 0.5in, right: 0.5in))[
    You can write _macros_ to encapsulate functionality that you need to reuse. Any invocation of the macro will be replaced with the macro's contents during compilation.

    #grid(
        columns: 2,
        column-gutter: 3in,
    )[
        ```qat
        // Implementation of `while A equals 5 { }`
        .macro while {
            ($R:reg equals $N:int $code:block) => {
                loop {
                    if $R not-equals $N {
                        goto break
                    }
                    $code
                }
            }
        }
        ```
    ][
        ```qat
        // This code...
        while A equals 6 {
            print "A is 6"
        }
        // gets replaced with...
        loop {
            if A not-equals 6 {
                goto break
            }
            print "A is 6"
        }
        ```
    ]
]

#box(
    width: 100%,
    fill: black,
    inset: 0.3in,
)[
    #set text(size: 0.8in, weight: "semibold", font: "Monaspace Xenon")
    Step Four: Try the Example Programs
]

#box(inset: (left: 0.5in, right: 0.5in))[
    The example programs and programs written by others contain lots of tricks that can help you solve problems.
]

#place(bottom, box(width: 100%, fill: colors.yellow, inset: 0.3in)[
    #bordered-text(0.8in)[
        #set text(weight: "semibold", font: "Monaspace Xenon")
        Step Five: Run and Save Your Program!
    ]
])

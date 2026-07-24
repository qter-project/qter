#import "book.typ": book-page

#show: book-page.with(title: "Introduction")

Qter is a computer architecture that allows humans to perform computations by manipulating the Rubik's Cube (or any twisty puzzle) by hand. Following is an example executable program that accepts an index as user input and computes the corresponding Fibonacci number, written in our custom twisty puzzle file format named Q. It can be physically executed by a human without needing to know how computers work.

`fib.q`
```l
Puzzles
A: 3x3

1 | input "Which Fibonacci number to calculate:"
           B2 U2 L F' R B L2 D2 B R' F L
           max-input 8
2 | solved-goto UFR 14
3 | D L' F L2 B L' F' L B' D' L'
4 | L' F' R B' D2 L2 B' R' F L' U2 B2
5 | solved-goto UFR 15
6 | repeat until DL DFL solved
            L U' B R' L B' L' U'
            L U R2 B R2 D2 R2 D'
7 | L' F' R B' D2 L2 B' R' F L' U2 B2
8 | solved-goto UFR 16
9 | repeat until FR DRF solved
            D' B' U2 B D' F' D L' D2
            F' R' D2 F2 R F2 R2 U' R'
10 | L' F' R B' D2 L2 B' R' F L' U2 B2
11 | solved-goto UFR 17
12 | repeat until UF solved
            B R2 D' R B D F2 U2 D'
            F' L2 F D2 F B2 D' L' U'
13 | goto 4
14 | halt "The number is: 0"
15 | halt until DL DFL solved
          "The number is"
          L D B L' F L B' L2 F' L D'
16 | halt until FR DRF solved
          "The number is"
          F2 L2 U2 D' R U' B L' B L' U'
17 | halt until UF solved
          "The number is"
          U L' R' F' U' F' L' F2 L U R
```

This was compiled from our custom high level programming language named QAT (Qter Assembly Text):

`fib.qat`

```l
.registers {
    A, B, C, D <- 3x3 builtin (30, 18, 10, 9)
}

.macro fib-shuffle {
    // Let `fib(n)` be the nth fibonacci number
    // Expects $R1 = fib(n), $R2 = fib(n-1), $R3 = 0
    // Sets the registers to $R1 = 0, $R2 = fib(n+1), $R3 = fib(n) by adding $R1 to $R2 and $R3
    ($R1:reg $R2:reg $R3:reg $counter:reg) => {
        dec $counter
        if solved $counter {
            halt "The number is" $R1
        }
        while not-solved $R1 {
            dec $R1
            inc $R2
            inc $R3
        }
    }
}

input "Which Fibonacci number to calculate:" D
if solved D {
    halt "The number is 0"
}
inc B
loop {
    fib-shuffle B A C D
    fib-shuffle A C B D
    fib-shuffle C B A D
}
```


This book is intended to comprehensively describe Qter and how we created it. It has the following chapters:

#table(
    columns: 3,
    [], [*Description*], [*Background required*],
    [*Overview*], [What Qter is and how you can play with it], [Basic programming],
    [*Theory*], [Mathematics and algorithms behind encoding computations into move sequences], [Data structures & algorithms — discrete math],
    [*Technical Documentation*], [Implementation details of our software including lots of cool optimizations], [Software engineering],
    [*Blog*], [Stories from our journey creating this!], [None],
)

Note, most of the content in *Overview* and *Theory* was originally written for our #link("https://qter.dev/paper.pdf")[technical paper].

#html.script("
let params = new URLSearchParams(window.location.search)

document.getElementById(\"sidebar-close\").addEventListener(\"click\", () => {
    var newurl = window.location.protocol + \"//\" + window.location.host + window.location.pathname + '?mobile-close-hamburger=';
    window.history.pushState({path:newurl},'',newurl);
})

document.getElementById(\"sidebar-open\").addEventListener(\"click\", () => {
    var newurl = window.location.protocol + \"//\" + window.location.host + window.location.pathname;
    window.history.pushState({path:newurl},'',newurl);
})

if (!params.has(\"mobile-close-hamburger\")) {
    document.getElementById(\"sidebar-open\").click()
}
")

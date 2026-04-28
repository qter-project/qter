#import "../book.typ": book-page, diagram

#show: book-page.with(title: "Getting Started")

Qter has two programming languages:

#table(
    columns: 2,
    [], [],
    [Q], [An assembly-like language intended to be easy to follow by humans],
    [QAT], [A high-level language that compiles to Q and is intended to be written and understood]
)

To run a program, you would first write it in QAT and then use the _QAT compiler_ to compile it to Q.

#diagram(image("../../media/paper/Light Compilation Pipeline.png"))

= Installation

First, install Rust through #link("https://rustup.rs")[rustup] or your package manager. Second, run the following command to install Qter (TODO)

= Usage

Our CLI tool will allow you to compile and execute programs written in QAT. You can compile a program by executing the following command.

```bash
qter compile program.qat
# Will output the file `program.q` with the compiled code
```

To interpret and execute a program, you can run the following command.
```bash
qter interpret program.qat
```


#import "@preview/shiroa:0.4.0": *
#import "@preview/cetz:0.4.2"

#show: book

#book-meta(
  title: "Qter Documentation",
  repository: "https://github.com/qter-project/qter",
  repository-edit: "https://github.com/qter-project/qter/edit/main/docs/{path}",
  discord: "https://discord.gg/geEhJ6DjBb",
  summary: [
    #prefix-chapter("introduction.typ")[Introduction]

    = Overview

    - #chapter("./overview/rubiks-cube-theory.typ")[Rubik's Cube Theory]
    - #chapter("./overview/what-is-qter.typ")[What is Qter?]
    - #chapter("./overview/programming-with-qter.typ")[Getting Started]
      - #chapter("./overview/q-language.typ")[Q Language]
      - #chapter("./overview/qat-language.typ")[QAT language]
      - #chapter("./overview/standard-library.typ")[Standard Library]
      - #chapter("./overview/example-programs.typ")[Example Programs]
      - #chapter("./overview/memory-tapes.typ")[Memory Tapes]

    = Theory

    - #chapter("./theory/introduction.typ")[Introduction]
      - #chapter("./theory/group-theory.typ")[Group Theory]
      - #chapter("./theory/permutation-groups.typ")[Permutation Groups]
      - #chapter("./theory/parity-and-orientation-sum.typ")[Parity and Orientation Sum]
      - #chapter("./theory/cycle-structures.typ")[Cycle Structures]
        - #chapter("./theory/ori-pari-sharing.typ")[Orientation and Parity Sharing]
    - #chapter("./theory/qas.typ")[The Qter Architecture Solver]
      - #chapter("./theory/ccf.typ")[Cycle Combination Finder]
      - #chapter("./theory/ccs.typ")[Cycle Combination Solver]
        - #chapter("./theory/tree-searching.typ")[Tree Searching]
        - #chapter("./theory/pruning.typ")[Pruning]
          - #chapter("./theory/symmetry-reduction.typ")[Symmetry Reduction]
          - #chapter("./theory/pruning-table-types.typ")[Pruning Table Types]
        - #chapter("./theory/ida-star.typ")[IDA\* Optimizations]
        - #chapter("./theory/larger-puzzles.typ")[Larger Puzzles]
        - #chapter("./theory/fixed-pieces.typ")[Fixed Pieces]
      - #chapter("./theory/mcc.typ")[Movecount Coefficient Calculator]

    = Technical Documentation

    - #chapter("./technical-docs/cli.typ")[CLI]
    - #chapter("./technical-docs/compiler/compiler.typ")[Compiler]
      - #chapter("./technical-docs/compiler/qat-grammar.typ")[QAT Grammar]
    - #chapter("./technical-docs/interpreter.typ")[Interpreter]
    - #chapter("./technical-docs/ccf.typ")[Cycle Combination Finder]
    - #chapter("./technical-docs/ccs.typ")[Cycle Combination Solver]
    - #chapter("./technical-docs/robot.typ")[Robot]
      - #chapter("./technical-docs/robot/setup-process.typ")[Setup Process]
        - #chapter("./technical-docs/robot/software-setup.typ")[Software Setup]
        - #chapter("./technical-docs/robot/assemble-frame.typ")[Assembling the Frame]
        - #chapter("./technical-docs/robot/assemble-electronics.typ")[Assembling the Electronics]
        - #chapter("./technical-docs/robot/set-up-visualizer.typ")[Setting up Visualizer]
      - #chapter("./technical-docs/robot/electronics.typ")[Electronics]
      - #chapter("./technical-docs/robot/hardware-interfacing.typ")[Hardware interfacing]
      - #chapter("./technical-docs/robot/interpreter-interfacing.typ")[Interpreter interfacing]
  ]
)

// re-export page template
#import "templates/page.typ": project
#let book-page = project

#let diagram(c) = context {
    if target() == "html" {
        html.div(html.frame(c), class: "diagram", style: "background: white; padding: 1em;")
    } else { c }
}

#let canvas(..vals) = diagram(cetz.canvas(..vals))

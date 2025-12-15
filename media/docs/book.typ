#import "@preview/shiroa:0.3.1": *

#show: book

#book-meta(
  title: "Qter Documentation",
  repository: "https://github.com/qter-project/qter",
  repository-edit: "https://github.com/qter-project/qter/edit/main/media/docs/{path}",
  summary: [
    #prefix-chapter("introduction.typ")[Introduction]

    = Overview

    - #chapter(none)[Rubik's cube theory]
    - #chapter(none)[What is Qter?]
    - #chapter(none)[Coding with Qter]
      - #chapter(none)[Q language]
      - #chapter(none)[QAT language]
      - #chapter(none)[Memory tapes]

    = Theory

    - #chapter(none)[Introduction]
      - #chapter(none)[Group Theory]
      - #chapter(none)[Permutation Groups]
      - #chapter(none)[Parity and Orientation Sum]
      - #chapter(none)[Cycle Structures]
    - #chapter(none)[The Qter Architecture Solver]
      - #chapter(none)[Cycle Combination Finder]
      - #chapter(none)[Cycle Combination Solver]

    = Technical Documentation

    - #chapter("./technical-docs/cli.typ")[CLI]
    - #chapter("./technical-docs/compiler.typ")[Compiler]
    - #chapter("./technical-docs/interpreter.typ")[Interpreter]
    - #chapter("./technical-docs/ccf.typ")[Cycle Combination Finder]
    - #chapter("./technical-docs/ccs.typ")[Cycle Combination Solver]
    - #chapter("./technical-docs/robot.typ")[Robot]
      - #chapter("./technical-docs/robot/setup-process.typ")[Setup Process]
        - #chapter("./technical-docs/robot/checklist.typ")[Parts Checklist]
        - #chapter("./technical-docs/robot/install-qteros.typ")[Installing QterOS]
        - #chapter("./technical-docs/robot/assemble-frame.typ")[Assembling the Frame]
        - #chapter("./technical-docs/robot/assemble-electronics.typ")[Assembling the Electronics]
        - #chapter("./technical-docs/robot/set-up-visualizer.typ")[Set up Visualizer]
      - #chapter("./technical-docs/robot/electronics.typ")[Electronics]
      - #chapter("./technical-docs/robot/hardware-interfacing.typ")[Hardware interfacing]
      - #chapter("./technical-docs/robot/interpreter-interfacing.typ")[Interpreter interfacing]
  ]
)

// re-export page template
#import "templates/page.typ": project
#let book-page = project

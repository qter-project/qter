<!-- cspell:disable -->

# TODO

⭐means it's important to be done before the video series
😎means it's optional

- crashlog

## CCF

- possible orders reuse possible orders list if two orbits are the same
- can we remove constraints?

## CCS

- https://sebastiano.tronto.net/blog/2026-01-28-prefetch/
- solver.rs
  - 😎figure out move ordering dependence
  - F B cycle type is NOT checked!!
  - document
  - check for logs in test cases; tracing
  - hardcode the first solution moves in the test cases to be sure
- dont pack bit vector for AuxMem
- try out a different exact hasher
  - 3x3 https://github.com/Voltara/vcube/blob/9f5bc2cce18f29437879ace825f22917f6705378/src/cube.h#L240
  - any puzzle https://github.com/cubing/twsearch/blob/main/src/cpp/index.cpp
- spam debug_assert!()
- solve for all cycle structures from CCF at once vs many runs of single cycle structure at a time
- ⭐pruning table
  - generate table during the solve
    - if the number of probes exceeds the number of set values in the table by a certain factor (3x?) it considers generating another level of the table, if there is enough memory to do so
  - cartesian product tables
    - seed only one value
    - <https://discord.com/channels/772576325897945119/1326029986578038784/1347580846647017482>
  - fix storage backend initialization meta
  - approximate pruning table
    - reread this <https://discord.com/channels/@me/1399108854784065677/1431035660839555187>
  - tANS table compression
  - cycle type pruning table
    - with fewer state spaces, go back to an exact pruning table
    - generate the cycle type table before the approximate table and roughly guess the average pruning value
    - <https://discord.com/channels/772576325897945119/1326029986578038784/1374906236956573806>
  - ⭐each thread fills in 1024 entires at a time
  - ⭐exact: dfs at low levels instead of scanning
- search
  - ⭐stabilizer
    - Look into fixing a corner for even cubes/other puzzles
    - use this data structure https://discord.com/channels/772576325897945119/1326029986578038784/1414832577369342023
  - ⭐standard symmetry
    - ⭐Doug's canon_into function simplified and explained in #programming
    - ⭐reread kociemba's website and h48.md
    - densely pack symmcoords
      - <https://discord.com/channels/1007840975726575667/1407079970039267514/1414811607892230249>
  - ⭐multithreading
    - For example, for a 48-symmetric state, the search begins with the symmetry marker 48-symmetric. Before taking the first move, we determine which possible moves are possible based on the symmetry state. For this state, the first possible transitions are either U or U2; all other states are reachable through symmetry. Suppose we take U as the first move, resulting in an 8-symmetric state. Then, if we continue with this 8-symmetric state, the possible move are (R, R2, R', D, D2, D'). This approach reduces the search tree size to approximately 1/48th of its original size, and eliminates the need for specialized handling of various cases.
    - microthreading
- you NEED to account for parity constraints when calculating orbit size; address this in schreier sims
- Generate a pruning table starting from the scramble instead of the solved state and then began the search from the solved state
- ⭐solved state for 4x4
- ⭐antisymmetry
- 😎mulcmp3 and mul3 optimizations from twsearch
- 😎PGO
- 😎replace pub(crate) with getters
- can we use move tables? look into at the end
- 😎use *mut u8 instead of Box<[u8]> for generic puzzle

## Paper

- talk about multiplication in the paper

## QAT

- Replace repeat-untils that can never terminate with a `halt` instruction
- ⭐Rhai macros for QAT
  - Allow encoding/decoding all possible types of instructions
- 😎Translate QAT comments into Q
- ⭐Memory tapes
  - ⭐Implement in QAT
- Function macro
- Directory of testing programs instead of hardcoding into Rust
  - Inline testing in the QAT format
- 😎Write tree sitter grammar
- 😎Architecture switching
- 😎Instruction to copy solving moves to other puzzle
- 😎Architecture that avoids sharing a piece by always having two additions simultaneously which avoids parity
- 😎force conditional blocks that end with "halt" to codegen at the end of the instruction memory, optimizing a goto
- 😎Test with https://github.com/dtolnay/trybuild
- 😎Write a tree-sitter grammer for QAT
- 😎Syntax for declaring what moduli you want from the registers
- Evaluate repeat instruction for examinx
- 😎Optimizations
  - Convert QAT to SSA
    - Make the repeat-until vectorization optimization work with multiple puzzles
    - Unreachable removal with real control flow analysis
    - Dead code removal?
    - Coalesce adjacent labels
    - Strength reduction of `solved-goto` after a `repeat until` or `solve` that guarantees whether or not it succeeds
    - If there's a goto immediately after a label, move the label to where the goto goes to
    - Be able to actually shuffle code around to minimize branching
  - Asher's repeated move post process optimization: R U R repeated = R then U R2 repeated then R'

## Interpreter/CLI

- ⭐Implement tapes
- Debugging tool
- 😎Implementing the fancy CRT/loop-repetition-calculating thingy

## Q

- ⭐Compile to Q
  - ⭐"[repeat|print|halt] until _ solved" syntax
- Make solved-goto pieces "optimal" in a meaningful sense of "optimal"
- Parse Q

## End user

- ⭐Youtube videos
- ⭐Animation of the robot doing a computation

## Robot

- ⭐Add robot to the README

## Misc

- 😎Contributor badges
  - Arhan
  - Henry
  - Asher
  - Daniel
  - Neel
  - Tomas Rokicki
  - Purdue Hackers organizers collectively
  - Maybe
    - Kian (Q tree-sitter)
    - Lillith (Vulcan + designing something during 252 lab + python rewrite of something)
    - Hazel (soldering protoboard)
    - Sriram (robot stuff? idk)

## Docs

- Add cayleypy to the paper
- Add linear order growth to the paper

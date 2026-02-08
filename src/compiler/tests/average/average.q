Puzzles
A: 3x3

0  | input "First number:"
           R' F' L U' L U L F U' R
           max-input 89
1  | input "Second number:"
           U F R' D' R2 F R' U' D
           max-input 89
2  | solved-goto UR DFL 6
3  | B2 R L2 D L' F' D2 F' L2 B' U' R D' L' B2 R F
4  | solved-goto UBL UB 12
5  | goto 2
6  | solved-goto UBL UB 11
7  | R' U F' L' U' L' U L' F R
8  | solved-goto UBL UB 11
9  | F' R' B2 L D R' U B L2 F D2 F L D' L2 R' B2
10 | goto 6
11 | halt "The average is"
          D' U R F' R2 D R F' U'
          counting-until UR DFL
12 | repeat until DR solved
            B2 R L2 D L' F' D2 F'
            L2 B' U' R D' L' B2 R F
13 | repeat until UFR solved
            B2 R L2 D L' F' D2 F' L2 B' U'
            R D' L' B2 R F B2 R L2 D L' F'
            D2 F' L2 B' U' R D' L' B2 R F
14 | repeat until DFL solved
            B2 R L2 D L' F' D2 F' L2 B' U' R D'
            L' B2 R F B2 R L2 D L' F' D2 F' L2
            B' U' R D' L' B2 R F B2 R L2 D L'
            F' D2 F' L2 B' U' R D' L' B2 R F B2
            R L2 D L' F' D2 F' L2 B' U' R D' L'
            B2 R F B2 R L2 D L' F' D2 F' L2 B'
            U' R D' L' B2 R F B2 R L2 D L' F'
            D2 F' L2 B' U' R D' L' B2 R F
15 | repeat until UR solved
            B2 R L2 D L' F' D2 F' L2 B' U' R D' L'
            B2 R F B2 R L2 D L' F' D2 F' L2 B' U'
            R D' L' B2 R F B2 R L2 D L' F' D2 F'
            L2 B' U' R D' L' B2 R F B2 R L2 D L'
            F' D2 F' L2 B' U' R D' L' B2 R F B2 R
            L2 D L' F' D2 F' L2 B' U' R D' L' B2 R
            F B2 R L2 D L' F' D2 F' L2 B' U' R D'
            L' B2 R F B2 R L2 D L' F' D2 F' L2 B'
            U' R D' L' B2 R F B2 R L2 D L' F' D2
            F' L2 B' U' R D' L' B2 R F B2 R L2 D
            L' F' D2 F' L2 B' U' R D' L' B2 R F B2
            R L2 D L' F' D2 F' L2 B' U' R D' L' B2
            R F B2 R L2 D L' F' D2 F' L2 B' U' R
            D' L' B2 R F B2 R L2 D L' F' D2 F' L2
            B' U' R D' L' B2 R F B2 R L2 D L' F'
            D2 F' L2 B' U' R D' L' B2 R F B2 R L2
            D L' F' D2 F' L2 B' U' R D' L' B2 R F
            B2 R L2 D L' F' D2 F' L2 B' U' R D' L'
            B2 R F B2 R L2 D L' F' D2 F' L2 B' U'
            R D' L' B2 R F B2 R L2 D L' F' D2 F'
            L2 B' U' R D' L' B2 R F B2 R L2 D L'
            F' D2 F' L2 B' U' R D' L' B2 R F
16 | solved-goto UBL UB 21
17 | R' U F' L' U' L' U L' F R
18 | solved-goto UBL UB 21
19 | F' R' B2 L D R' U B L2 F D2 F L D' L2 R' B2
20 | goto 16
21 | U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D U F R' D' R2 F R' U' D
     U F R' D' R2 F R' U' D
22 | halt "The average is"
          D' U R F' R2 D R F' U'
          counting-until UR DFL

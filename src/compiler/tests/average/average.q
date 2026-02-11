Puzzles
A: 3x3

0  | input "First number:"
           R' F' L U' L U L F U' R
           max-input 89
1  | input "Second number:"
           U F R' D' R2 F R' D U'
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
          U D' R F' R2 D R F' U'
          counting-until UR DFL
12 | repeat until DR solved
            B2 R L2 D L' F' D2 F'
            L2 B' U' R D' L' B2 R F
13 | repeat until UFR solved
            B2 L U' F2 L U2 F' L F'
            L D' L B' R2 U2 R' B2 D
14 | repeat until DFL solved
            L U2 D F' R' U2 L2 D L2
            B' U2 D2 B' R2 L U F'
15 | repeat until UR solved
            U2 F2 U2 D2 F B L' U'
            F2 B2 U R' U' D F D'
16 | solved-goto UBL UB 21
17 | R' U F' L' U' L' U L' F R
18 | solved-goto UBL UB 21
19 | F' R' B2 L D R' U B L2 F D2 F L D' L2 R' B2
20 | goto 16
21 | B2 U R D2 R2 F L2 U2 D' B' R' F2 R2 D' L2 F
22 | halt "The average is"
          U D' R F' R2 D R F' U'
          counting-until UR DFL

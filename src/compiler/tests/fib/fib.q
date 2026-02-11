Puzzles
A: 3x3

0  | input "Which Fibonacci number to calculate:"
           B2 U2 L F' R B L2 D2 B R' F L
           max-input 8
1  | solved-goto UBL 3
2  | goto 4
3  | halt "The number is 0"
4  | D L' F L2 B L' F' L B' D' L'
5  | L' F' R B' D2 L2 B' R' F L' U2 B2
6  | solved-goto UBL 8
7  | goto 9
8  | halt "The number is"
          L D B L' F L B' L2 F' L D'
          counting-until UFL DL
9  | repeat until DL solved
            L U' B R' L B' L' U'
            L U R2 B R2 D2 R2 D'
10 | repeat until UFL solved
            U2 R U2 D2 L2 F U F' D
            R F L2 F2 L' F2 R' D B2
11 | L' F' R B' D2 L2 B' R' F L' U2 B2
12 | solved-goto UBL 14
13 | goto 15
14 | halt "The number is"
          U' F2 L2 D' U2 R U' B L' B L'
          counting-until FR DFR
15 | repeat until DFR solved
            D' B' U2 B D' F' D L' D2
            F' R' D2 F2 R F2 R2 U' R'
16 | repeat until FR solved
            D' L2 F U' F' U B2 L' B'
            L D' R U F' D F D F2 B
17 | L' F' R B' D2 L2 B' R' F L' U2 B2
18 | solved-goto UBL 20
19 | goto 21
20 | halt "The number is"
          U L' R' F' U' F' L' F2 L U R
          counting-until UB
21 | repeat until UB solved
            B R2 D' R B D F2 U2 D'
            F' L2 F D2 F B2 D' L' U'
22 | goto 5

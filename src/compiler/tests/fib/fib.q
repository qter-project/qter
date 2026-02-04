Puzzles
A: 3x3

0  | input "Which Fibonacci number to calculate:"
           B2 U2 L F' R B L2 D2 B R' F L
           max-input 8
1  | solved-goto UBL 3
2  | goto 4
3  | halt "The number is: 0"
4  | D L' F L2 B L' F' L B' D' L'
5  | L' F' R B' D2 L2 B' R' F L' U2 B2
6  | solved-goto UBL 8
7  | goto 9
8  | halt "The number is"
          L D B L' F L B' L2 F' L D'
          counting-until UFL DL
9  | repeat until UFL DL solved
            L U' B R' L B' L' U'
            L U R2 B R2 D2 R2 D'
10 | L' F' R B' D2 L2 B' R' F L' U2 B2
11 | solved-goto UBL 13
12 | goto 14
13 | halt "The number is"
          F2 L2 U2 D' R U' B L' B L' U'
          counting-until FR DFR
14 | repeat until FR DFR solved
            D' B' U2 B D' F' D L' D2
            F' R' D2 F2 R F2 R2 U' R'
15 | L' F' R B' D2 L2 B' R' F L' U2 B2
16 | solved-goto UBL 18
17 | goto 19
18 | halt "The number is"
          U L' R' F' U' F' L' F2 L U R
          counting-until UB
19 | repeat until UB solved
            B R2 D' R B D F2 U2 D'
            F' L2 F D2 F B2 D' L' U'
20 | goto 5

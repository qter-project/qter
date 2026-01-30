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

Puzzles
A: 3x3

0 | input "Number to modulus:"
          U R U' D2 B
          max-input 209
1 | U R' F U2 R' F L F2 L' F U' F' U R2 U2
2 | solved-goto UL UFL 1
3 | solved-goto UBL BL 6
4 | F L2 F' B' U D R' U2 R' U F U2 F D R U'
5 | goto 2
6 | repeat until UL UFL solved
           F L2 F' B' U D R' U2 R' U F U2 F D R U'
7 | B' L U R' U' B2 L' D2 B2 D2 B2
8 | halt "The modulus is"
         B' D2 U R' U'
         counting-until UBL BL

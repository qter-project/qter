Puzzles
A: 3x3

0 | input "Number to modulus:"
          U R U' D2 B
          max-input 209
1 | solved-goto UBL BL 7
2 | solved-goto UL UFL 4
3 | goto 5
4 | U R' F U2 R' F L F2 L' F U' F' U R2 U2
5 | F L2 F' B' U D R' U2 R' U F U2 F D R U'
6 | goto 1
7 | solved-goto UL UFL 9
8 | U2 R2 U' F U F' L F2 L' F' R U2 F' R U'
9 | halt "The modulus is"
         B U2 B' L' U2 B U L' B L B2 L
         counting-until UL UFL

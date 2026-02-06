Puzzles
A: 3x3

0  | input "Number to modulus:"
           U R U' D2 B
           max-input 209
1  | print "A is now"
           B' D2 U R' U'
           counting-until UBL BL
2  | U R' F U2 R' F L F2 L' F U' F' U R2 U2
3  | solved-goto UBL BL 7
4  | solved-goto UL UFL 1
5  | F L2 F' B' U D R' U2 R' U F U2 F D R U'
6  | goto 3
7  | repeat until UFL solved
            F L2 F' B' U D R' U2
            R' U F U2 F D R U'
8  | repeat until UL solved
            D' F2 B' D' F U2 R L F2
            U2 R' U2 D' L B R2 F R
9  | B' L U R' U' B2 L' D2 B2 D2 B2
10 | halt "The modulus is"
          B' D2 U R' U'
          counting-until UBL BL

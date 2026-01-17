Puzzles
A: 3x3

0  | input "Number to modulus:"
           U R U' D2 B
           max-input 209
1  | goto 2
2  | solved-goto UBL BL 2
3  | print "A is now"
           B' D2 U R' U'
           counting-until UBL BL
4  | U R' F U2 R' F L F2 L' F U' F' U R2 U2
5  | solved-goto UBL BL 9
6  | solved-goto UL UFL 3
7  | F L2 F' B' U D R' U2 R' U F U2 F D R U'
8  | goto 5
9  | repeat until UL UFL solved
            F L2 F' B' U D R' U2
            R' U F U2 F D R U'
10 | B' L U R' U' B2 L' D2 B2 D2 B2
11 | halt "The modulus is"
          B' D2 U R' U'
          counting-until UBL BL

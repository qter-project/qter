Puzzles
A: 3x3

0  | input "Enter number X"
           L2 F2 U L' F D' F' U' L' F D U L' U'
           max-input 29
1  | input "Enter number Y"
           R2 L U' R' L2 F' D R' D L B2 D2
           max-input 29
2  | solved-goto UB DBL 4
3  | goto 5
4  | halt "X×Y mod 30 = 0"
5  | print "Factorizing out two"
6  | solved-goto UB 8
7  | goto 17
8  | repeat until DBL solved
            F' B' D2 R' B2 R U R2 B2
            L' B' U B R2 L2 F R L'
9  | repeat until UB solved
            U B R L' B' R2 L2 B2
            L B R2 F' R' D' L' D
10 | repeat until UFL solved
            D B2 D2 L' D' B2 D' B D2
            L2 B' D F2 B2 U' F2 B2
11 | repeat until UL solved
            D' R L' U' F' B2 L B
            U B L U R' D2 B' U'
12 | repeat until FR solved
            D' F' U' R U' B2 R' B'
            U2 R' D R' D F D L' D2
13 | repeat until UBR solved
            F' U' R F L' U2 L2 F D
            R' F' L' U2 F' U' R' F
14 | repeat until UFL solved
            F2 R2 F L D L' F U2 F
            U2 F U2 R' U2 R2 F2 U2
15 | repeat until UL solved
            L' U' B' R' U' F R B2 R
            U2 L2 B L' F2 L D2 L'
16 | goto 6
17 | D2 B2 L' D' R D' F R L2 U R2 L'
18 | solved-goto UB 20
19 | goto 24
20 | R2 L U' R' L2 F' D R' D L B2 D2
21 | repeat until UFL solved
            F' R' F' D' R2 F' D2 F2
            L2 B U' R' D R2 F2 U' F R
22 | repeat until UL solved
            D' L D R F R2 B' L'
            B2 R2 L2 B R' L B' U'
23 | goto 26
24 | F2 B2 U F2 B2 D' B L2 D2 B' D B2 D L D2 B2 D'
25 | goto 6
26 | print "Factorizing out three"
27 | goto 36
28 | repeat until UB solved
            U' R2 L2 U R2 F2 D2 R'
            F2 L' U2 L U L' B D' B
29 | repeat until DBL solved
            F D' F2 U L2 U' L2 F U' F2 D F2 U
30 | repeat until UFL solved
            D B2 D2 L' D' B2 D' B D2
            L2 B' D F2 B2 U' F2 B2
31 | repeat until UL solved
            D' R L' U' F' B2 L B
            U B L U R' D2 B' U'
32 | repeat until FR solved
            D' F' U' R U' B2 R' B'
            U2 R' D R' D F D L' D2
33 | repeat until UBR solved
            F' U' R F L' U2 L2 F D
            R' F' L' U2 F' U' R' F
34 | repeat until UFL solved
            D L2 D' F' U' R B2 R2
            U R F U' F2 L2 F2 L2 U
35 | repeat until UL solved
            B2 F2 L' B L D F2 L
            F2 L2 D' B F2 D' R D
36 | solved-goto DBL 28
37 | print "Factorizing out five"
38 | R2 L' F R2 D2 L' D R' U L' B2 R L D' L2 B D'
39 | solved-goto UB 42
40 | B' U R U F U L' B2 D2 R' L2 D' B' U' L2 U2 F'
41 | goto 52
42 | B' U R U F U L' B2 D2 R' L2 D' B' U' L2 U2 F'
43 | repeat until UB solved
            F2 B2 U F2 D L' B D2 B2
            D L2 D' B R' L2 B2 R L
44 | repeat until DBL solved
            R D2 L D' L' U2 F2 R
            B R' F2 B' U2 D' R'
45 | repeat until UFL solved
            D B2 D2 L' D' B2 D' B D2
            L2 B' D F2 B2 U' F2 B2
46 | repeat until UL solved
            D' R L' U' F' B2 L B
            U B L U R' D2 B' U'
47 | repeat until FR solved
            D' F' U' R U' B2 R' B'
            U2 R' D R' D F D L' D2
48 | repeat until UBR solved
            F' U' R F L' U2 L2 F D
            R' F' L' U2 F' U' R' F
49 | repeat until UFL solved
            L2 R2 F D B2 F' R2 B
            R B D B D' U R' U' L2
50 | repeat until UL solved
            U R L' U' B2 R2 B2 R L' B' L2 U2 R2 U'
51 | goto 38
52 | print "Factorizing out seven"
53 | D2 B2 L' D' R D' F R L2 U R2 L'
54 | solved-goto UB 57
55 | R2 L U' R' L2 F' D R' D L B2 D2
56 | goto 59
57 | R2 L U' R' L2 F' D R' D L B2 D2
58 | goto 68
59 | repeat until DBL solved
            F' D' F' U' R B2 U2 D'
            R D F2 L B2 L D2 L2 D2
60 | repeat until UB solved
            R U2 B R2 L' D F D R' L' U' D2 R D B
61 | repeat until UFL solved
            D B2 D2 L' D' B2 D' B D2
            L2 B' D F2 B2 U' F2 B2
62 | repeat until UL solved
            D' R L' U' F' B2 L B
            U B L U R' D2 B' U'
63 | repeat until FR solved
            D' F' U' R U' B2 R' B'
            U2 R' D R' D F D L' D2
64 | repeat until UBR solved
            F' U' R F L' U2 L2 F D
            R' F' L' U2 F' U' R' F
65 | repeat until UFL solved
            B D F U' L2 D2 B D U
            B F U F' R' F U R F'
66 | repeat until UL solved
            L D2 L' F R2 F D F2
            B U' F U2 B U B2 D R
67 | goto 53
68 | print "Factorizing out eleven"
69 | D2 B2 L' D' R D' F R L2 U R2 L'
70 | solved-goto UB DBL 81
71 | R2 L U' R' L2 F' D R' D L B2 D2
72 | repeat until DBL solved
            R' B' R D F L2 U' B2 L2
            B' U L2 U L' U' B2 L2 F'
73 | repeat until UB solved
            F2 L' U2 D R2 L U'
            F' D' B L D L F R B2
74 | repeat until UFL solved
            D B2 D2 L' D' B2 D' B D2
            L2 B' D F2 B2 U' F2 B2
75 | repeat until UL solved
            D' R L' U' F' B2 L B
            U B L U R' D2 B' U'
76 | repeat until FR solved
            D' F' U' R U' B2 R' B'
            U2 R' D R' D F D L' D2
77 | repeat until UBR solved
            F' U' R F L' U2 L2 F D
            R' F' L' U2 F' U' R' F
78 | repeat until UFL solved
            U L' R2 U2 B' U2 B' L
            R2 U' F R' U F2 U' F R
79 | repeat until UL solved
            F2 L F2 D B' D2 B F D'
            B R2 B' F' D2 L' F2 R2
80 | goto 82
81 | R2 L U' R' L2 F' D R' D L B2 D2
82 | halt "X×Y mod 30 ="
          U L U' D' F' L U F D F' L U' F2 L2
          counting-until UBR FR

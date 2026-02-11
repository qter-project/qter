Puzzles
A: 3x3

0 | input "First number:"
          U
          max-input 3
1 | input "Second number:"
          D
          max-input 3
2 | repeat until DFL solved
           U D'
3 | halt "(A + B) % 4 ="
         U'
         counting-until UFL

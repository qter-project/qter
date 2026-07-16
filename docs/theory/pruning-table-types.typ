#import "../book.typ": book-page, canvas, diagram
#import "@preview/cetz:0.4.2"
#import "../cube/cube.typ": *

#show: book-page.with(title: "Pruning Table Types")

The Cycle Combination Solver uses a separate pruning table per the puzzle orbits. For the Rubik's Cube, that means one pruning table for the corners and one for the edges. To get an admissible heuristic for an individual position, we query each pruning table based on the states of the position's corresponding orbits and take the maximum value. A brief example: if querying a Rubik's Cube state returns $3$ on the corners pruning table and $5$ on the edges pruning table, then its admissible heuristic is the maximum of the two, $5$. We established that larger heuristic values are better, and the optimality guarantee still stands because each individual pruning table is already admissible.

Generating a pruning table for an orbit is done in two phases. First, we enumerate every single position of the orbit and mark solutions of the Cycle Combination Solver. Then, we search the Rubik's Cube tree but from these solution states instead of from the solved state, and storing the amount of moves required to reach each state found as the admissible heuristic.

The Cycle Combination Solver supports four different types of pruning tables: the exact pruning table, the approximate pruning table, the cycle structure pruning table, and the fixed pruning table. They are dynamically chosen at runtime based on a maximum memory limit option.

*We defer our discussion of pruning table types for a later revision.*

// Don't store keys
// antisymmetry means we have to premove
// show that postmoves(inv(P)) = inv(premove(P))
// The exact pruning table . This is formally known as a perfect hash table.
// Exact:
// - P H F
// - IDDFS
// - Scanning
// - Backwards scanning
// Approximate:
// - Each entry in a pruning table represents many puzzle positions.
// - Lossy compression
// Cycle structure:
// Populating the pruning table is a form of search

Finally, the Cycle Combination Solver generates the pruning tables and performs IDA\* search at the same time. It would not be very efficient for the Cycle Combination Solver to spend all of its time generating the pruning tables only for the actual searching part to be easy, so it balances out querying and generation; starting from an uninitialized pruning table, if the number of queries exceeds the number of set values by a factor of $3$, it pauses the search to generate a deeper layer of that pruning table and then continues.

==== Pruning table compression

The Cycle Combination Solver supports three different data compression types: no compression, nxopt compression, and tabled asymmetric numeral systems (tANS) compression. They are dynamically chosen at runtime based on a maximum memory limit option.

*We defer our discussion of pruning table compression for a later revision.*

// - tANS
//     - a potentially better pruning table implementation over the traditional 2-bit and "base" value approach.

// There's a relatively new entropy coding algorithm called "tabled asymmetric numeral systems" (tANS), where encoding and decoding a symbol consists only of a table lookup and some additions and bitwise operations, so it's very fast and close to the limit of the source coding theorem.

// We wrote a simple sagemath script (which I can send if you like) to roughly estimate how many more pruning values this would allow you to fit. It seems to be highly parameter sensitive, but for a 32GB table, 512 bit blocks, and 'n' = 439 (I manually tuned this for the best results), the two bit method (by my math) is able to store \~2.3 trillion correct pruning values and this new method should be able to store \~7.5 trillion correct pruning values.

// The big tradeoff with this method would be CPU time, because the chunk can't be randomly accessed and on average half the symbols in the block would need to be decoded. The blocks could be shrunk to mitigate this, but tANS has a constant space overhead as well as the maximum depth metadata, so it would come at the cost of storage. Distance entropy algorithm

// We note that, in the case where an orbit has a large number of states, we cannot further split up the state space into multiple smaller pruning tables; we are only allowed to use a single pruning table per orbit. This is unlike Korf's optimal solver, which split up the 981 billion edge states of the Rubik's Cube into two smaller pruning tables of a more manageable 42 million states each. Instead of storing all of those 981 billion edge states, the only option for the Cycle Combination Solver is to resort to selecting a less accurate pruning table that takes up less memory. There are two reasons why: first, the cycle structure is an interdependent property of the entire orbit that cannot be subdivided. A state with our target cycle structure can possibly permute all of the edges on the Rubik's Cube, so it would be nontrivial to look at just a subset of the edges and confidently produce an admissible heuristic. Second, we don't even know where each edge will end up in a Cycle Combination Solver solution . We have not closely examined if such a pruning table is possible for the Cycle Combination Solver, but we expect this problem to be at best nontrivial.

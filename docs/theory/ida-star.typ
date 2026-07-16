#import "../book.typ": book-page, canvas, diagram as diagram2
#import "@preview/cetz:0.4.2"
#import "../cube/cube.typ": *
#import "@preview/fletcher:0.5.8" as fletcher: diagram, edge, node, shapes

#show: book-page.with(title: "IDA* Optimizations")

We employ a number of tricks to improve the running time of the Cycle Combination Solver's IDA\* tree search.

==== SIMD

We enhance the speed of puzzle operations through the use of puzzle-specific SIMD on AVX2 and Neon instruction set architectures. Namely, the `VPSHUFB` instruction on AVX2 and the `tbl.8`/`tbl.16` instructions on Neon perform permutation composition in one clock cycle, enabling for specialized SIMD algorithms to compose two Rubik's Cube states and test for a Cycle Combination Solver solution. They have both been disassembled and highly optimized at the instruction level. Additionally, the puzzle-specific SIMD uses compacted representations optimized for the permutation composition instructions. For example, it uses a representation of a Rubik's Cube state that can fit in a single `YMM` CPU register on AVX2 and in the `D` and `Q` CPU registers on Neon.

Pruning table generation also uses puzzle-specific SIMD. To generate a pruning table on the corners orbit, we need to use a different Rubik's Cube representation because we don't want to waste CPU caring about what happens to edges. So, every orbit has its own specialized SIMD representation and SIMD algorithm modifications.

*We leave the precise details at the prescribed references; we defer our discussion of how the SIMD algorithms work for a later revision.*

==== Canonical sequences

At every increasing depth level of the IDA\* search tree we explore $18$ times as many nodes. We formally call this number the _branching factor_—the average number of child nodes visited by a parent node. A few clever observations can reduce the branching factor.

We observe that we never want to rotate the same face twice. For example, if we perform $R$ followed by $R'$, we've just reversed the move done at the previous level of the tree. Similarly if we perform $R$ followed by another $R$, we could have simply done $\R2$ straight away. In general, any move should not be followed by another move in the same _move class_, the set of all move powers. This reduces the branching factor of the child nodes from $18$ for all $18$ moves to $15$. Additionally, we don't want to search both $R L$ and $L R$ because they commute, and result in the same net action. So, we assume that $R$ (or $\R2, R'$) never follows $L$ (or $\L2, L'$), and in general, we only permit searching distinct commutative move classes strictly in a single order only. Move sequences that satisfy these two conditions are called _canonical sequences_. Canonical sequences are special because these two conditions make it easy to check if a move sequence in the search tree is redundant.

What does the second condition reduce our branching factor from $15$ to? We start by counting the number of canonical sequences at length $N$, denoted $a_n$, using a recurrence relation. We consider the last move of the sequence $M_1$, the second to last move $M_2$, and the third to last move $M_3$. The recurrence relation can be constructed by analyzing two cases:

- Case 1: $M_1$ and $M_2$ do not commute.

    In this case, $a_n$ is simply $a_(n-1)$ multiplied by the number of possibilities of $M_1$. Since $M_1$ and $M_2$ do not commute, $M_1$ cannot be $M_2$ ($-1$) nor its opposite face ($-1$). Therefore, $M_1$ must be one of $6 - 1 - 1 = 4$ move classes, or one of the $4 * 3 = 12$ possible moves. We can establish that the first component in the recurrence relation for $a_n$ is $12a_(n-1)$.

- Case 2: $M_1$ and $M_2$ commute.

    We need to be careful to only count $M_1$ and $M_2$, one time so we count them in pairs. In this case, $a_n$ is simply $a_(n-2)$ multiplied by the number of strictly ordered $(M_1, M_2)$ pairs. There are $3$ pairs of commutative move classes: $\FB, \UD, "and" \RL$. We have to discard one of these pairs because $M_3$ necessarily commutes with the move classes in one of these pairs since the union of all of these pairs is every move. Such a canonical sequence where the subsequence $M_3 M_2 M_1$ all commute cannot exist because one of those moves will always violate the strict move class ordering. For example, if $M_1$ is $L$ and $M_2$ is $R$, then there is no possible option for $M_3$ that makes the full sequence a canonical sequence.

    Each move class in each pair can perform three moves, which implies that each pair contributes $3 * 3 = 9$ possible moves. Overall we find this number to be $(3 - 1) * 9 = 18$ possible moves. We can establish that the second component in the recurrence relation for $a_n$ is $18a_(n-2)$.

$a_n$ can be thought of as the superposition of these two cases with the base cases $a_1 = 18 "and" a_2 = 243$ (exercise to the reader: figure out where these come from). Hence, $a_n = 12a_(n-1) + 18a_(n-2) "for" n > 2$. The standard recurrence relation can be solved as follows:

$
    & r^n = 12r^(n-1) + 18r^(n-2) \
    & r^(n-2)(-r^2 + 12r + 18) = 0 \
    & r = (-12 plus.minus sqrt(12^2 - 4(-1)(18))) / (2(-1)) \
    & r_(1,2) = 6 plus.minus 3sqrt(6) \
    & a_n = A r_1^(n-2) + B r_2^(n-2) = A/r_1^2 r_1^n + B/r_2^2 r_2^n \
    & cases(
             a_1 = 18,
              a_2 = A & + B                     &  = 243,
          a_3 = A r_1 & + B r_2 = 12a_2 + 18a_1 & = 3240,
      ) \
    & "Solve for" A "and" B \
    & "..." \
    & a_n tilde.eq 1.362(13.348)^n + 0.138(-1.348)^n \
$

The $1.362(13.348)^n$ term dominates $0.138(-1.348)^n$ as $n$ approaches infinity; our new branching factor is approximately $13.348$!

It turns out that $a_n$ is not an exact bound on the number of distinct positions at sequence length $N$ but merely an upper bound. This is because the formula overcounts, and the actual number is always lower: it considers canonical sequences that produce equivalent states such as $\R2$ $\L2$ $\U2$ $\D2$ and $\U2$ $\D2$ $\R2$ $\L2$ as two distinct positions. It turns out it is extremely nontrivial to describe and account for these equivalences, to the point where it's not worth doing so: at shallow and medium depths, $a_n$ roughly stays within $10%$ of the actual distinct position count. The Cycle Combination Solver considers the extra work negligible and searches equivalent canonical sequences anyways. The Big O time complexity of IDA\* can be realized as $O(13.348^d/m)$, an improvement over $O(18^d/m)$ from.

The Cycle Combination Solver uses an optimized finite state machine to perform the canonical sequence optimization.

==== Sequence symmetry <time-complexity-2>

We use a special form of symmetry reduction during the search we call _sequence symmetry_, first observed by Rokicki and improved by our implementation. Some solution to the Cycle Combination Solver $A B C D$ conjugated by $A^(-1)$ yields $A^(-1) (A B C D) A = B C D A$, which we observe to be a rotation of the original sequence as well as a solution to the Cycle Combination Solver by the properties of conjugation discussed earlier. Repeatedly applying this conjugation:

$
       & A^(-1) (A B C D) A = B C D A \
    => & B^(-1) (B C D A) B = C D A B \
    => & C^(-1) (C D A B) C = D A B C \
    => & D^(-1) (D A B C) D = A B C D \
$

forms an equivalence class based on all the rotations of sequences that are all solutions to the Cycle Combination Solver. The key is to search a single representative sequence in this equivalence class to avoid duplicate work.

Similarly to symmetry conjugation, we choose the representative as the lexicographically minimal sequence on a move-by-move basis (with a move class ordering relation defined). Unlike symmetry conjugation, we don't manually apply all sequence rotations to find the representative; rather, we embed sequence symmetry as a modification to the recursive IDA\* algorithm such that it only ever searches the representative sequence. We do this by observing that if a _representative sequence_ starts with move $A$, then every other move cannot be lexicographically lesser than it. If this observation were to be false, we could keep on rotating the sequence until the offending move is at the beginning of the sequence, and since that move is lexicographically lesser than $A$ that sequence rotation would be the true representative. This contradicts the initial _representative sequence_ assumption. We permit moves that are lexicographically equal to $A$ (i.e. in the same move class) but change the next recursive step to repeat the logic on the move _after_ $A$. The overall effect is that the IDA\* algorithm only visits move sequences such that no later subsequence is lexicographically lesser than the beginning of the move sequence. This suffices for the complete sequence symmetry optimization.

The modification described is not yet foolproof. The sequence $A B A B C A B$ would technically be valid as there is no later subsequence lesser than the beginning, but the actual lexicographically minimal representative is the $A B A B A B C$ sequence rotation. The "later subsequence" of the true representative wraps around from the end to the beginning. So, extra care must be taken at the last depth to manually account for the wrapping behavior. We only apply this to the last depth, so sequences like $A B A B C A B C$ are still searched by the next depth limit of IDA\*.

We can extend our prior definition of canonical sequences to include sequence symmetry as a third condition. How does sequence symmetry affect the number of canonical sequences at depth $N$? Because a sequence of length $N$ has $N$ sequence rotations, sequence symmetry logically divides the total number of nodes visited by $N$, but only in the best case. The canonical sequence $R$ $U$ $R$ $U$ $R$ $U$ only has $2$ members in its sequence rotational equivalence class, not $6$, so the average value to divide by is actually a bit less than $N$. It follows that the average number of canonical sequences at depth $N$ (and the IDA\* asymptotic time complexity) is bound by $Omega(13.348^d/(\md))$ and $O(13.348^d/m)$. Testing has shown this number to typically be right in the middle of these two bounds.

Furthermore, we take advantage of the fact that the optimal solution sequence _almost never_ starts and ends with commutative moves. We claim that the IDA\* algorithm _almost never_ needs to test $A B$ $...$ $C$ such that $A$ and $C$ commute for a solution. The proof is as follows.

We first observe that if $A B$ $...$ $C$ is a solution, then $C A B$ $...$ is also a solution by a sequence rotation. This tells us that $A$ and $C$ cannot be in the same move class or else they could be combined to produce the shorter solution $D B$ $...$ . Such a shorter solution would have been found at the previous depth limit, implying that $A B$ $...$ $C$ never would have been explored, making this situation an impossibility. This also tells us that $A$ also cannot be in a greater move class than $C$ because $C A B$ $...$ would be a lexicographically lesser than $A B$ $...$ $C$, contradicting our earlier proof that IDA\* only searches the lexicographically minimal sequence rotation (the representative). Therefore, $A$ must be in a lesser move class than $C$.

If $C A B$ $...$ is a solution, then $A C B$ $...$ is also a solution because $A$ and $C$ commute. By the transitive property, if $A B$ $...$ $C$ is a solution, then so is $A C B$ $...space.nobreak$ . Both of these sequences are independently searched and tested as a solution because there is no direct "commutative move ordering" or sequence symmetry relation between them. This is redundant work; we choose to discard the $A B$ $...$ $C$ case. This completes our proof.

This optimization only applies to the last depth in IDA\*, so it only prevents running the test to check if a node is a solution and does not affect the time complexity. It turns out to be surprisingly effective at reducing the average time per node because most of the time is spent at the last depth.

We alluded to an edge case when we said "_almost never_." If $B$ doesn't exist, or if every move from $B$ $...$ commutes with $A$ and $C$, then this optimization will skip canonical sequences where every move commutes with each other; for example $F$ $B$ on the Rubik's Cube. The number of skipped sequences is so small that we have the bandwidth to manually search and test these sequences for solutions before running IDA\*.

==== Pathmax

We use a simple optimization described by Mérõ called _pathmax_ to prune nodes with large child pruning heuristics. When a child node has a large pruning heuristic, we can set the current node cost to that value minus one and re-prune to avoid expanding the remaining child nodes. This larger heuristic is still admissible because it is one less than a known lower bound, and the current node is one move away from all of its child nodes. This is only effective when the heuristics are _inconsistent_, or, in this case, when the pruning table entries are the minimum of two or more other values. With exact pruning tables only, this optimization will never run because the entries are perfect heuristics that cannot exhibit this type of discrepency.

#diagram2(figure(
    diagram(
        node((0, 0), [2], stroke: 0.5pt, name: <first>),
        node((rel: (11.75mm, 0mm)), [$+$ #h(0.5mm) $5 gt.not 8$]),
        node((0.75, 0.75), "5", stroke: 0.5pt, name: <second>),
        node((-0.75, 0.75), "1", stroke: 0.5pt, name: <third>),
        node(enclose: ((-0.75, 0), (0.75, 0.75)), name: <wrapper1>),
        edge(<first>, <second>),
        edge(<first>, <third>),
        edge(<wrapper1>, <wrapper2>, align(bottom)[Pathmax], "-|>"),

        node((4.5, 0), "4", stroke: 0.5pt, name: <fourth>),
        node((rel: (0mm, 0mm)), move(dx: 51pt)[#box[$+$ #h(0.5mm) $5 gt 8$ (Prune)]]),
        node((5.25, 0.75), "5", stroke: 0.5pt, name: <fifth>),
        node((3.75, 0.75), "1", stroke: 0.5pt, name: <sixth>),
        node(enclose: ((4.5, 0), (3.75, 0.75)), name: <wrapper2>),
        edge(<fourth>, <fifth>),
        edge(<fifth>, <fourth>, text(size: 10pt)[$-1$], "-|>", bend: 30deg),
        edge(<fourth>, <sixth>),
    ),
    caption: text(size: 12pt)[IDA\* pathmax at $"depth"=5, "depth limit"=8$],
    supplement: none,
))

==== Parallel IDA\*

Our last trick is to enhance IDA\* through the use of parallel multithreaded IDA\* (PMIDA\*). PMIDA\* runs in two phases. In the first phase, we use BFS to explore the state space to a shallow depth, maintaining a queue of all of states at the last search depth. In the second phase, we use a thread pool to run IDA\* in parallel for every state in that queue, utilizing of all of the CPU cores on the host machine. To uphold the optimality guarantee, PMIDA\* synchronizes the threads using a barrier that triggers when they have all completed exploring the current level. It can be thought of as a simple extension to the familiar IDA\* algorithm.

There have been many parallel IDA\* algorithms discussed in literature; how do we know PMIDA\* is the best one? We take advantage of the special fact that the Cycle Combination Solver starts searching from the solved state. In order to understand this, we compare the total Rubik's Cube position counts with the Rubik's Cube position counts that are unique by symmetry.

#diagram2(grid(
    columns: (20em, 20em),
    align: center + bottom,
    grid.cell(breakable: false)[
        Rubik's Cube position counts
        #table(
            columns: (auto, auto, auto),
            table.header([*Depth*], [*Count*], [*Branching\ factor*]),
            [0], [1], [NA],
            [1], [18], [18],
            [2], [243], [13.5],
            [3], [3240], [13.333],
            [4], [43239], [13.345],
            [5], [574908], [13.296],
            [6], [7618438], [13.252],
            [7], [100803036], [13.231],
            [8], [1332343288], [13.217],
            [9], [17596479795], [13.207],
        )
    ],
    grid.cell(breakable: false)[
        Rubik's Cube position counts unique by \ symmetry $+$ antisymmetry
        #table(
            columns: (auto, auto, auto),
            table.header([*Depth*], [*Count*], [*Branching\ factor*]),
            [0], [1], [NA],
            [1], [2], [2],
            [2], [8], [4],
            [3], [48], [6],
            [4], [509], [10.604],
            [5], [6198], [12.177],
            [6], [80178], [12.936],
            [7], [1053077], [13.134],
            [8], [13890036], [13.190],
            [9], [183339529], [13.199],
        )
    ],
))

Recall that our theoretical branching factor is $13.348$. In the table of Rubik's Cube position counts, the branching factor roughly matches this number. However, at the shallow depths of the table of Rubik's Cube position counts unique by symmetry $+$ antisymmetry, our branching factor is much less because there are duplicate positions when performing moves from the solved state. Intuitively, this should make sense: the Rubik's Cube is not scrambled enough to start producing unique positions. It is easy to pick out two sequences of length two that are not unique by symmetry; for example $\R2$ $U$ and $\R2$ $F$. The branching factor converges to its theoretical value as the Rubik's Cube becomes more scrambled because symmetric positions become more rare. In fact, it was shown by Qu that scrambling the Rubik's Cube can literally be modelled as a Markov chain (it's almost indistinguishable from a random walk of a graph). Hence, it is unlikely for two random move sequences of the same length to produce positions equivalent by symmetry. We know that such collisions _do_ happen because the branching factor doesn't actually reach the $13.348$ value, but we consider them negligible.

The effectiveness of the PMIDA\* algorithm stems from combining all of these observations. When our initial shallow BFS search is done, we filter out the many symmetrically equivalent positions from the queue to avoid redundant work before we start parallelizing IDA\*. The savings are incredibly dramatic: at depth $4$, for example, we symmetry reduce the number of nodes from $43239$ to $509$. This is a reduction by $~84.9$, a factor that is close to the familiar $96$ (the number of symmetries $+$ antisymmetries). Once we do that, and the cube starts to become sufficiently scrambled, we are confident to claim that each IDA\* thread worker explores their own independent regions of the search space and duplicates a negligible amount of work.

We make note that there are almost always going to be more positions in the queue to parallelize than available OS threads. We use an optimized thread pool work stealing algorithm for our multithreaded implementation.

We squeeze out our last bit of juice by overlapping pruning table memory latency with the computation. It has been empirically observed that random access into the pruning table memory is the dominating factor for Rubik's Cube solvers. Modern processors include prefetching instructions that tell the memory system to speculatively load a particular memory location into cache without stalling the execution pipeline to do so. Our PMIDA\* implementation uses a technique described by Rokicki called _microthreading_ to spend CPU time on different subsearches while waiting for the memory to come to a query. It splits up each thread into eight "slivers" of control. Each sliver calculates a pruning table query memory address, does a prefetch, and moves on to the next sliver. When that sliver gets control again, only then does it reference the actual memory. By handling many subsearches simultaneously, microthreading minimizes the CPU idle time.

How does PMIDA\* affect the asymptotic time complexity? We established in an upper bound of $O(13.348^d/m)$. The time required by PMIDA\* can be computed by adding the time of the first and second phases. In the first phase the time required for the BFS is $O(13.348^(d_1))$ where $d_1$ is the aforementioned shallow depth. In the second phase we symmetry reduce at the shallow depth, split the work across $t$ independent threads, and ignore nodes before depth $d_1$. The time required is $O((13.348^d/(\ms) - 13.348^(d_1)) slash t)$ where $s$ is the number of symmetries $+$ antisymmetries. The PMIDA\* time complexity is thus $O(13.348^(d_1) + (13.348^d/(\ms) - 13.348^(d_1)) slash t)$, but we consider $d_1$ to be very small and $s$ to be a negligible constant. As such the final time complexity becomes $O(13.348^d/(\mt))$. We can apply the exact same logic to our lower bound, and we get $Omega(13.348^d/(d\mt))$.

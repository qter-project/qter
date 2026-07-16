#import "../book.typ": book-page, canvas, diagram
#import "@preview/cetz:0.4.2"
#import "../cube/cube.typ": *

#show: book-page.with(title: "Pruning")

IDDFS solves the memory issue, but is lacking in speed because tree searching is still slow. The overwhelming majority of paths explored lead to no solution. What would be nice is if we could somehow know whether all paths that continue from a given node are dead ends without having to check by brute-force.

For this, we introduce the idea of a _pruning table_. For any given Rubik's Cube position, a pruning table tells you a lower bound on the number of moves needed to reach a Cycle Combination Solver solution. Suppose we are running IDDFS until depth $12$, we've done 5 moves so far, and we have reached this node.

#diagram(figure(
    cetz.canvas(length: 15pt, {
        import cetz.draw: *

        cube("ywrgwygwg ybwygwrrw orborbory", offset: (-2.5, 0))
        cube("wgbybbgoo rbbwyybgg yrrgoowoo", offset: (2.5, 0), back: true)
    }),
    caption: figure.caption(position: top, text(1.2em)[R' U2 L' D' R']),
    supplement: none,
))

If we _query_ the pruning table and it says that this position needs at least $8$ moves to reach a Cycle Combination Solver solution, we know that this branch is a dead end. $5$ moves done so far plus $8$ left is $13$, which is more than the $12$ at which we plan to terminate. Hence, we can avoid having to search this position any longer.

The use of pruning tables in this fashion was originated by Korf in his optimal Rubik's Cube solver. He observed the important requirement that pruning tables must provide _admissible heuristics_ to guarantee optimality. That is, they must never overestimate the distance to a solution. If in the above example, the lower bound was wrong and there really was a solution in $12$ moves, then the heuristic would prevent us from finding it. Combining IDDFS and an admissible heuristic is known as Iterative Deepening A\* (IDA\*).

How are we supposed to store all 43 quintillion positions of the Rubik's Cube in memory? Well, we don't: different types of pruning tables solve this problem by sacrificing either information or accuracy to take up less space. Hence, pruning tables give an admissible heuristic instead of the exact number of moves needed to reach a Cycle Combination Solver solution.

Loosely speaking, pruning tables can be thought of as a form of meet-in-the-middle search, more generally known as a space—time trade-off. Even when running the Cycle Combination Solver on the same puzzle, we _must_ generate a new pruning table for every unique cycle structure. It turns out this is still worth it. In general, we can characterize the effectiveness of a pruning table by its expected admissible heuristic, $p$. Pruning tables reduce the search depth of the tree because they have the effect of preventing searching the last $p$ depths, and the improvements are dramatic because the number of nodes at increasing depths grows exponentially. But there is no free lunch: we have to pay for this speedup by memory.

We are left with a need to examine the asymptotic time complexity of IDA\*. In general pruning table distributions are nontrivial to analyze, so our observations below are not a formal analysis but rather a series of intuitive arguments. An IDA\* search to depth limit $d$ is similar to an IDDFS search to depth limit $d - p$, implying a time complexity of IDA\* is $O(18^(d - p))$ (recall how the last depth is the dominating factor). One might even be eagar to consider these two searches exactly equivalent, but Korf describes a perhaps surprising discrepancy: the number of nodes visited by IDA\* is empirically far greater, up to a magnitude of two. Nodes with large pruning values are quickly pruned, while nodes with small pruning values survive to spawn more nodes. Thus, IDA\* search is biased in favor of smaller heuristic values, and the expected admissible heuristic is actually lesser.

Next we conjecture that $p$ is logarithmically correlated to the number of states the pruning table can store, which we denote as the amount of memory used $m$. If we first assume the branching factor $b$ to be constant, implying each depth has exactly $b$ times more states stored in the pruning table than the previous depth, we notice the maximum depth that is stored in the pruning table is at least $log_b m$. Since there are exponentially more states at the last depth, $p$ is negligibly less than $log_b m$; hence, $p tilde.eq log_b m$. In reality, there are two flaws with this assumption. First, the branching factor is not constant and always less than its theoretical value, eventually converging to zero. This implies our estimate of $p tilde.eq log_b m$ is an egregious overestimate of the actual average pruning value, but we consider this okay because IDA\* is biased in favor of smaller heuristic values. Second, when there are relatively many Cycle Combination Solver solutions, the maximum depth state stored in the pruning table decreases. We also consider this okay because many solutions implies that one will be found at a lesser search depth. If we let $lambda$ equal to both of these reductions, we find that the IDA\* search depth limit remains approximately the same: $(d - lambda) - (p - lambda) = d - p$. All of the aforementioned biases cancel each other out to some extent, so we proceed with this approximation of $p$.

As such, $O(18^(d - p)) = O(18^(d - log_18 m)) = O(18^d/m)$. Empirically and analytically, doubling the size of the pruning table halves the CPU time required to perform a search.

=== Pruning table design

The larger the admissible heuristic, the better the pruning, and the lesser the search depth. So, we need to carefully design our pruning tables to maximize:
- how much information we can store within a given memory constraint; and
- the value of the admissible heuristic

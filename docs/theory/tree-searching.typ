#import "../book.typ": book-page, canvas
#import "@preview/cetz:0.4.2"
#import "../cube/cube.typ": *

#show: book-page.with(title: "Tree Searching")

A more formal way to think about the Cycle Combination Solver is to think of the state space as a tree of Rubik's Cube positions joined by the 18 moves. The number of moves that have been applied to any given position is simply that position's corresponding level in the tree. We will refer to these positions as _nodes_.

#let cubenode(faces) = {
    cetz.canvas({
        cube(faces, scale-amt: 0.5)
    })
}

#canvas({
    import cetz.draw: *

    stroke(2pt + black)
    let data = (
        cubenode("rrrrrrrrr wwwwwwwww ggggggggg"),
        ([#text(size: 33pt)[\...]],),
        ([#cubenode("rrrrrrrrr gggwwwwww yyygggggg")],),
        ([#cubenode("rrrrrrrrr yyywwwwww bbbgggggg")],),
        (
            cubenode("rrrrrrrrr bbbwwwwww wwwgggggg"),
            [
                #text(size: 33pt)[\...]
                #h(10pt)
                $cubenode("rrbrrwrrw bbowwowwo ggwggwggw")$
                #h(10pt)
                $cubenode("rrorrorro bbywwywwg ggggggwww")$
                #h(10pt)
                $cubenode("rrrrrrwgg bwwbwwbww owwoggogg")$
                #h(10pt)
                #text(size: 33pt)[\...]
            ],
        ),
        ([#cubenode("rrwrrwrrw wwowwowwo ggggggggg")],),
        ([#text(size: 33pt)[\...]],),
    )
    cetz.tree.tree(
        data,
        spread: 1.25,
        grow: 1.5,
        direction: "down",
        draw-node: (node, ..) => {
            content((), [#node.content])
        },
        draw-edge: (from, to, ..) => {
            let (a, b) = (from + ".center", to + ".center")
            line((a, 1.3, b), (b, 1.5, a))
        },
        name: "tree",
    )

    line((to: "tree.g0-3", rel: (-0.9, -1)), (to: "tree.0-3-0", rel: (-4.5, 1.2)))
    line((to: "tree.g0-3", rel: (-0.6, -1.15)), (to: "tree.0-3-0", rel: (-2, 1.2)))
    line((to: "tree.g0-3", rel: (0.6, -1.15)), (to: "tree.0-3-0", rel: (2, 1.2)))
    line((to: "tree.g0-3", rel: (0.9, -1)), (to: "tree.0-3-0", rel: (4.5, 1.2)))
})

Our goal is now to find a node with the specified cycle structure at the _topmost_ level of the tree, a solution of the optimal move length. Those familiar with data structures and algorithms will think of the most obvious approach to this form of tree searching: breadth-first search (BFS). BFS is an algorithm that explores all nodes in a level before moving on to the next one. Indeed, BFS guarantees optimality, and works in theory, but not in practice: extra memory is needed to keep track of child nodes that are yet to be explored. At every level, the number of nodes scales by a factor $18$, and so does the extra memory needed. At a depth level i.e. sequence length of just $8$, BFS would require storing $18^9$ depth-9 nodes or roughly 200 billion Rubik's Cube states in memory. This is clearly not practical; we need to do better.

We now consider a sibling algorithm to BFS: depth-first search (DFS). DFS is an algorithm that explores all nodes as deep as possible before backtracking. It strikes our interest because the memory overhead is minimal; all you need to keep track of is the path taken to reach a node, something that can be easily managed during the search. However, because we explore nodes depth-first, it offers no guarantee about optimality, so we still have a problem.

A simple modification to DFS can make it always find the optimal solution. We tweak the DFS implementation so that it explores up until a specified depth, testing whether each node at this depth is a solution, without exploring further. We repeatedly run this implementation at increasing depth limits until a solution _is_ found. Put simply, you do a DFS of depth 1, then of depth 2, and so on. This idea is known as iterative-deepening depth-first search (IDDFS), a hybrid of a breadth-first and depth-first search. IDDFS does repeat some work each iteration, but the cost is always small relative to the last depth because the Rubik's Cube search tree grows exponentially. The insignificance of the repeat work is further exacerbated given that even more time is spent at the last depth running the test for a solution. Because the majority of the time is spent at the last depth $d$, the asymptotic time complexity of $O(18^d)$ in Big O notation is actually identical to BFS while solving the memory problem. We will gradually improve this time complexity bound throughout the rest of this section.

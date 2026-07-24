# References and implementation mapping

## Implemented practical algorithms

1. Valeriu Soltan and Alexei Gorpinevich, "Minimum Dissection of a
   Rectilinear Polygon with Arbitrary Holes into Rectangles," *Discrete &
   Computational Geometry* 9, 57--79, 1993.
   DOI: 10.1007/BF02189307.

   `rect-oracle-sg` uses Definition 7's four effective-chord conditions, the
   admissible-family reduction, the rectangle-count formula, and Section 10's
   horizontal-then-vertical completion. The current input adapter implements
   ordinary nondegenerate grid-cell polygons, not ornaments or degenerate
   formal holes.

2. David Eppstein, "Graph-Theoretic Solutions to Computational Geometry
   Problems," arXiv:0908.3916, 2009.

   This is background for the graph-theoretic rectangle-partition reduction;
   it is not used as a substitute for Soltan--Gorpinevich Definition 7.

3. John E. Hopcroft and Richard M. Karp, "An n^(5/2) Algorithm for Maximum
   Matchings in Bipartite Graphs," *SIAM Journal on Computing* 2(4), 1973.
   DOI: 10.1137/0202019.

   `rect-graph::hopcroft_karp` is the independent explicit matching oracle and
   supplies alternating reachability for a Konig minimum vertex cover.

4. Donald E. Knuth, "Dancing Links," arXiv:cs/0011047, 2000.

   `rect-oracle-exact-cover` uses Algorithm X's constrained branching idea with
   dynamic bitsets. It deliberately does not use pointer-based dancing links.

5. Jean Cardinal and Yelena Yuditsky, "Compact Representation of Semilinear
   and Terrain-Like Graphs," ESA 2025, LIPIcs 351, Article 67.
   DOI: 10.4230/LIPIcs.ESA.2025.67.

   A fixed-dimensional comparability bigraph has two point sets, with an edge
   when the point on the first side is strictly smaller than the point on the
   second side in every coordinate. `rect-dominance::biclique` directly
   implements the induction in Theorem 8 for this strict coordinatewise
   dominance relation. Lemma 12 supplies the constructive representation.

   The project embedding has four coordinates and uses even/odd parity so that
   no horizontal/vertical cross-side pair has an equal coordinate. The general
   `O(n log^d n)` representation bound therefore specializes to
   `O(q log^4 q)` for `d = 4`. The flow reduction would remain correct for a
   biclique cover, but the implementation claims a partition and checks the
   stronger condition that every explicit edge occurs exactly once.

6. Yefim A. Dinitz, "An Algorithm for the Solution of the Problem of Maximal
   Flow in a Network with Power Estimation," *Doklady Akademii Nauk SSSR* 194,
   1970.

   `rect-graph::dinic` is the implemented practical exact maximum-flow backend.
   It uses safe Rust, integral capacities, and returns residual cut reachability.

## Theoretical algorithm cited only for asymptotic complexity

7. Jan van den Brand et al., "A Deterministic Almost-Linear Time Algorithm for
   Minimum-Cost Flow," arXiv:2309.16629 / FOCS 2023.

   This implementation does **not** implement that algorithm. It appears only
   in the supplied paper's asymptotic analysis. `MaxFlowBackend` keeps geometry
   independent of the current `DinicBackend` so another exact backend can be
   added later without changing reductions or certificates.

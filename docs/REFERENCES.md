# References and implementation mapping

## Implemented practical algorithms

1. Valeriu Soltan and Alexei Gorpinevich, "Minimum Dissection of a
   Rectilinear Polygon with Arbitrary Holes into Rectangles," *Discrete &
   Computational Geometry* 9, 57--79, 1993.
   DOI: 10.1007/BF02189307.

   `mrd-domain::formal_polygon` implements the ornament, formal-boundary,
   vertex, and elementary-segment contracts in Definitions 1, 3, and 4.
   `sg-oracle` uses Definition 7's four effective-chord conditions, the
   admissible-family reduction, the rectangle-count formula, and Section 10's
   horizontal-then-vertical completion. The current input adapter implements
   ordinary nondegenerate grid-cell polygons. Formal-hole chord enumeration
   and completion remain a later phase.

2. David Eppstein, "Graph-Theoretic Solutions to Computational Geometry
   Problems," arXiv:0908.3916, 2009.

   This is background for the graph-theoretic rectangle-partition reduction;
   it is not used as a substitute for Soltan--Gorpinevich Definition 7.

3. John E. Hopcroft and Richard M. Karp, "An n^(5/2) Algorithm for Maximum
   Matchings in Bipartite Graphs," *SIAM Journal on Computing* 2(4), 1973.
   DOI: 10.1137/0202019.

   `graph::hopcroft_karp` is the independent explicit matching oracle and
   supplies alternating reachability for a Konig minimum vertex cover.

4. Donald E. Knuth, "Dancing Links," arXiv:cs/0011047, 2000.

   `exact-cover-oracle` uses Algorithm X's constrained branching idea with
   dynamic bitsets. It deliberately does not use pointer-based dancing links.

5. Jean Cardinal and Yelena Yuditsky, "Compact Representation of Semilinear
   and Terrain-Like Graphs," ESA 2025, LIPIcs 351, Article 67.
   DOI: 10.4230/LIPIcs.ESA.2025.67.

   A fixed-dimensional comparability bigraph has two point sets, with an edge
   when the point on the first side is strictly smaller than the point on the
   second side in every coordinate. `dominance::biclique` directly
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

   `graph::dinic` provides a practical exact maximum-flow backend.
   It uses safe Rust, integral capacities, and returns residual cut reachability.

7. Andrew V. Goldberg and Robert E. Tarjan, "A New Approach to the
   Maximum-Flow Problem," *Journal of the ACM* 35(4), 1988.

   `graph::dinic::PushRelabelBackend` implements the integral
   highest-label preflow-push family with global relabeling and gap heuristic
   counters. It is a practical exact backend, not an almost-linear claim.

## Theoretical algorithm cited only for asymptotic complexity

8. Jan van den Brand et al., "A Deterministic Almost-Linear Time Algorithm for
   Minimum-Cost Flow," arXiv:2309.16629 / FOCS 2023.

   `graph::min_cost` implements only a deliberately superlinear exact
   integer circulation and signed residual minimum-ratio-cycle Oracle. It
   explicitly does not claim the paper's interior-point reduction, hidden
   stability, dynamic cycle structure, or almost-linear bound. The permanent
   practical maximum-flow backends (`DinicBackend` and `PushRelabelBackend`)
   remain independent of this baseline.

9. Ittai Abraham and Ofer Neiman, "Using Petal-Decompositions to Build a Low
   Stretch Spanning Tree," *SIAM Journal on Computing* 48(2), 2019,
   pp. 227--248. DOI: 10.1137/17M1115575.

   `graph::source_an19` implements and audits hierarchy mechanics,
   tree/stretch certificates, source/work counters, and the complete workspace
   scan ledger. The formal SIAM text was checked, but it does not establish the
   reduced-event ordering/counting conversion required by P9.3.2d. The AN19
   runtime chain therefore remains unverified; finite tests and observed event
   counts are not attributed to the paper as a proof.

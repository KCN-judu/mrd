# P18 Post-Clone Representation Audit

This audit is intentionally separate from the canonical clone experiment. It
does not change production representation construction and does not contribute
an unmeasured optimization to the P18 evidence campaign.

## Measured boundary

The original P17 report overstated the post-optimization dominance of cloning
and representation because its analyzer selected only fine-grained leaf fields
and silently omitted coarse geometry and completion parents. The accepted P17
raw measurements show a family-dependent picture: completion dominates
comb-staircase and dense-conflict, geometry dominates
representation-crossover, sparse-conflict, and supported-holes, and completion
dominates random-connected. P18 therefore treats representation construction as
a separate candidate boundary, not as a proven global bottleneck. It does not
change production representation construction, and stopped levels remain
censored rather than timings.

## Operation inventory

| Family | Algorithm | Dominant representation operation | Candidate optimization | Correctness risk | Expected structural variable |
| --- | --- | --- | --- | --- | --- |
| comb-staircase | compact MRD | direct parity embedding followed by Theorem-8 biclique partition | reusable compact representation workspace | partition ordering, block coverage, and dominance certificate | `sigma`, `M` |
| comb-staircase | explicit Hopcroft--Karp | explicit H/V conflict adjacency materialization | no new implementation in P18; measure graph capacity reuse separately | edge identity and matching parity | `K` |
| comb-staircase | explicit C0 flow | explicit graph to singleton-block partition conversion | no new implementation in P18; measure incidence reuse separately | singleton block completeness | `K`, `sigma` |
| representation-crossover | compact MRD | compact biclique block and compressed-network incidence materialization | reusable compact representation workspace | exact Theorem-8 partition and deterministic block order | `sigma`, `M` |
| representation-crossover | explicit Hopcroft--Karp | dense conflict adjacency construction | no new implementation in P18 | full edge-set equality | `K` |
| representation-crossover | explicit C0 flow | explicit graph plus singleton biclique conversion | no new implementation in P18 | C0 edge-to-block identity | `K`, `sigma` |
| dense-conflict | compact MRD | high-incidence biclique construction and network materialization | reusable compact representation workspace | compressed flow cut/cover equality | `sigma`, `M` |
| dense-conflict | explicit Hopcroft--Karp | dense adjacency vectors | no new implementation in P18 | Hopcroft--Karp graph dimensions | `K` |
| dense-conflict | explicit C0 flow | singleton-block expansion of explicit edges | no new implementation in P18 | partition audit and flow certificate | `K`, `sigma` |
| random-connected | compact MRD | repeated compact block construction for irregular chord families | reusable compact representation workspace | block partition exactness | `sigma`, `M` |
| random-connected | explicit Hopcroft--Karp | irregular conflict adjacency construction | no new implementation in P18 | conflict identity and cover checksum | `K` |
| random-connected | explicit C0 flow | explicit graph and C0 conversion | no new implementation in P18 | flow network dimensions | `K`, `sigma` |
| sparse-conflict | compact MRD | representation setup despite a small conflict set | reusable compact representation workspace; separately measure a zero-conflict fast path later | zero-conflict optimum and output validity | `q`, `sigma`, `M` |
| sparse-conflict | explicit Hopcroft--Karp | sparse adjacency allocation | no new implementation in P18 | absent-edge handling | `K` |
| sparse-conflict | explicit C0 flow | singleton-block conversion with low incidence | no new implementation in P18 | empty/singleton partition cases | `K`, `sigma` |
| supported-holes | compact MRD | hole-induced biclique incidence and compressed network | reusable compact representation workspace | hole-aware completion and cover certificate | `sigma`, `M` |
| supported-holes | explicit Hopcroft--Karp | conflict adjacency across supported holes | no new implementation in P18 | boundary/chord identities | `K` |
| supported-holes | explicit C0 flow | singleton-block representation | no new implementation in P18 | partition/network conservation | `K`, `sigma` |

The table names the structural variable that should be paired with the timing;
it is not an asymptotic classification. The explicit paths remain useful
baselines even where their representation operation is not the recommended
next target.

## Single recommended next experiment

The only recommended next experiment is **reusable compact representation
workspace**: add a `PartitionWorkspace` owned by one benchmark/solver lane,
whose node, incidence, and compressed-network buffers are deterministically
reset before each solve and reused across that lane's iterations. It must not
be shared between compact MRD, Hopcroft--Karp, and C0 flow. The paired
experiment should compare fresh allocation against capacity reuse on the two
P18 primary families, record retained capacity and
`representation_construction_ns`, and require identical partition ordering,
`sigma`, `M`, flow cover, optimum, and witness checksum. This is a lifecycle
proposal for the next experiment, not an implementation already present in
P18.

No selector, hybrid policy, zero-conflict shortcut, or representation rewrite is
implemented in P18. A zero-conflict fast path remains an unresolved alternative
for a later separately measured experiment, not part of this recommendation.

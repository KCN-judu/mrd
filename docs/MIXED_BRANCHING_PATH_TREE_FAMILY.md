# Mixed-Branching Path-Tree Evidence

The witness search is geometry-backed. It starts with ordinary finite unit-cell
regions, prepares the boundary and endpoint index, applies the clean-hole-free
certificate, and evaluates the BoundaryLaminar dual and endpoint-only HLD. No
synthetic tree is used as a production input.

The deterministic command is:

```text
cargo run --release -p mrd -- search-path-tree-witness \
  --max-width 12 --max-height 12 --seed 42 --require-clean \
  --min-horizontal-chords 2 --min-vertical-chords 2 \
  --min-dual-branching 3 --min-path-count 3 \
  --min-heavy-chain-intervals 4 --min-canonical-nodes 2 \
  --output-dir results/path-tree-witnesses
```

The search performs deterministic delta-debugging cell minimization before
dihedral canonicalization. The committed population contains 16 minimized,
translation/dihedral-canonical witnesses with 47--115 cells. Every witness
retains both chord orientations, degree at least 3, a path using multiple heavy
chains, and at least two canonical segment nodes. The bundles record the
original and minimized cell counts, input grid, dual tree, compact paths, HLD,
biclique partition, diagnostics, and SVG geometry.

The source population is a boundary-notch construction. Separated unit notches
are attached to the outer boundary, and deterministic one-cell mutations search
the neighboring clean population. Every retained result is rebuilt through the
production geometry; no final optimum value is used as the search objective.
Derived `mixed-branching-connected-sum` witnesses may be retained as regressions,
but they are excluded from the family seed population. This prevents the
committed output from changing the next search input and makes repeated search
runs byte-for-byte idempotent.

The parameterized `mixed-branching-connected-sum` family starts with the
smallest minimized witness and attaches another minimized witness gadget
through exactly one unit-width corridor. A join is accepted only after the
ordinary grid geometry, clean certificate, event-sweep dual, endpoint-only HLD,
and biclique partition all validate. The first four members are:

| modules | q | dual regions | path count | heavy-chain intervals | canonical nodes |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 6 | 4 | 3 | 4 | 2 |
| 2 | 14 | 9 | 6 | 10 | 6 |
| 3 | 22 | 13 | 10 | 15 | 8 |
| 4 | 30 | 17 | 14 | 20 | 11 |

All five required structural quantities increase strictly. The generated
scaling campaign extends the construction through eight modules and is stored
in `results/v0.8-path-tree-families.csv`. These are geometry-backed rows, not a
synthetic dual graph or a coordinate-only scaling claim.

The permanent regressions require `H >= 2`, `V >= 2`, dual branching degree at
least 3, at least three paths, at least one multi-heavy-chain path, and at least
two canonical segment nodes. A separate family regression requires strict
growth in q, dual regions, paths, heavy-chain intervals, and canonical nodes.

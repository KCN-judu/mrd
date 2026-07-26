# Mixed-Branching Path-Tree Evidence

The witness search is geometry-backed. It starts with ordinary finite unit-cell
regions, prepares the boundary and endpoint index, applies the clean-hole-free
certificate, and evaluates the BoundaryLaminar dual and endpoint-only HLD. No
synthetic tree is used as a production input.

The deterministic command is:

```text
cargo run --release -p rect-cli -- search-path-tree-witness \
  --max-width 12 --max-height 12 --seed 42 --require-clean \
  --min-horizontal-chords 2 --min-vertical-chords 2 \
  --min-dual-branching 3 --min-path-count 3 \
  --min-heavy-chain-intervals 4 --min-canonical-nodes 2 \
  --output-dir results/path-tree-witnesses
```

The committed population contains three minimized, translation/dihedral
canonical witnesses:

- `mutated-notch-057-054`: 332 foreground cells, five horizontal and three
  vertical chords, dual maximum degree 3, three paths, four heavy-chain
  intervals, one path crossing multiple heavy chains, and three canonical
  segment nodes;
- `mutated-notch-097-124`: 378 cells, four horizontal and four vertical
  chords, degree 3, four paths, four intervals, two multi-chain paths, and two
  canonical nodes;
- `mutated-notch-074-017`: 440 cells, seven horizontal and four vertical
  chords, degree 3, four paths, five intervals, two multi-chain paths, and four
  canonical nodes.

Inputs, dual trees, compact paths, HLD records, biclique partitions,
diagnostics, and SVG renderings are in `results/path-tree-witnesses/`.

The source family is a boundary-notch construction: separated unit notches
are attached to the outer boundary, and deterministic one-cell mutations are
used only to search the neighboring clean population. Increasing the number
of separated notch positions increases the boundary and chord populations;
the search records the resulting `q`, dual degree, path count, heavy-chain
intervals, and canonical-node count rather than extrapolating from a final
optimum value.

The witness regression requires `H >= 2`, `V >= 2`, dual branching degree at
least 3, at least three paths, at least one path using multiple heavy chains,
and at least two canonical segment nodes. The scaling campaign varies the
geometry-family parameter while retaining this predicate for stored witnesses.
The generated structural scaling campaign is in
`results/v0.8-path-tree-families.csv`. It varies the geometry-family scale from
3 through 64; the retained mixed witness bundles remain the canonical
predicate population rather than being claimed as a coordinate-only scaling
law.

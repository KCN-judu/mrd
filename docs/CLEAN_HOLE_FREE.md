# Clean Hole-Free Components

The v0.5 structural backend applies Definition 9.1 to the supported input
model: a finite four-connected component of unit grid cells. The adapter has no
ornament object, so `UnsupportedOrnamentModel` is not inferred for ordinary
inputs; arbitrary formal ornaments remain outside the model.

`sg-oracle::classify_clean_hole_free` checks one outer loop, zero ordinary
holes, proper chord interiors, boundary endpoint identities, and distinct
endpoints. Endpoint identities are `(loop_id, cyclic_index)` values from the
normalized integer boundary loops. Shared endpoints are rejected before any
alternation query.

For four distinct endpoints on one loop, `endpoints_alternate` uses modular
interval membership and handles wraparound without copying or rotating the
loop. FullyAudited compares this predicate with the closed integer chord
intersection predicate on every horizontal/vertical pair of a clean component.

The path-tree solver exposes four orientation policies: `build-both` constructs
both orientations and selects the smaller audited sigma; `bound-estimate`
chooses one orientation from the paper-shaped pre-construction bound; and the
  two fixed policies are debugging/benchmark controls. FullyAudited and
  CompactOnly both default to exact `build-both`. The v0.8 mixed-witness audit
  found positive regret for the heuristic, so it remains an explicit benchmark
  policy rather than the production default.
BoundaryLaminar uses an
axis-generic view for the horizontal-tree orientation, while the historical
coordinate-transpose implementation remains an audited reference.

The census artifacts count chord mass as well as components. A large number of
zero-chord rectangles is therefore not treated as evidence that the path-tree
representation covers substantial matching work. The current checked grid
census is generated with:

```text
mrd benchmark --suite clean-census --output results/v0.5-clean-census.csv
```

The command also writes `results/v0.5-clean-census.json` and
`results/v0.5-clean-census.md` beside the requested CSV.

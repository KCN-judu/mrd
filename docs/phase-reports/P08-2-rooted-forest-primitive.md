# P08.2 - Rooted Forest Primitive

## Contract

P8.2 adds `rect-graph::rooted_forest::DynamicRootedForest`, an exact,
deterministic baseline for the rooted-forest state required by Definitions 5.2
and 5.3 and the operation boundary of Lemma 5.4 in arXiv:2309.16629v1. The
forest edge set is decremental. Active graph edges may be deleted; a vertex
split moves an explicit set of active incident edges to a new singleton
forest vertex and removes any moved forest edge. Deterministic root restoration
maintains exactly one root per forest component.

The primitive supports exact root-path addition/query, Definition 5.3 stretch
calculation, static BFS certificate recomputation, and source-shaped recourse
counters. It is not a link-cut tree or the full low-stretch-forest construction,
and makes no Lemma 5.4 or amortized complexity claim.

## Evidence

- Four focused tests cover exact same-component stretch `8/5`, accepted and
  rejected stretch certificates, root-path updates and queries, deletion
  recourse/root creation, split detachment, and invalid repeated deletion.
- All graph mutation preconditions are checked: endpoint and positive-length
  validation, active-edge status, forest acyclicity, unique roots, and distinct
  active incident edges for a split.
- No fallback or performance selection exists; the static BFS stretch check is
  retained as the reference Oracle.

## Full audit

Phase baseline: `5f5720ee9a1cdac8cd52647fb8636a99fd2c2410`.

The complete required audit exited 0 in 25.1 seconds:

```text
git diff --check
cargo fmt --all -- --check
python3 tools/check_biclique_bound.py
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps
cargo build --workspace --release
python3 tools/check_release_consistency.py
```

Final diff inspection found no ignored tests, stale generated evidence,
fallbacks, credentials, tokens, private keys, or local absolute paths.

## Next action

P8.3 is restricted to simple undirected decremental spanner certificates.
It must reject directed, insertion, and arbitrary-update requests.

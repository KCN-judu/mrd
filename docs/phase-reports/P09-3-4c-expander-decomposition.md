# P09.3.4c - Certified Expander Decomposition

## Scope

P9.3.4c adds a finite-domain, one-level certificate shaped after the output
contract of Theorem 8.5. It does not implement the theorem's general
deterministic expander-decomposition construction and makes no CGLNPS20 or
source runtime claim.

## Construction

`graph::source_spanner::experiment::decomposition::single_level` accepts only
a connected simple graph with two through twenty vertices and a positive exact
`phi`. It chooses the deterministic level
`ceil(log2(ceil(m / n)))`, checks `m <= 2^level n`, and accepts only when the
minimum degree is at least `phi * 2^level` and the exact expansion is at least
`phi`.

The returned `Decomposition` has exactly one explicit `Component`. Its ordered
vertex list is the complete source vertex set and its ordered edge list is the
complete source edge set, so the finite-domain edge partition is checked by
equality and every edge occurs exactly once. The component records its minimum
degree, exact exhaustive expansion value, and number of nontrivial cuts
checked. `Decomposition::verify` rebuilds this complete certificate rather
than trusting any stored field.

`complete`, `decomposition`, and `domain` are separate public experiment
namespaces; private `certificate` contains the shared pure recomputation
functions for degrees, connectivity, expansion, and arithmetic conversion.
There is no old flat compatibility re-export.

## Explicit Limits

The construction rejects disconnected inputs, nonpositive `phi`, domains over
twenty vertices, and every input requiring a multi-level decomposition. The
single-level partition is intentionally trivial. It neither supplies a
general edge-disjoint expander decomposition nor proves the source's work or
depth bounds. P9.3.4d still owns Theorem 8.6 decremental paths; P9.3.4e still
owns Algorithm 4 integration.

## Focused Evidence

- The unit-weight complete graph on four vertices gives level one, one
  component, six source edges exactly once, and verifies after recomputation.
- A disconnected two-edge graph rejects before a component certificate is
  emitted.
- Complete-witness tests retain the exact degree-three, expansion-`2/3`,
  fourteen-cut certificate and reject both degree-sandwich and oversized-domain
  requests.

## Audit

Phase baseline: `64ce6f44e928564fac214b8b8960cc13999c0183`.
Implementation SHAs: `f9dd410`, `bce0f14`.

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatted |
| `python3 tools/check_biclique_bound.py` | 0 | bound checker passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test -p graph source_spanner::experiment -- --nocapture` | 0 | 3 passed |
| `cargo test --workspace --quiet` | 0 | 270 passed, 3 existing ignored |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | warning-free documentation |
| `cargo build --workspace --release` | 0 | optimized build passed |
| `python3 tools/check_release_consistency.py` | 0 | release metadata and baseline evidence passed |

The final diff and source-spanner namespace inspection found no fallback to the
simple-path Oracle, ignored P9.3.4c tests, stale generated evidence,
credentials, tokens, private keys, or local absolute paths. No generated
result file is required: all evidence is recomputed by the certificate verifier
and its tests.

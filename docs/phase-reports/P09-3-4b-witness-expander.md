# P09.3.4b - Certified Witness Expander

## Scope

P9.3.4b supplies a deterministic finite-domain witness for the degree and
expansion contract consumed by Algorithm 4. It is not the general
`ConstructExpander(n, w)` algorithm delegated to CGLNPS20, does not implement
an expander decomposition or decremental paths, and makes no source runtime
claim.

## Construction

The historic `graph::source_spanner::experiment::complete` fixture remains
available, but Algorithm 4 now uses
`graph::source_spanner::experiment::circulant` for positive levels. It tries
canonical circulant degrees in increasing order, accepts the first graph whose
per-vertex degree satisfies `w_v <= degree_v <= 18 w_v`, then enumerates every
nontrivial vertex cut to certify its exact positive expansion floor. The
certificate records the graph, degree vector, expansion, and checked-cut count;
`Witness::verify` reconstructs every field rather than trusting stored values.

Inputs outside the supplied exhaustive domain or outside the degree sandwich
are explicitly rejected. This restriction is intentional: no unverified degree
rounding, randomized sampling, generic expander claim, or CGLNPS20 runtime
claim is introduced.

## Evidence

- A five-vertex unit-weight request selects the five-edge cycle, has degree two,
  and passes the exhaustive positive-expansion certificate.
- A five-vertex degree-four request selects the complete graph only because its
  degree sandwich requires it.

## Audit

Phase baseline: `a71dceef117ae37decca82b7ea4cbe098af9b092`.
Implementation SHAs: `77878a8`, `cc54c10`.

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatted |
| `python3 tools/check_biclique_bound.py` | 0 | bound checker passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test -p graph source_spanner::experiment -- --nocapture` | 0 | 2 passed |
| `cargo test --workspace` | 0 | 269 passed, 3 existing ignored, 403.71s |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | warning-free documentation |
| `cargo build --workspace --release` | 0 | optimized build passed |
| `python3 tools/check_release_consistency.py` | 0 | release metadata and baseline evidence passed |

Final diff inspection found no fallback use, ignored P9.3.4b tests, stale
generated evidence, credentials, tokens, private keys, or local absolute paths.

# P09.3.4b - Certified Witness Expander

## Scope

P9.3.4b supplies a deterministic finite-domain witness for the degree and
expansion contract consumed by Algorithm 4. It is not the general
`ConstructExpander(n, w)` algorithm delegated to CGLNPS20, does not implement
an expander decomposition or decremental paths, and makes no source runtime
claim.

## Construction

`graph::source_spanner::experiment::complete` accepts at most 20 vertices and
positive exact weights only when the complete graph's degree `n - 1` satisfies
`w_v <= n - 1 <= 18 w_v` for every vertex. It constructs the complete graph
deterministically, enumerates every nontrivial vertex cut, computes the exact
minimum conductance-style expansion ratio, and stores the degree vector,
expansion, and number of checked cuts. `Witness::verify` recomputes every
measurement without trusting stored values.

Inputs outside the 20-vertex exhaustive domain or outside the degree sandwich
are explicitly rejected. This restriction is intentional: no unverified
degree rounding, randomized sampling, or generic expander claim is introduced.

## Evidence

- A four-vertex unit-weight witness has degree three, exact expansion `2/3`,
  and 14 checked nontrivial cuts.
- A 20-vertex unit-weight request fails the upper degree sandwich; a graph over
  the supplied finite domain fails before enumeration.

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

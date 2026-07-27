# P09 Dynamic Cycle Query Oracle

## Scope

`DynamicMinRatioReplay` previously exposed only coordinate replay operations.
This subphase adds a deterministic exact query over the current checked stable
ledger: it enumerates signed simple cycles, returns the least exact
gradient/length ratio, and records query and candidate totals.

## Boundary

The source query in CKLPPS22/Theorem 6.2 (and the deterministic counterpart's
Theorem 5.1) returns a compact approximate cycle using dynamic trees under a
hidden stable-flow-chasing contract. The new query is deliberately a
superlinear audit Oracle. It does not use a max-flow fallback, does not claim
compact output, and does not claim any source runtime or approximation bound.

The enumeration forbids reusing an edge ID, so a single edge immediately
traversed in reverse is not accepted as a false two-edge cycle. Parallel edges
remain valid two-edge cycles.

## Evidence

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo fmt --all` | 0 | formatted source |
| `cargo check -p rect-graph` | 0 | passed |
| `cargo clippy -p rect-graph --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test -p rect-graph dynamic_min_ratio -- --nocapture` | 0 | 4 passed, 34 filtered |
| `git diff --check` | 0 | no whitespace errors |

P9 remains incomplete because this Oracle cannot satisfy the source-grade
dynamic-query, forest, spanner, IPM, or integration requirements.

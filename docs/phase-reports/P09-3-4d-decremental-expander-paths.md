# P09.3.4d - Certified Decremental Path Semantics

## Scope

P9.3.4d adds an exact, deterministic deletion-state and bounded-path contract
needed before Algorithm 4 can consume a decremental path interface. It is not
the general Theorem 8.6 decremental expander-path structure and does not claim
its pruning rule, work, depth, or recourse bounds.

## Layers

`graph::source_spanner::decremental::state` is a pure immutable transition
model. It retains the source graph and stable edge IDs, records every requested
deletion with a sequence number and accepted/rejected outcome, and recomputes
the pruned set after every transition. In this constrained model a vertex is
pruned exactly when its active degree is zero. That set is monotone under edge
deletions, and `State::verify` replays the full trace without trusting stored
sets or outcomes.

`decremental::query` is the production path layer. After verifying the state,
it performs a stable-edge-ID breadth-first search over active edges and returns
one exact path, `Disconnected`, or `HopBoundExceeded`. Pruned and out-of-range
endpoints reject explicitly. It does not call a simple-path Oracle.

`decremental::certificate` is the isolated verification layer. It enumerates
bounded simple active paths with `source_spanner::oracle::simple_paths`, then
chooses the same semantic target by shortest hop count followed by edge-ID
lexicographic order. Its verifier rejects a production response that differs
from that independent result. The enumerator is never a query fallback.

## Limits

The isolated-vertex rule is a deliberately explicit finite semantic rule, not
the source's expander-cut pruning mechanism. Snapshots and traces are cloned
for clarity, so no decremental time, space, or recourse claim follows. The
enumerating certificate has exponential worst-case work and is restricted to
small differential fixtures. A source-backed general Theorem 8.6 construction
and matching operation counters remain required before any such claim.

## Focused Evidence

- Accepted, repeated, and out-of-range deletion requests are all preserved in
  a replayed trace; tampering with an outcome rejects the state certificate.
- Deleting the direct edge of a triangle deterministically replaces its
  one-hop path with the two-hop stable-ID path through the remaining vertex.
- An insufficient hop limit returns the exact shortest-hop count, while a
  pruned endpoint rejects explicitly.
- The independent certificate accepts the production path and hop-bound
  result, then rejects an intentionally mutated `Disconnected` response.

## Audit

Phase baseline: `d5bb65dbecf504ef1cafc97de8fc9895b854cd35`.
Implementation SHAs: `d5d80cc`, `f097cd4`, `838a321`.

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatted |
| `python3 tools/check_biclique_bound.py` | 0 | bound checker passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test -p graph source_spanner::decremental -- --nocapture` | 0 | 6 passed |
| `cargo test --workspace` | 0 | 276 passed, 3 existing ignored |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | warning-free documentation |
| `cargo build --workspace --release` | 0 | optimized build passed |
| `python3 tools/check_release_consistency.py` | 0 | release metadata and baseline evidence passed |

The final diff and namespace inspection found `simple_paths` only in the
Oracle itself and the independent `decremental::certificate`, never in the
production query layer. It found no ignored P9.3.4d tests, stale generated
evidence, credentials, tokens, private keys, or local absolute paths. No
generated result file is required: state and path evidence are recomputed by
the certificate verifier and its tests.

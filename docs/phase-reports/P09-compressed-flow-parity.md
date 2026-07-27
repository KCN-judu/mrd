# P09 Compressed Flow Parity

## Scope

`audit_biclique_flow_parity` proves the present compressed-flow boundary is
consistent across independent exact references. It first verifies that a
biclique partition represents the explicit graph exactly, then compares the
explicit Hopcroft--Karp matching cardinality with compressed Dinic and
compressed Push--Relabel flow/cut recovery.

## Evidence

The differential test enumerates all 16 two-by-two explicit bipartite graphs.
For each graph it builds the singleton-edge partition and requires all three
exact paths to agree on matching and recovered-cover cardinality. An invalid
partition is rejected before a compressed network is built.

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo fmt --all` | 0 | formatted source |
| `cargo check -p rect-dominance` | 0 | passed |
| `cargo clippy -p rect-dominance --all-targets -- -D warnings` | 0 | no warnings |
| direct `rect_dominance` test binary, exact parity test | 0 | all 16 graphs passed in 0.00s after the independent Push--Relabel regression fix |
| `git diff --check` | 0 | no whitespace errors |

## Push--Relabel regression closure

| Field | Evidence |
| --- | --- |
| Classification | implementation gap and test-evidence gap |
| Observed | The first clean `cargo test -p rect-graph` run after stale-process cleanup failed `push_relabel_matches_dinic_on_deterministic_networks`: the replacement FIFO implementation reported `global_relabel_count == 0` despite the public highest-label/global-relabel/gap contract. |
| Expected | Preserve the P5 Goldberg--Tarjan exact backend: highest-label active selection, reverse-residual global relabeling, the gap heuristic, terminating discharge, and exact flow/cut equality with Dinic. |
| Cause | The earlier highest-label implementation capped all unreachable heights at `n + 1`. A trapped preflow that needed to return through multiple vertices could not acquire heights above that cap and looped. Replacing the implementation with FIFO discharge removed the loop but also removed the promised heuristics. |
| Change | Restore highest-label selection, initial and periodic global relabeling, and gap relabeling. Heights above `n + 1` are now allowed to grow according to the residual admissibility rule, so trapped excess can return to the source without weakening the algorithm contract. |
| Focused acceptance | `cargo test -p rect-graph`: 41 passed; `cargo test -p rect-dominance compressed_flow`: 3 passed, 33 filtered; the new multi-vertex trapped-excess regression terminates with a valid unit flow and cut. |
| Full acceptance | `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`, warning-free workspace rustdoc, release build, and both repository consistency scripts all exited 0. The workspace run reported 186 passed and 3 ignored across 13 suites. |

The fix restores a permanent practical exact reference backend only. It does
not supply any P9 almost-linear theorem component or justify an `AlmostLinear`
backend name.

## Boundary

This establishes integration parity only for permanent reference backends. It
does not select or emulate an almost-linear backend, so P9 remains incomplete.

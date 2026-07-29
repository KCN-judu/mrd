# P09.5 - Exact IPM-to-Source Provenance

## Status

**State: complete as the P9.5a provenance substep; P9.5a selection remains
blocked.** Implementation SHA: `91132c4`.

## Scope and contract

`graph::source_min_ratio::input::Input` is a pure projection from one exact
current circulation coordinate vector to source structural identities. Given a
`CirculationNetwork` and exact caller-supplied gradient, length, and tree-weight
vectors, it validates dimensions, positivity of lengths and tree weights,
non-loop arcs, and a stable orientation-preserving
`SourceEdgeId <-> CirculationArcId` mapping. `materialize()` constructs the
positive structural `SourceDynamicGraph` and checked `ArcBindings` in the same
operation.

Signed IPM gradients are retained as coordinate data and are never used as tree
weights. The tree weights are an independent positive input to the later source
tree construction. The API also does not manufacture an exact rational
approximation from `CertifiedIpmSnapshot` intervals: a future caller must supply
the exact vectors that its Lemma 4.4 certificate checks.

## Source basis

van den Brand et al., arXiv:2309.16629v1, Section 5.4 Algorithm 1 and the
proof of Lemma 5.11 identify maintained tree-chain/core/spanner structures and
a heap over fundamental spanner and terminal-level tree cycles. Appendix A.3,
Definition A.1 and Lemma A.2 define fundamental chain cycles and their
quality relation. The paper therefore supports explicit provenance feeding
maintained candidate structures; it does not justify an enumerating selector.

## Evidence

The focused tests establish that materialized arc bindings preserve orientation
through a compact-cycle decode, including a signed gradient that remains
separate from structural tree weight. Invalid vector dimensions and nonpositive
tree weights reject explicitly. `tools/check_source_min_ratio_audit.py`
requires the new production input module and continues to reject the enumerating
Oracle boundary.

The implementation SHA passed:

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo fmt --all -- --check` | 0 | formatting accepted |
| `git diff --check` | 0 | no whitespace errors |
| `python3 tools/check_biclique_bound.py` | 0 | compact-biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | source minimum-ratio boundary accepted |
| `python3 tools/check_source_flow_audit.py` | 0 | source-flow no-fallback boundary accepted |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | workspace suite passed |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | rustdoc accepted with warnings denied |
| `cargo build --workspace --release` | 0 | release build accepted |

## Non-claims and next action

This substep does not construct a source tree chain, shifts, core/spanner
embeddings, fundamental candidates, a heap query, or an exact quality
certificate for `Step`. It does not select or enumerate a cycle, expose the
hidden stability witness, invoke `dynamic_min_ratio`, or establish Theorem 5.1
or an AN19 runtime bound.

The next P9.5a substep is to connect this provenance to the maintained
tree-chain and explicit embeddings, then implement the source-specified
fundamental-candidate heap and its exact certificate. P9.3.2d remains the
separate low-priority P9.6a proof debt; it does not permit a selection shortcut
and does not block this semantic integration work.

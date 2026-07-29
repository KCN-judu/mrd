# P09.5a.2 - Exact Fundamental-Candidate Heap

## Status

**State: complete as finite source-declared candidate maintenance; P9.5a live
selection remains blocked.** Implementation SHA: `0bf9d37`.

## Scope

`graph::source_min_ratio::candidate` implements the narrow `FindCycle()`
maintenance boundary supported by the primary source. A `Context` revalidates
the exact `Input` materialization before a `Registry` accepts only explicitly
declared `FundamentalSpanner` or `FundamentalTree` compact cycles. The registry:

- verifies each candidate's fundamental anchor and terminal-tree shape;
- decodes the supplied compact cycle through checked graph, chain, shifts, and
  arc bindings;
- aggregates repeated arc occurrences before computing the exact ratio
  `|<gradient, delta>| / ||length * delta||_1`;
- maintains a deterministic binary heap with stable candidate-ID ties and
  auditable stale records after source-driven replacement or retirement; and
- returns a nonzero candidate reversed when necessary so its gradient dot
  product is negative for the next IPM update boundary.

This is a finite exact semantic contract. Its counters record operations but do
not assert a source amortized or priority-queue bound.

## Source basis

van den Brand et al., arXiv:2309.16629v1, Algorithm 1 `FindCycle()` and the
proof of Lemma 5.11 prescribe taking the best ratio among maintained
fundamental spanner cycles and terminal-level fundamental tree cycles, tracking
their qualities as embeddings change with a heap. Appendix A.3, Definition A.1
and Lemma A.2 define the associated fundamental chain cycle and quality basis.
The implementation mirrors that maintenance boundary only; it does not claim
the live tree-chain or embedding construction that supplies the population.

## Tests and audit

The focused graph tests cover two distinct declared fundamental cycles, exact
ratio ordering, deterministic descent orientation, stale heap cleanup after a
replacement, duplicate/unanchored/nonterminal rejection, and an empty
declaration set that remains empty rather than triggering enumeration.

Implementation SHA `0bf9d37` passed:

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo fmt --all -- --check` | 0 | formatting accepted |
| `git diff --check` | 0 | no whitespace errors |
| `python3 tools/check_biclique_bound.py` | 0 | compact-biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | candidate boundary has no Oracle fallback |
| `python3 tools/check_source_flow_audit.py` | 0 | source-flow no-fallback boundary accepted |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | workspace suite passed |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | rustdoc accepted with warnings denied |
| `cargo build --workspace --release` | 0 | release build accepted |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

## Non-claims and next action

The registry does not create a live source tree chain, selected shifts,
core/spanner embeddings, or candidate declarations. It does not expose the
hidden stability witness, import `dynamic_min_ratio`, enumerate graph cycles,
or certify the chosen compact cycle as a Lemma 4.4 `Step`. It makes no Theorem
5.1, AN19 priority-queue, amortized, or runtime claim.

P9.5a.3 must construct the live source structures that emit the registry's
fundamental declarations and connect its chosen cycle to the exact current
approximation certificate consumed by `Step::from_compact_candidate`.

# P09.5a - Source Compact-Candidate Selection Gap

## Status

**State: blocked.** This report records the P9.5a semantic construction that
is still absent at baseline `d11cb3f`. It is independent of the P9.3.2d AN19
runtime proof debt: P9.3.2d remains deferred to low-priority P9.6a after the
complete source-shaped flow backend exists. P9.5a instead blocks the backend
from selecting the next exact source-shaped IPM direction.

## Audit evidence

The audited production boundary has the following deliberate shape:

| Module | Implemented responsibility | Deliberate absence |
| --- | --- | --- |
| `graph::min_ratio_cycle::StableMinRatioLedger` | checks stable-witness validity, update quality, exact coordinate queries, and Detect accounting | neither `StableEdge` nor the consumed `StableWitness` input carries a compact-cycle selection or source-arc provenance |
| `graph::source_min_ratio::model` and `chain` | represent validated immutable source-tree branches and deterministic shifts | no constructor derives a tree-chain from a live IPM snapshot |
| `graph::source_min_ratio::cycle` | decodes a supplied compact cycle through selected branches and checked arc bindings | no candidate generation or score computation |
| `graph::source_min_ratio::query` | validates a supplied compact candidate against a checked ledger | no minimum-ratio selection query |
| `graph::source_min_ratio::execution` | applies supplied ledger transitions and records finite accounting | no dynamic sparsification, link-cut maintenance, or cycle search |
| `graph::source_flow::iteration` | converts a supplied compact candidate to an exact direction and applies a certified Lemma 4.4 update | no caller-free source candidate selection |

`StableMinRatioLedger::edges()` intentionally exposes only the checked
coordinates used by an independent audit. `StableWitness` is consumed during
ledger construction; the retained stability floors are not a direction witness
and do not identify a compact cycle. The public source graph, tree chain, and
circulation bindings use different stable-ID domains. No current API relates
those domains to each other or to the live gradient and length vectors in
`CertifiedIpmSnapshot`.

The existing `dynamic_min_ratio` and min-cost cycle implementations can
enumerate candidates, but they are reference Oracles. The P9.5 source-flow
static audit rejects `dynamic_min_ratio`, `min_cost::oracle`, and
`min_cost::experiment` in its production modules. The source-min-ratio audit
separately keeps the tree-chain boundary free of enumerating-cycle imports.
These references remain valid only for bounded test differentials.

## Required construction

P9.5a must add a source-shaped selector with an explicit input/output
certificate. Given a live certified IPM snapshot, it must:

1. build and validate a source dynamic graph with stable correspondence to the
   snapshot's circulation coordinates;
2. build or maintain the source tree-chain, selected shifts, and arc bindings
   with the same provenance;
3. invoke a source-defined candidate-selection operation that returns one
   compact cycle without returning the stability-witness input;
4. decode the candidate and certify its full exact direction against the
   snapshot's current approximate gradients, lengths, and `kappa`; and
5. reject unsupported source operations without choosing an enumerating,
   Dinic, Push--Relabel, or min-cost fallback.

The construction must make all ID mappings explicit. A conversion based only on
ledger index, a tree branch's storage slot, or matching endpoint pairs is not
sufficient: those values do not establish live residual-coordinate provenance.

## Rejected shortcuts

- Returning `StableWitness` would not construct a compact candidate and would
  weaken the stability-input boundary.
- Enumerating fundamental or residual cycles would import an Oracle as the
  production algorithm.
- Selecting the first decodable compact cycle would have no source-defined
  quality guarantee for the Lemma 4.4 transition.
- Treating a finite differential fixture as a selector certificate would turn
  test evidence into a production assumption.

## Audit

The documentation and static-boundary audit passed at baseline `d11cb3f`:

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatting accepted |
| `python3 tools/check_biclique_bound.py` | 0 | compact-biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | source minimum-ratio boundary has no Oracle fallback |
| `python3 tools/check_source_flow_audit.py` | 0 | source-flow boundary has no reference-flow or recovery fallback |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | workspace tests passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | rustdoc accepted with warnings denied |
| `cargo build --workspace --release` | 0 | release build accepted |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

No production code or generated result file changed. The report records the
interface evidence needed to resume this construction without depending on a
previous session's memory.

## Next action

Find or construct the missing source-level mapping and query semantics before
adding any selector API. The design must identify the exact source operation
that selects a compact candidate, its required maintained data, and the
certificate that connects it to the current `CertifiedIpmSnapshot`. Only then
may P9.5 connect it to `Step::from_compact_candidate`, run the full
no-fallback differential campaign, and enable `Backend::require_complete()`.

P9.6a remains after that chain is complete. It is the separate low-priority
task to prove or replace the AN19 reduced-event ordering and hierarchy-wide
amortization obligations; it does not authorize a P9.5 candidate-selection
shortcut.

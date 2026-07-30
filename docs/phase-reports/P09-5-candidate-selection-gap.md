# P09.5a - Source Compact-Candidate Selection Gap

## Status

**State: in_progress.** Commits `91132c4`, `0bf9d37`, `abb77ac`, `5afa4c7`,
`cdb2ce9`, and `9238b37` close the provenance, source-declared candidate heap,
single-snapshot terminal-tree projection, terminal-only `Step` bridge, finite
rejected-core declarations, and finite same-network core recourse substeps of
P9.5a. The complete source-defined compact-candidate selector is still absent.
P9.5a is independent of the P9.3.2d AN19 runtime proof debt:
P9.3.2d remains deferred to low-priority P9.6a after the complete
source-shaped flow backend exists. P9.5a instead blocks the backend from
selecting the next exact source-shaped IPM direction.

## Audit evidence

The audited production boundary has the following deliberate shape:

| Module | Implemented responsibility | Deliberate absence |
| --- | --- | --- |
| `graph::min_ratio_cycle::StableMinRatioLedger` | checks stable-witness validity, update quality, exact coordinate queries, and Detect accounting | neither `StableEdge` nor the consumed `StableWitness` input carries a compact-cycle selection or source-arc provenance |
| `graph::source_min_ratio::input` | validates exact caller-supplied gradient/length/tree-weight vectors, assigns stable source-edge/circulation-arc provenance, and materializes `SourceDynamicGraph` with matching `ArcBindings` | does not infer an exact approximation from a snapshot interval, construct a tree chain, or choose a candidate |
| `graph::source_min_ratio::candidate` | validates externally declared fundamental spanner/tree compact cycles, computes exact current quality, maintains a deterministic stale-record heap, and orients a nonzero choice for descent | does not construct the tree chain, core/spanner embeddings, or candidate population; it cannot produce a Lemma 4.4 certificate |
| `graph::source_min_ratio::spanner` | builds one immutable finite Section 9.1 singleton-core snapshot, translates selected stable paths, declares every rejected core edge as an oriented, contiguous `SpannerPath` plus anchor edge, and derives same-network stable-ID recourse | no terminal population merge, complete `Step`, general dynamic maintenance, or runtime claim |
| `graph::source_min_ratio::terminal` | constructs one AN19-shaped static terminal tree for a live exact `Input`, retains the tree certificate, forms a one-level checked chain, and declares every non-tree source edge as its terminal fundamental cycle | no core/spanner population merge or cross-snapshot replacement/retirement |
| `graph::source_min_ratio::model` and `chain` | represent validated immutable source-tree branches and deterministic shifts | no constructor derives a tree-chain from a live IPM snapshot |
| `graph::source_min_ratio::cycle` | decodes a supplied compact cycle through selected branches and checked arc bindings | no candidate generation or score computation |
| `graph::source_min_ratio::query` | validates a supplied compact candidate against a checked ledger | no minimum-ratio selection query |
| `graph::source_min_ratio::execution` | applies supplied ledger transitions and records finite accounting | no dynamic sparsification, link-cut maintenance, or cycle search |
| `graph::source_flow::iteration` | converts a supplied compact candidate to an exact direction and, for terminal declarations only, selects the terminal registry's best candidate after exact equality with the terminal input coordinates | no caller-free complete source candidate selection; terminal-only declarations are not a core/spanner population |

`StableMinRatioLedger::edges()` intentionally exposes only the checked
coordinates used by an independent audit. `StableWitness` is consumed during
ledger construction; the retained stability floors are not a direction witness
and do not identify a compact cycle. `input::Input` now supplies the exact
source-edge/circulation-arc correspondence and materializes the matching graph
and bindings in one checked operation. It deliberately takes caller-supplied
exact approximation vectors rather than deriving a rational point from
`CertifiedIpmSnapshot` intervals. `candidate::Registry` now consumes exact
declared compact candidates and evaluates their quality through that
provenance. `terminal::Tree` now constructs the terminal branch and its
non-tree declarations directly from a static source-tree certificate.
`spanner::Snapshot` additionally constructs a finite rejected-core population
from the source Section 9.1 chain and rejects both tree-path and
noncontiguous-path substitutions. `spanner::Transition` applies source-declared
same-network insert/refresh/retire recourse only to a matching prior registry.
The complete combined terminal/core population remains absent.

The existing `dynamic_min_ratio` and min-cost cycle implementations can
enumerate candidates, but they are reference Oracles. The P9.5 source-flow
static audit rejects `dynamic_min_ratio`, `min_cost::oracle`, and
`min_cost::experiment` in its production modules. The source-min-ratio audit
separately keeps the tree-chain boundary free of enumerating-cycle imports.
These references remain valid only for bounded test differentials.

## Required construction

P9.5a must add a source-shaped selector with an explicit input/output
certificate. The stable projection, finite terminal/rejected-core declarations,
and heap over supplied declarations are complete; the remaining work must:

1. merge the current terminal and rejected-core declarations into one checked
   population with stable identities and exact source provenance;
2. extend the completed terminal-only exact-coordinate bridge to the complete
   maintained population and certify its selected compact direction with the
   current approximation gradients, lengths, and `kappa`; and
3. reject unsupported source operations without choosing an enumerating,
   Dinic, Push--Relabel, or min-cost fallback.

The construction must make all ID mappings explicit. A conversion based only on
ledger index, a tree branch's storage slot, or matching endpoint pairs is not
sufficient: those values do not establish live residual-coordinate provenance.

## Primary-source basis

The primary source is van den Brand et al., arXiv:2309.16629v1. Algorithm 1,
Section 5.4, `FindCycle()` returns the best exact-ratio fundamental spanner or
terminal-level tree cycle. The proof of Lemma 5.11 specifies the candidate
population: rejected core edges produce fundamental spanner cycles, terminal
edges produce fundamental tree cycles, known embeddings permit their quality
to be updated, and a heap returns the best one. Appendix A.3, Definition A.1
and Lemma A.2 define the associated fundamental chain cycles and relate
their quality to the hidden witness.

Accordingly, a production selector may not replace these maintained candidates
with simple-cycle enumeration. The completed provenance, terminal declarations,
and heap are necessary but not sufficient: live source core/spanner embedding
maintenance must feed the full declared candidate population.

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

The static boundary and complete workspace audit passed for implementation SHA
`0bf9d37` after the provenance and candidate-heap substeps:

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

The production changes establish exact input provenance, a heap over only
externally declared candidates, static terminal declarations, and immutable
finite rejected-core declarations with exact compact-cycle decoding. They do
not add cross-snapshot embedding maintenance, a complete selector, a runtime
claim, or a generated result file.

## Next action

The finite Algorithm 4 replay now satisfies the single-snapshot declaration
requirement: positive-level circulant witnesses and independent Task 3 paths
produce rejected-core edges on K5, and `spanner::Snapshot` records each compact
embedding cycle. Do not recast this as a complete selector. The next action is
P9.5a.3.2b now maintains the finite core population across same-network
snapshots. The next action is P9.5a.3.3b: merge it with terminal declarations
and run the no-fallback `Step` differential. Only after that campaign may P9.5
enable `Backend::require_complete()`.

P9.6a remains after that chain is complete. It is the separate low-priority
task to prove or replace the AN19 reduced-event ordering and hierarchy-wide
amortization obligations; it does not authorize a P9.5 candidate-selection
shortcut.

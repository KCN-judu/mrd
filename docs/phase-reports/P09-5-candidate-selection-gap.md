# P09.5a - Source Compact-Candidate Selection Gap

## Status

**State: blocked.** Commits `91132c4` and `0bf9d37` close the provenance and
source-declared candidate-heap substeps of P9.5a, but the source-defined
compact-candidate selector is still absent. P9.5a is
independent of the P9.3.2d AN19 runtime proof debt: P9.3.2d remains deferred to
low-priority P9.6a after the complete source-shaped flow backend exists. P9.5a
instead blocks the backend from selecting the next exact source-shaped IPM
direction.

## Audit evidence

The audited production boundary has the following deliberate shape:

| Module | Implemented responsibility | Deliberate absence |
| --- | --- | --- |
| `graph::min_ratio_cycle::StableMinRatioLedger` | checks stable-witness validity, update quality, exact coordinate queries, and Detect accounting | neither `StableEdge` nor the consumed `StableWitness` input carries a compact-cycle selection or source-arc provenance |
| `graph::source_min_ratio::input` | validates exact caller-supplied gradient/length/tree-weight vectors, assigns stable source-edge/circulation-arc provenance, and materializes `SourceDynamicGraph` with matching `ArcBindings` | does not infer an exact approximation from a snapshot interval, construct a tree chain, or choose a candidate |
| `graph::source_min_ratio::candidate` | validates externally declared fundamental spanner/tree compact cycles, computes exact current quality, maintains a deterministic stale-record heap, and orients a nonzero choice for descent | does not construct the tree chain, core/spanner embeddings, or candidate population; it cannot produce a Lemma 4.4 certificate |
| `graph::source_min_ratio::model` and `chain` | represent validated immutable source-tree branches and deterministic shifts | no constructor derives a tree-chain from a live IPM snapshot |
| `graph::source_min_ratio::cycle` | decodes a supplied compact cycle through selected branches and checked arc bindings | no candidate generation or score computation |
| `graph::source_min_ratio::query` | validates a supplied compact candidate against a checked ledger | no minimum-ratio selection query |
| `graph::source_min_ratio::execution` | applies supplied ledger transitions and records finite accounting | no dynamic sparsification, link-cut maintenance, or cycle search |
| `graph::source_flow::iteration` | converts a supplied compact candidate to an exact direction and applies a certified Lemma 4.4 update | no caller-free source candidate selection |

`StableMinRatioLedger::edges()` intentionally exposes only the checked
coordinates used by an independent audit. `StableWitness` is consumed during
ledger construction; the retained stability floors are not a direction witness
and do not identify a compact cycle. `input::Input` now supplies the exact
source-edge/circulation-arc correspondence and materializes the matching graph
and bindings in one checked operation. It deliberately takes caller-supplied
exact approximation vectors rather than deriving a rational point from
`CertifiedIpmSnapshot` intervals. `candidate::Registry` now consumes exact
declared compact candidates and evaluates their quality through that
provenance, but tree-branch and embedding maintenance still cannot construct
the candidate declarations.

The existing `dynamic_min_ratio` and min-cost cycle implementations can
enumerate candidates, but they are reference Oracles. The P9.5 source-flow
static audit rejects `dynamic_min_ratio`, `min_cost::oracle`, and
`min_cost::experiment` in its production modules. The source-min-ratio audit
separately keeps the tree-chain boundary free of enumerating-cycle imports.
These references remain valid only for bounded test differentials.

## Required construction

P9.5a must add a source-shaped selector with an explicit input/output
certificate. The stable projection and the heap over supplied declarations are
complete; the remaining work must:

1. attach a tree-chain, selected shifts, and source core/spanner embeddings to
   the materialized graph with the same provenance;
2. construct and update the fundamental spanner cycles for rejected core edges
   and the fundamental tree cycles at the terminal level as declarations for
   `candidate::Registry`;
3. connect the exact heap choice to the decoded full direction and certify it
   against the snapshot's current approximate gradients, lengths, and `kappa`;
   and
4. reject unsupported source operations without choosing an enumerating,
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
with simple-cycle enumeration. The completed provenance and heap are necessary
but not sufficient: live source tree-chain/embedding maintenance must feed the
declared candidate population.

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

The production changes establish exact input provenance and a heap over only
externally declared candidates. They do not add live candidate construction, a
complete selector, a runtime claim, or a generated result file.

## Next action

Implement the live source-maintained tree chain and its core/spanner embeddings
over the materialized `Input`, then emit/replace the fundamental spanner/tree
declarations consumed by `candidate::Registry`. Connect its selected compact
cycle to an exact certificate for `Step::from_compact_candidate`. Only then may
P9.5 run the full no-fallback differential campaign and enable
`Backend::require_complete()`.

P9.6a remains after that chain is complete. It is the separate low-priority
task to prove or replace the AN19 reduced-event ordering and hierarchy-wide
amortization obligations; it does not authorize a P9.5 candidate-selection
shortcut.

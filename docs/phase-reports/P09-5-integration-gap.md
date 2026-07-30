# P09.5 - Source-Shaped Flow Integration Gap

The certified IPM exposes public snapshots, update metrics, additive-half
termination, and recovery APIs. Inspection established that the older
snapshot-recovery route invokes the permanent exact rounding implementation.
The residual refinement experiment likewise depends on an enumerating
residual-cycle implementation. Neither may be called by `source_flow::Backend`.

Commit `094a289` adds `source_flow::recovery::round`, an independent,
deterministic exact fractional-cycle cancellation. `Backend::recover_terminated`
first certifies the additive-half boundary, invokes only that local recovery,
checks equality with the snapshot's retained integral optimum, and validates
the final integral circulation. It retains the complete cancellation trace.
The test-only differential compares the complete trace on a shared fractional
cycle with the permanent recovery implementation; production code has no such
dependency. `tools/check_source_flow_audit.py` rejects reference max-flow,
enumerating min-cost, legacy snapshot-recovery, and dynamic-cycle dependencies
from the production P9.5 boundary.

Commit `b34be66` adds `source_flow::iteration::Session`. It owns a certified
snapshot and its `IpmDetectLedger`; an immutable `Step` supplies exact
direction, approximations, and `kappa`. The session applies the existing
Lemma 4.4 transition, records the accepted step, and exposes exact Detect
threshold accounting. This connects update semantics to recovery without
using a reference flow path. It deliberately does not select `Step.direction`:
the P9.4 query boundary validates candidate cycles but does not construct the
minimum-ratio update required by the source iteration.

Commit `1a95a59` adds `Step::from_compact_candidate`. It calls the P9.4 query
boundary only to decode an externally selected compact cycle, then sums signed
arc occurrences into a complete exact direction vector and revalidates its
circulation. This binds compact source-tree semantics to the P9.5 step input
without selecting a candidate or using an enumerating cycle implementation.

Commit `91132c4` supplies the first P9.5a input bridge before any tree chain is
chosen. `source_min_ratio::input::Input` accepts exact caller-supplied IPM
gradient and length vectors plus independent positive structural tree weights,
validates stable `SourceEdgeId <-> CirculationArcId` provenance, then
materializes `SourceDynamicGraph` and its `ArcBindings` together. The compact
decode test proves that provenance survives one supplied chain. It neither
derives exact approximations from snapshot intervals nor constructs the source
tree chain, core/spanner embeddings, candidate heap, or Lemma 4.4 quality
certificate; P9.5a therefore remained incomplete on selection semantics.

Commit `cdb2ce9` closes the finite P9.5a.3.2a declaration gap. It replaces the
degenerate complete witness with an exactly certified positive-level circulant,
replays an independent `J -> W` embedding, and exposes the resulting finite
Section 9.1 rejected-core paths through `source_min_ratio::spanner::Snapshot`.
Each path becomes an explicit oriented `SpannerPath` and anchor-edge compact
cycle, which the exact registry decodes and scores. This is deliberately one
immutable snapshot; it does not supply cross-snapshot recourse, a merged
terminal/core population, a `Step` bridge, or an AN19 runtime claim.

Commit `0bf9d37` adds the P9.5a.2 exact registry over source-declared compact
candidates. It validates fundamental spanner/tree shapes, decodes them through
the checked provenance boundary, calculates exact absolute quality from the
current gradient and length vectors, maintains a deterministic stale-record
heap over source-driven updates, and reverses a positive choice for descent.
It cannot create a tree chain, spanner embedding, or candidate declaration,
and it does not yet connect the choice to a current `Step` certificate.

Commit `abb77ac` adds P9.5a.3.1's static terminal-tree projection. It runs the
exact AN19-shaped hierarchy over the current materialized `Input`, retains the
source tree certificate, forms one checked terminal branch, and declares the
unique tree-path-plus-edge compact cycle for every non-tree source edge. It is
direct candidate construction from source state, not cycle enumeration. It does
not attach core/spanner embedding provenance, update candidates across
snapshots, or certify a selected direction as a `Step`.

Commit `8d7975b` adds source-flow terminal recovery through both initial-point
augmentation and lower-bound normalization. It first recovers the augmented
integral circulation, rejects any surviving artificial arc, restores the
normalized network, then restores original lower bounds and objective offset.
The focused lower-bound fixture includes a fixed edge, a negative lower bound,
and artificial root arcs; all are checked without a reference recovery path.

Commit `6179f22` removes an implicit residual-cycle Oracle from this recovery
boundary. `source_flow` now uses a narrow exact feasibility-and-objective
validator after the additive-half certificate and exact snapshot-optimum
equality establish optimality. Augmentation and lower-bound restoration use
corresponding feasibility-only maps, while the permanent full optimality
validators remain available for reference code. The static audit now rejects a
direct optimality-validator call from production `source_flow`; a regression
test demonstrates that feasibility validation does not itself claim
optimality.

Commit `08eaae4` adds the first exact bridge from compressed biclique flow to
the source-flow circulation domain. Its only negative-cost arc returns sink
flow to the source, so the circulation objective is the negated matching
value. From an externally certified terminal solution, recovery pairs active
left and right incidences in each complete biclique, checks every outer arc,
then derives and validates a Konig cover. Production code uses only exact
feasibility validation; the permanent min-cost, Dinic, and Push--Relabel
implementations occur only in the bounded test differential. The source-flow
audit now includes this production module. The two current fixtures cover a
two-by-two explicit-edge differential and a single-biclique terminal snapshot
recovered through `source_flow::Backend`.

Commit `0359194` adds a third, chord-level differential using the repository's
MRD chord types. It derives the four-dimensional dominance embedding and its
Theorem 8 compact partition, checks the exact partition and strict dominance,
then requires source-circulation recovery to agree with the explicit matching,
Dinic, Push--Relabel, and min-cost references. This exercises a genuine
compressed chord graph, but it does not yet carry the selected cover through
the full polygon rectangle-recovery workflow.

Commit `40bb2f1` closes that specific fixture gap: its recovered cover is
converted to selected chord flags and passed through formal-polygon completion
for the source Figure 3 formal input. The resulting rectangle count equals the
formal optimum formula. The test supplies its terminal circulation through the
permanent min-cost implementation under `#[cfg(test)]`; it is a differential
of recovery and completion semantics, not source-flow candidate selection.

P9.5 remains open. Source candidate selection, MRD compressed-network
differential evidence for flow, cut, cover, chords, and rectangles, and an
end-to-end no-fallback audit are still absent.
`Backend::require_complete()` therefore continues to reject execution and
`an19_runtime_verified` remains false. P9.3.2d's missing AN19 proof is a
separate low-priority debt: it does not block these semantic integration tasks,
but must be closed in P9.6a before an `AlmostLinear` name or AN19 runtime
claim.

## P9.5a candidate-selection audit

The focused audit began at `d11cb3f`; implementation SHA `91132c4` closes its
provenance row, `0bf9d37` closes its supplied-candidate heap row, `abb77ac`
closes its terminal-tree declaration row, `5afa4c7` closes its terminal-only
`Step` bridge, and `cdb2ce9` closes the finite rejected-core declaration row.
The missing live selector remains a concrete semantic construction, not a
license to reuse an Oracle and not the deferred P9.3.2d runtime proof debt.

| Observed boundary | Evidence | Consequence |
| --- | --- | --- |
| `StableMinRatioLedger` | Its public `edges()` slice contains only anonymous `StableEdge` coordinates; `StableWitness` is consumed at construction and only checked stability floors are retained. | Neither coordinate identity nor the witness input identifies a `source_min_ratio::cycle::Cycle`. |
| `source_min_ratio::input::Input` | It validates caller-supplied exact current coordinates and constructs an orientation-preserving source-edge/circulation-arc binding with the source graph. | Stable arc provenance is now available; it still does not create a tree chain, embeddings, a candidate set, or a selection certificate. |
| `source_min_ratio::candidate::Registry` | It accepts only declared fundamental spanner/tree compact cycles, computes their exact current quality, and chooses the best nonzero candidate with deterministic stale-record handling. | It cannot discover candidate declarations from a graph, construct source embeddings, or certify the chosen direction as a `Step`. |
| `source_min_ratio::terminal::Tree` | It materializes an exact source tree from one live `Input`, retains the AN19-shaped certificate, and declares every non-tree terminal fundamental cycle. | It has no core/spanner embeddings, cross-snapshot candidate maintenance, or `Step` certificate. |
| `source_min_ratio::spanner::Snapshot` | It constructs one exact finite singleton-core chain, translates every rejected core edge's selected spanner path, and emits an explicit oriented compact cycle. | It is immutable and one-snapshot only; it does not merge terminal candidates, perform replacement/retirement, or create a `Step`. |
| `source_min_ratio::query::decode_candidate` | The API accepts a caller-supplied compact `Cycle`; its result contains decoded circulation arcs only. | It validates an already selected candidate and cannot select one. |
| `source_min_ratio::execution::Executor` | It forwards supplied `Update`, `Query`, and `Detect` ledger transitions and rejects unsupported source-grade operations. | It has no minimum-ratio query or compact-candidate search operation. |
| `source_flow::iteration::Step::from_terminal_candidate` | It checks that the caller's current coordinates exactly equal the immutable terminal `Input`, obtains the terminal declaration heap's best nonzero choice, and decodes it into a full exact circulation direction. | It cannot select core/spanner candidates or stand in for the complete maintained population. |
| Permanent references | `dynamic_min_ratio` and the min-cost cycle paths enumerate candidates; the P9.5 source-flow audit rejects `dynamic_min_ratio`, `min_cost::oracle`, and `min_cost::experiment`. | They may remain test Oracles but cannot become the production selector. |

The missing P9.5a construction must do all of the following using the completed
exact input projection and supplied-candidate heap:

1. Maintain finite core/spanner and terminal declarations across supported
   snapshots through stable `candidate::Registry` replacement/retirement.
2. Merge those declarations into one live source population with exact
   provenance and stable identities.
3. Extend the completed terminal-only coordinate check and `Step` bridge to
   the complete maintained population, certifying the selected compact
   direction with the current approximation gradients, lengths, and `kappa`
   required by the Lemma 4.4 transition.
4. Keep the stability-witness input out of the query result, retain exact
   arithmetic, and reject rather than fall back when the source construction is
   unavailable.

This audit did not expose `StableWitness`, add a heuristic selector, enumerate
fundamental cycles, or import `dynamic_min_ratio`. Those routes would only make
the existing test path look complete while leaving the P9.5 production semantic
contract unimplemented. P9.5 and P9.5a therefore remain `in_progress`; the
remaining cross-snapshot and combined-population work blocks the complete
backend, and P9.6 remains gated on its completion. P9.3.2d remains the
separate, low-priority P9.6a proof debt after the complete flow-solver chain is
available.

## Incremental audit

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo fmt --all -- --check` | 0 | Rust formatting accepted |
| `python3 tools/check_biclique_bound.py` | 0 | compact-biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | source minimum-ratio no-fallback boundary accepted |
| `python3 tools/check_source_flow_audit.py` | 0 | no production recovery or reference-flow fallback dependency |
| `cargo test -p graph source_flow` | 0 | 8 focused tests passed, including augmented and lower-bound recovery |
| `cargo test -p graph` | 0 | 164 graph tests passed, including feasibility-versus-optimality regression |
| `cargo test -p dominance` | 0 | 38 dominance tests passed and 2 existing tests ignored; includes compressed source-flow, MRD chord, and formal completion differentials |
| `cargo clippy -p graph --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo clippy -p dominance --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no workspace warnings |
| `cargo test --workspace` | 0 | workspace suite passed |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | documentation built without warnings |
| `cargo build --workspace --release` | 0 | six crates built in release mode |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

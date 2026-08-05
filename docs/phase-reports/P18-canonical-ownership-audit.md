# P18 Canonical Ownership and Clone Audit

Status: the first section records the pre-refactor finding; the disposition
section records what P18 implemented. The audit is not itself a performance
claim.

This audit follows every operation measured as
`canonical_component_clone_ns` in the accepted P17 kernel harness. It is an
ownership audit, not a performance claim: the copied-byte quantities below are
structural estimates derived from Rust layouts, not allocator or RSS samples.

## Boundary of the audited operation

P17 Scope A calls `measure_scope_a` once per algorithm and iteration. The
operation is:

1. clone the canonical `GridComponent<bool>`;
2. pass a reference to the clone to the selected production algorithm;
3. build geometry, representation, flow/matching state, completion output, and
   validation state inside that solve; and
4. drop the clone and all solve-local state at return.

The production solver functions do not mutate the component today. The clone
was therefore an ownership convenience in the benchmark boundary, not a
semantic requirement of compact MRD, explicit Hopcroft--Karp, or explicit C0
flow. This is important scope: P18 removes a benchmark-boundary copy; ordinary
production callers already pass the component by reference and should not be
described as receiving a new asymptotic solver.

## Implemented disposition

P18 centralizes the ownership choice in one `Cow<GridComponent<bool>>`
acquisition function: the reference backend produces `Cow::Owned`, whereas the
optimized backend produces `Cow::Borrowed`. Both correctness gates and measured
Scope A iterations use that same explicit boundary. A fresh algorithm-local
`SolverWorkspace` owns only selection buffers; it is not shared between
algorithms or measured iterations, and Scope B does not create it.

## Concrete cloned type and fields

The cloned value is `mrd_domain::GridComponent<bool>`:

| Field | Mutated during solve | Copy/allocation behavior | Ownership classification |
| --- | --- | --- | --- |
| `id: ComponentId` | no | inline copy | immutable canonical input |
| `color: bool` | no | inline copy | immutable canonical input |
| `grid_width`, `grid_height: usize` | no | inline copy | immutable canonical input |
| `cells: Vec<Cell>` | no | allocates a new buffer and copies `N` `Cell` values | immutable canonical input |

`Cell` contains two `usize` coordinates. The deep-copy payload is therefore
`N * size_of::<Cell>()` bytes, plus vector capacity and allocator bookkeeping.
The exact allocator overhead is intentionally not claimed. No nested
`HashMap`, `HashSet`, `BTreeMap`, `String`, or coordinate object is reachable
from this clone; those allocations occur later in the prepared geometry or
solver workspace.

## Immutable prepared context

`PreparedComponentContext<'a, C>` borrows the canonical component and owns the
following immutable values for one solve:

- `PreparedGridComponent`: local occupancy, prefix sums, and horizontal and
  vertical interior runs;
- normalized `Boundary`, `BoundaryIndex`, and deterministic reflex groupings;
- boundary build metrics and backend provenance strings; and
- timing counters for preparation and indexing.

The context is consumed by `sg_oracle::grid::analyze_prepared_geometry`, which
transfers the prepared boundary, endpoint index, occupancy, and chord vectors
into an immutable `Geometry`. These values are derived from the canonical
component and are not solver state. A future shared-prepared backend can
borrow this context/geometry across iterations, but the first P18 comparison
keeps geometry construction inside both Scope A paths so that the clone phase
is isolated.

## Algorithm-specific mutable workspace

Each measured algorithm creates fresh local state. The mutable values are:

- embedding coordinate vectors;
- explicit conflict graph or compact biclique partition;
- explicit or compressed flow network and residual capacities;
- matching arrays, vertex-cover arrays, and selected chord buffers; and
- completion queues, temporary cut materialization, rectangle recovery, and
  validation scratch state.

None of these values may be shared between compact MRD, explicit
Hopcroft--Karp, and explicit C0 flow, or between measured iterations. The P18
interface makes this boundary explicit with a per-solve `SolverWorkspace` and
records its preparation separately from canonical borrowing/cloning. The
workspace owns only horizontal and vertical selection buffers, reserves their
exact respective chord counts, and is absent from Scope B because that scope
does not materialize selected cuts. Representation, matching, flow, completion,
and validation allocations remain algorithm-local values rather than hidden
workspace state.

## Owned output and temporary validation state

`DissectionResult` owns the final rectangle vector and certificate/diagnostic
data. Witness checksums are computed from a canonical sorted copy of the
rectangle list and are not borrowed after the solve. Completion and final
validation may allocate temporary queues and coverage maps; their timing and
structural estimates remain separate from the solver workspace.

## Cost model and decision

The old clone cost is proportional to `N`, the number of foreground cells, and
not to `B`, `U`, `q`, `K`, or `M`. It is not part of the MRD mathematical
reduction. The selected design is therefore:

1. keep `clone-canonical-reference` as an internal reference backend;
2. add `borrowed-canonical`, which passes the immutable canonical component by
   ordinary Rust borrowing and performs no deep clone;
3. leave prepared-context reuse unrepresented until it has distinct executable
   semantics and its own evidence campaign; and
4. allocate a fresh algorithm-local `SolverWorkspace` for every Scope A solve.

Ordinary borrowing is sufficient. No `Arc`, interior mutability, or `unsafe`
aliasing is necessary or permitted by this phase. The benchmark therefore
reports clone time, canonical borrow/share time, and workspace preparation as
distinct fields rather than relabeling residual setup cost as zero clone cost.
Allocation diagnostics count only the audited ownership-layer `Vec` buffers:
one canonical cell buffer on the nonempty clone path and up to two nonempty
selection buffers. Representation bytes are separately derived from canonical
graph/network structural counters. None of these estimates is an allocator or
RSS measurement.

## Invariants to preserve

The optimized path must preserve the canonical component, prepared geometry,
chord order, conflict identities, structural counters (`q`, `H`, `V`, `K`,
`M`), optimum count, canonical witness checksum, and output validity. Running
the three algorithms in any counterbalanced order must produce the same values
as the clone reference and must not affect later timings except for normal
measurement noise.

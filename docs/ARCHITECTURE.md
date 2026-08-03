# Workspace Architecture

The workspace is organized by responsibility rather than by a repeated `rect`
prefix:

- `mrd-domain`: immutable geometry, normalized input models, certificates, and
  validation contracts;
- `graph`: reusable exact graph primitives and source-shaped graph algorithms;
- `exact-cover-oracle`: the deliberately slow definition-level cover Oracle;
- `sg-oracle`: Soltan--Gorpinevich grid and polygon Oracles plus explicitly
  named experimental backends;
- `dominance`: the experimental MRD algorithms and their zero-cost static
  backend selection;
- `verification`: differential campaigns, adversarial generators, benchmarks,
  and reports;
- `mrd`: the process boundary for CLI parsing, filesystem IO, and dispatch.

## Namespace Rules

Oracle and experimental implementations live in distinct `oracle` and
`experiment` modules. Shared parent modules contain only stable domain types,
traits, certificates, and pure orchestration. Names omit information already
carried by their namespace: for example, use `dominance::experiment::Mode`,
`sg_oracle::grid::oracle::Pairwise`, and
`sg_oracle::grid::experiment::InteriorRuns`.

The crate roots do not re-export removed paths. This is intentionally a
breaking architecture: new code must name the ownership boundary it depends
on.

## Functional Boundaries

Domain transformations accept explicit inputs and return values or structured
errors. Mutation is local to graph, sweep, queue, and arena implementations;
it is not exposed as cross-module shared state. Filesystem IO, process exit,
clock reads, and command dispatch stay in `mrd::application`. Verification and
benchmark code depend on production and Oracle APIs, while algorithm crates do
not depend on verification campaigns.

Backend choice uses enums, generics, and monomorphized traits. The architecture
does not introduce trait objects, runtime registries, or compatibility
adapters, so the namespace boundaries add no runtime allocation or dispatch.

## Layered solver backend

`mrd::layered` exposes an explicit three-layer backend model:

- `SolverMode` (`Reference`, `SourceWithTarget { target, source_config }`)
  selects which backend solves an instance. There is deliberately no
  `AutomaticSource` variant.
- `SolverProvenance` (`ReferenceExact`, `SourceCertifiedAtMost { target }`) is
  recorded on every `LayeredResult`.
- `solve_reference` wraps the permanent reference backends; `solve_source_with_target`
  wraps the source-shaped execution path under a caller-supplied inclusive
  target; `verify_source_infeasible_below`, `verify_cover_below`, and
  `verify_source_feasible_at_most` verify caller-supplied certificates exactly.

The source layer never calls a reference solver, never infers `F*`, and never
classifies an ordinary execution failure as target infeasibility. Automatic
target search remains blocked (P9.5e.3g.3); `Backend::require_complete()`
stays `Error::Incomplete`.

The reference layer is the complete production-ready solver surface for its
supported formal-polygon domain. The source layer is research-only and
target-bound: its exact outputs are useful for experiments and certificates,
but it is neither an automatic solver nor an AN19 runtime implementation.

`mrd::layered::experiment` is the public, serializable measurement boundary.
Its records explicitly separate category, input representation, target
provider, outcome, and stage timings. The private `layered::execution` module
owns source-run timing collection; it does not widen the public solver API.
P10 rows are polygon-derived only. An explicit direct-grid `unavailable` record
prevents the measurement layer from implying a P11 implementation, while a
reference-provided target remains a separately labelled experiment rather than
automatic `F*` search.

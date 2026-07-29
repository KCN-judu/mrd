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

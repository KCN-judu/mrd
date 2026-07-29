# P09 - Integration Gate Audit

## Decision

P9 cannot introduce an `AlmostLinear` backend at this state. The P7/P8 code is
an exact baseline and audit layer, not an implementation of the theorem cited
by `docs/NEAR_LINEAR_FLOW_IMPLEMENTATION.md`. Naming it otherwise would violate
the source gate in the master plan.

## Required gate status

| Required contract | Current evidence | Status |
| --- | --- | --- |
| Exact rational fractional flow and IPM potential | P9 has `FractionalCirculation` plus `RationalInteriorPointState`; it verifies an exact rational reciprocal-slack surrogate, not the source's fixed-point log/fractional-power potential | partial |
| KP15 deterministic fractional-to-integral rounding | `round_fractional_costed` implements deterministic cycle cancellation with an exact rational differential, but uses a BFS Oracle rather than the source's link-cut-tree bound | partial |
| Theorem 4.6 bounded-domain/IPM accounting | bounded integral inputs, strict rational slacks, observed surrogate-potential decrease, and exact coordinate-update totals are checked; the cited source version labels the relevant IPM result Theorem 4.3 and its log Taylor fact Lemma 4.6 | partial |
| Theorem 5.1 approximate dynamic query | P9 now exactly enumerates signed simple cycles over the current checked ledger and records query/candidate work; it is a superlinear Oracle, not the theorem's dynamic approximate structure | partial |
| Source-grade low-stretch/spanner construction | AN19 hierarchy mechanics, certificates, and workspace scan counting are implemented and empirically audited; the formal SIAM text does not supply the reduced-event ordering/counting conversion, so the runtime claim remains deferred proof debt | partial |
| Exact recovery and MRD compressed-flow parity | P9 has an exact parity audit over partition validation, Hopcroft--Karp, compressed Dinic, and compressed Push--Relabel. The audit exposed and closed a trapped-preflow termination regression while preserving highest-label, global-relabel, and gap behavior; no selected `AlmostLinear` backend exists | partial |
| No fallback | satisfied only because no P9 backend exists | not sufficient |

## Next implementation order

1. Add exact rational arithmetic, fractional feasible-flow representation, and
   KP15 rounding Oracle with a rational differential.
2. Implement a checked bounded-domain IPM state and potential-decrease
   evidence, retaining P7 as the exact recovery Oracle.
3. Replace the P8 replay-only query with a source-matching dynamic query and
   counters, without fallback.
4. Integrate only after the preceding gates pass; compare every MRD compressed
   cut, cover, chord family, and rectangle result against existing exact
   backends.

This table records the original integration-gate baseline. Subsequent P9.1 and
P9.2 implementation work closed several rows. The formal SIAM journal version,
DOI `10.1137/17M1115575`, was obtained and checked; it does not establish the
required reduced-event conversion. P9.3.2d is therefore faithful implementation
with deferred low-priority proof debt: P9.3.3 through P9.5 may continue, while
the eventual `AlmostLinear` claim remains gated on the proof.

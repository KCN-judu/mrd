# P06 - Near-Linear Flow Specification

## Result

P6 is a source-specification phase, not an implementation claim.
`docs/NEAR_LINEAR_FLOW_IMPLEMENTATION.md` maps the primary deterministic
minimum-cost-flow result and every required predecessor contract to intended
Rust modules.

Pinned sources are van den Brand et al. arXiv:2309.16629v1; Chen et al.
arXiv:2203.00671v2; Kang--Payor arXiv:1507.08139v1;
Chuzhoy--Saranurak arXiv:2009.08479v1; and Sleator--Tarjan ST83,
DOI 10.1145/800076.802464.

## Guardrails

- `FlowNetwork` is not represented as an almost-linear min-cost-flow engine.
- Exact rational circulation, hidden-stability certificates, bounded-domain
  gates, deterministic rounding, and undirected decremental-spanner scope are
  explicit prerequisites.
- P7 is limited to a superlinear exact Oracle. P8 must split source-backed
  dynamic structures before implementation.

## Audit

All mandatory commands exited 0: diff check, formatting, biclique-bound check,
workspace clippy, workspace tests, warning-free rustdoc, release build, and
release consistency. The consistency checker reports 30 reachable manifest
commits. This phase changes source documentation only; no fallback, ignored
test, or theorem claim is added.

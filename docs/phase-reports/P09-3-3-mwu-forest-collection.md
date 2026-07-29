# P09.3.3 - Deterministic MWU Forest Collection

## Scope

P9.3.3 implements the construction part of Lemma 5.5 of
arXiv:2309.16629v1. It produces exactly `k` rooted source-shaped low-stretch
forests and an exact, self-verifying per-edge average-stretch certificate. The
permanent P8 weighted-Kruskal collection remains available as an independent
small-instance Oracle and is not called by this implementation.

## Construction

`graph::source_lsf::experiment::mwu::Collection` accepts a positive exact
envelope `W`, a positive reduction factor, and a positive tree count `k`. In
each round it:

1. builds the exact weighted-copy graph `G_v` from the current rational MWU
   weights;
2. constructs an AN19-shaped static tree on the unit-weight copy graph;
3. maps every selected copy edge back to its unique original edge, rejecting a
   duplicate or non-tree image;
4. initializes the P9.3.2 Spielman--Teng/branch-free rooted forest and exact
   stretch overestimates on the original weighted graph; and
5. checks the supplied finite-instance Lemma 5.4 envelope
   `sum_e v_e str_e <= W sum_e v_e` and
   `max_e str_e <= k W ceil(log2 n)^2`.

It rejects inactive snapshots, invalid roots, disconnected source
constructions, a failed weighted-copy/tree/forest certificate, an envelope
violation, certificate mutation, and all exact arithmetic overflow. There is
no Oracle fallback.

## Rational MWU Certificate

Appendix A.2 writes an exponential update. The source graph uses exact
rational weights, so the implementation uses
`v_(i+1,e) = v_(i,e) (1 + x + x^2)` with `x = str_(i,e) / rho` and
`rho = 10 k W ceil(log2 n)^2`. The checked maximum-stretch envelope gives
`0 <= x <= 1/10`. On that interval,
`1 + x <= 1 + x + x^2 <= 1 + 2x` and
`ln(1 + x) >= 19x/20`.

Consequently the checked potential has the same form as the source MWU proof:
the total weight grows by at most `1 + 2W/rho` per round, while every edge's
final weight lower-bounds its accumulated stretch. With
`L_m = max(1, ceil(log2 m))`, the certificate verifies the exact conservative
bound

```text
average_e <= (20/19) (10 W ceil(log2 n)^2 L_m + 2 W).
```

This proves the recorded uniform bound for the supplied `W`. It does not prove
the paper's asymptotic `O(log^7 n)` guarantee: that requires a separately
closed, uniform source Lemma 5.4 `W = O(log^4 n)` proof and the source-model
word-bound audit. P9.3.2d's deferred AN19 event-order/runtime proof remains
unverified and continues to prohibit an `AlmostLinear` claim.

## Focused Evidence

- Three P9.3.3 tests cover deterministic repeated construction of exactly
  three forests; original-tree/forest/root/piece structure; every per-edge
  average bound; envelope rejection; inactive-snapshot rejection; and
  certificate-mutation rejection.
- Ten `source_lsf` tests cover the weighted-copy transform, static exact tree
  Oracle, forest initialization, branch-free closure, exact stretch vectors,
  and decremental update mechanics.
- Two P8 `lsf_mwu` tests retain the independent weighted-Kruskal baseline.

## Audit

Phase baseline: `c2edf16d440cb34a2d58035f7958e7fa05981c75`.

Implementation SHA: `038f762e4a45c0f0bcd589b2258e2d879e4d3d68`.

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatted |
| `python3 tools/check_biclique_bound.py` | 0 | bound checker passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test -p graph source_lsf -- --nocapture` | 0 | 10 passed |
| `cargo test -p graph lsf_mwu -- --nocapture` | 0 | 2 passed |
| `cargo test --workspace --quiet` | 0 | full workspace passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | warning-free documentation |
| `cargo build --workspace --release` | 0 | optimized build passed |
| `python3 tools/check_release_consistency.py` | 0 | release metadata and baseline evidence passed |

The audit found no fallback use, ignored P9.3.3 tests, stale generated evidence,
credentials, tokens, private keys, or absolute local paths in the staged
implementation diff.

## Next Action

P9.3.4 may begin the deterministic static spanner-with-embedding primitive.
It must retain this collection as an input contract and preserve the explicit
AN19 runtime-proof debt boundary.

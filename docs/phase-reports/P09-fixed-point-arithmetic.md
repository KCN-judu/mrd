# P09.2.1 Certified Fixed-Point Arithmetic

## Scope and issue matrix

| Field | Evidence |
| --- | --- |
| Classification | implementation gap and numerical-certification gap |
| Observed | `RationalInteriorPointState` evaluates a reciprocal-slack surrogate. It has no representation or certified evaluator for the logarithm and fractional powers in CKLPPS22 Equation (9). |
| Expected | The computational model in CKLPPS22 Section 3 and the deterministic paper Section 2 requires fixed-point arithmetic with `O(log^O(1) z)`-bit words. Equation (9), Definition 4.2, Lemma 4.5, and Lemma 4.6 require certified `log(x)` and `x^-alpha` values on a strictly positive bounded domain. |
| Cause | Exact rational arithmetic is closed under the existing flow operations but not under nonzero logarithms or general nonintegral powers. Treating the rational surrogate as Equation (9), or using an unchecked binary floating result, would silently change the theorem contract. |
| Change | Add arbitrary-size integer-backed dyadic intervals with an explicit polylogarithmic word gate, outward-rounded arithmetic, and certified series enclosures for `log`, `exp`, and `x^-alpha`. |
| Verification | External `ln(2)` decimal bounds, `exp(log(x))`, reciprocal, and fractional-power identities; domain and word-budget rejection; focused strict Clippy; full repository gate. |
| Acceptance | Verified for the P9.2.1 arithmetic primitive. Equation (9), gradients/lengths, IPM steps, initialization, and recovery remain assigned to P9.2.2--P9.2.4. |

## Semantic basis and decision

The sources are CKLPPS22, arXiv:2203.00671v2, Section 3, Equation (9),
Definition 4.2, Lemmas 4.5--4.6, and Theorem 4.3; and the deterministic primary
source, arXiv:2309.16629v1, computational model and Theorem 4.6. The latter
encapsulates the former IPM and its dynamic min-ratio interface.

Three alternatives were evaluated:

- `f64` or `f32` was rejected because neither gives the required directed
  error enclosure or a checked word/model guarantee.
- The existing exact reciprocal-slack surrogate remains useful as an Oracle,
  but was rejected as a replacement for Equation (9).
- An unchecked arbitrary-precision transcendental library was rejected as the
  production proof boundary because it would not itself establish fixed-point
  rounding or the paper's bounded-word accounting.

The selected representation stores both interval endpoints as integers scaled
by `2^p`. Every division and fixed-point multiplication rounds the lower
endpoint down and the upper endpoint up. The configured word bound is

```text
ceil(log2(z + 1))^q bits,
```

where `z` is the recorded input encoding size and `q` is an explicit constant.
Every endpoint and multiplication intermediate is measured and rejected when
it exceeds that bound.

For logarithms, power-of-two range reduction is followed by

```text
log(y) = 2 * sum[j >= 0] z^(2j+1)/(2j+1),  z = (y-1)/(y+1),
```

with the omitted tail bounded by

```text
2 * |z|^(2N+1) / ((2N+1) * (1-|z|^2)).
```

For exponentials, repeated halving gives `|y| <= 1/8`; the Taylor remainder is
bounded from its first omitted term by a geometric majorant before repeated
interval squaring. `x^-alpha` is composed as `exp(-alpha * log(x))`. No binary
floating type or unbounded-error transcendental result occurs in the module.

## Evidence

- A 96-fractional-bit, 48-term configuration encloses an independent decimal
  interval for `ln(2)` and has width below `10^-15`.
- `exp(log(x))` encloses the exact input for `x` in
  `{1/8, 1/2, 1, 2, 8}`; `x^-1` encloses each exact reciprocal.
- `4^(-1/2)` encloses exactly `1/2` with width below `10^-12`.
- Nonpositive logarithm/power domains are rejected, and a deliberately small
  polylogarithmic word budget rejects an oversized scaled integer.
- Metrics expose arithmetic operations, directed rounds, logarithm terms,
  exponential terms, and the maximum observed word length.

## Audit

Phase baseline: `d1c7a3b8f3570f67f32c089c9054f87ef19fe02a`.

| Command | Exit | Duration/result |
| --- | ---: | --- |
| `git status --short` | 0 | only the P9.2.1 implementation and dependency changes |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | 0.23s |
| `python3 tools/check_biclique_bound.py` | 0 | 0.04s |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 5.23s; no warnings |
| `cargo test --workspace` | 0 | 191 passed, 3 ignored across 13 suites; 405.56s summed reported suite time |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | 3.40s |
| `cargo build --workspace --release` | 0 | 14.30s |
| `python3 tools/check_release_consistency.py` | 0 | 1.89s; baseline release and 30 reachable manifest commits verified |

Final inspection found no ignored-test changes, fallback path, stale generated
evidence, credential, token, private key, local absolute path, `f32`, or `f64`.
The new crates are pinned by `Cargo.lock`; reference flow backends are unchanged.

## Remaining gate

P9.2.1 supplies only the certified numerical substrate. It does not yet
evaluate Equation (9), generate Definition 4.2 gradients or lengths, certify
Theorem 4.3 approximation hypotheses, apply Lemma 4.4 updates, construct the
initial state, or prove additive-half termination and exact recovery. P9 stays
`in_progress`, and no `AlmostLinear` backend is present.

# P09.2.2 Certified IPM Quantities

## Scope and issue matrix

| Field | Evidence |
| --- | --- |
| Classification | source-semantic implementation gap |
| Observed | P9.2.1 could certify elementary transcendental values but did not connect them to a flow objective, Equation (9), lengths, gradients, or Theorem 4.3 approximation hypotheses. |
| Expected | CKLPPS22 Equation (9), Definition 4.2, and Theorem 4.3 require the exact objective gap `c^T f - F*`, `alpha = 1/(1000 log(mU))`, per-edge lengths, gradients, and certified factor-two/scaled-gradient approximation checks. |
| Change | `CertifiedIpmSnapshot` evaluates the source quantities on a strictly interior fractional circulation using P9.2.1 intervals. `certify_approximations` proves `ell/2 <= ell_tilde <= 2ell` and `|(g_tilde-g)/ell| <= kappa/8` for every edge. |
| Verification | Exact unit-slack identities, independent high-precision nonunit-slack bounds, strict-boundary rejection, bad length/gradient rejection, configuration mismatch rejection, focused and full workspace gates. |
| Acceptance | Verified for the Equation (9)/Definition 4.2 quantity layer. Lemma 4.4 updates, initialization, additive-half termination, and recovery remain P9.2.3--P9.2.4. |

## Semantic basis

The primary deterministic source is Jan van den Brand et al.,
arXiv:2309.16629v1, Theorem 4.6 and its Section 4 interface. It explicitly
delegates the potential-reduction method to CKLPPS22 arXiv:2203.00671v2,
Theorem 4.3. The implementation follows CKLPPS22 Equation (9), Definition 4.2,
and the approximation condition stated in Theorem 4.3; it does not relabel the
delegated theorem as Theorem 4.6.

For a current flow `f`, the implementation certifies

```text
Phi(f) = 20m log(c^T f - F*)
         + sum_e ((u+_e - f_e)^(-alpha) + (f_e - u-_e)^(-alpha))
ell_e   = (u+_e - f_e)^(-1-alpha) + (f_e - u-_e)^(-1-alpha)
g_e     = 20m c_e/(c^T f - F*)
         + alpha (u+_e - f_e)^(-1-alpha)
         - alpha (f_e - u-_e)^(-1-alpha).
```

The current `CirculationNetwork` is the normalized zero-lower-bound form
(`u-_e = 0`); lower-bound shifting into demands remains an explicit later
integration task. `F*` must be supplied as an exact integral objective, as in
the source's binary-search boundary. The evaluator rejects nonintegral `F*`,
`mU <= 1`, nonpositive objective gap, non-feasible flow, and every boundary
slack. The approximation checker requires the identical `FixedPointConfig` that
created the snapshot, so a different precision or word budget cannot silently
reuse an interval certificate.

## Evidence

- A two-edge unit-slack instance has exact objective gap `1`, potential `4`,
  lengths `2`, and gradients `40` and `0`; all are enclosed exactly.
- A capacity-three instance with flow one has slack pair `(1,2)`. Independent
  60-digit decimal bounds enclose `alpha = 0.0004808983469629878...`,
  `Phi = 3.999333444432099794...`, length
  `1.499833361108024948...`, gradient
  `39.999759470690150815...`, and gradient
  `-0.000240529309849184...`.
- Exact approximations pass the factor-two and `kappa/8` certificate; length
  `5` and gradient `41` are rejected on edge zero with structured errors.
- A boundary flow, mismatched arithmetic configuration, invalid `F*`, and
  nonpositive source domain are rejected before any unsupported claim.
- Snapshot metrics are per-evaluation fixed-point counts, not accumulated
  counts from a reused arithmetic object.

## Audit

Phase baseline: `44d50d46f25b2a493630481916be3589b6f3188e`.

| Command | Exit | Duration/result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | 0.20s |
| `python3 tools/check_biclique_bound.py` | 0 | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 4.06s; no warnings |
| `cargo test --workspace` | 0 | 194 passed, 3 ignored across 13 suites; 391.32s summed reported suite time |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | 2.94s |
| `cargo build --workspace --release` | 0 | 12.69s |
| `python3 tools/check_release_consistency.py` | 0 | 1.85s; release consistency and 30 reachable manifest commits verified |

Final staged-diff inspection found no ignored tests, fallback path, stale
generated evidence, credentials, tokens, private keys, or local absolute paths.
No `AlmostLinear` backend or complexity claim was introduced.

## Remaining gate

This subphase does not choose a min-ratio cycle, update the flow, prove the
Lemma 4.4 potential decrease, construct an initial point, or perform KP15
recovery. P9 remains `in_progress`; P9.2.3 must implement those transitions
using the certified quantities and retain P7 as the exact recovery Oracle.

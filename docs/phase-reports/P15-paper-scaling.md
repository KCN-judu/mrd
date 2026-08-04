# P15 Paper-Scaling Full Campaign Audit

**Status:** passed for the predeclared finite campaign.

## Provenance

- Source commit: `252d01f08c6ba64b17b8fe22ce7317d7c2d58c76`
- Release binary SHA-256:
  `e58e1b898a97dfba9334439b9a8b5b86aa10a7b554e312fdfe2ace98929c9129`
- Configuration SHA-256:
  `6245b382bccc7cfddb32806ad5dff20a1d4019a6991fdfde57254226db632fa3`
- Host: Apple M4, arm64, 10 logical CPUs, macOS 26.5, Rust 1.89.0.
- Campaign runner wall duration: 1,255.110297167 seconds.
- Full child-process wall sum: 312.813407982 seconds.

## Campaign Completion

The immutable plan contains 5,824 fresh-process sample identities: 672 warm-up
and 5,152 measured processes. The checkpoint and raw artifact both report all
5,824 identities terminal, with no duplicate or missing identity. Terminal
state counts are 4,522 `success`, 1,302 `unsupported`, zero `timeout`, and
zero `error`. `paired_validation_errors` is empty.

The unsupported rows are bounded exact-cover Oracle requests beyond its
declared cell limit. They remain in the raw evidence and are neither discarded
nor counted as compact wins. The three production paths agree on all 1,288
measured paired instance groups.

The 1,008-row pilot's child-process walls sum to 45.966973783 seconds; its
linear projection for the full plan was 265.586959635 seconds. The complete
run's child-wall sum is 312.813407982 seconds. The difference between that sum
and runner elapsed is a collection-protocol residual, including launch,
validation, and per-record atomic whole-checkpoint persistence. It is not
allocated to, or interpreted as, solver time.

## Generated Evidence

- Raw checkpoint, JSON, and CSV: `results/paper-scaling-full-*`.
- Summary JSON/CSV and report:
  `results/paper-scaling-full-summary.*` and
  `results/paper-scaling-full-report.md`.
- Paper tables: `results/paper-scaling-full-tables.tex`.
- XML-parseable figures: `results/paper-scaling-full-figures/`.

The manifest binds all eleven full-campaign artifacts to their SHA-256 values.
Its schema-equivalent standard-library validation, artifact-hash verification,
CSV row count, SVG parsing, and LaTeX table-pair checks passed. A third-party
`jsonschema` package was not installed; its absence is recorded rather than
misreported as a successful external validation.

## Quality Gates

All commands below exited successfully on the final evidence branch:

```text
git diff --check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps
cargo build --workspace --release
python3 tools/check_biclique_bound.py
python3 tools/check_source_flow_audit.py
python3 tools/check_source_lsst_audit.py
python3 tools/check_source_min_ratio_audit.py
python3 tools/check_release_consistency.py
python3 tools/test_paper_scaling.py
python3 tools/run_paper_scaling.py --self-test
python3 tools/analyze_paper_scaling.py --self-test
```

The final release binary hash still equals the raw campaign's recorded binary
hash. No absolute local paths, credentials, or tracked documentary files were
introduced by the P15 artifacts.

## Claim Boundary

The analysis supplies local process-wall ratios, structural `K`--`M`
comparisons, phase medians, and empirical log--log fits over the predeclared
measured interval. It does not prove an asymptotic runtime, an AN19 runtime,
or an automatic source-flow solver. The only reported fixed-rule crossover is
`representation-crossover` at target size 60; it is not a general backend
policy.

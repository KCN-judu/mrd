# P00 Persistent Plan and Baseline Report

- Phase: P0
- Branch: `codex/full-implementation`
- Start SHA: `72ce32a6fbde3c2d285ca7b8c9a21dc17e0dea64`
- Baseline `origin/main`: `72ce32a6fbde3c2d285ca7b8c9a21dc17e0dea64`
- Workspace version: `1.3.0`
- Release tag: `v1.3.0-output-sensitive-sparse-geometry`
- Release commit: `533e37a867610ff40e8f0fe936091154d3f6ef8b`
- Started: `2026-07-27T09:17:42Z`
- Audit completed: `2026-07-27T09:35:54Z`
- Closeout completed: `2026-07-27T09:40:49Z`
- Closeout commit and verified remote SHA: `ae763cacdb95e71130977c6cbaa621502b9ceafc`
- Algorithmic changes: none

## Repository and branch verification

The initial worktree was clean on `main`. After `git fetch origin`, local
`main`, `HEAD`, and `origin/main` agreed at the full baseline SHA. Neither a
local nor remote `codex/full-implementation` branch existed, so it was created
from the fetched `origin/main` without rewriting history. The release tag and
workspace version matched the expected public baseline.

The implementation inspection confirmed the baseline/gaps recorded in the
master plan. In particular, ranked coordinate maps, recursive biclique
sorting, and Dinic are current production behavior; formal-boundary cases and
deterministic almost-linear flow are not implemented or claimed.

## Mandatory audit

| Command | Exit | Duration | Result |
| --- | ---: | ---: | --- |
| `git status --short` | 0 | <0.1s | only the new plan was untracked |
| `git diff --check` | 0 | <0.1s | clean |
| `cargo fmt --all -- --check` | 0 | 0.33s | clean |
| `python3 tools/check_biclique_bound.py` | 0 | <0.01s | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 0.16s | no issues |
| `cargo test --workspace` | 0 | at least 642.76s in reported extended test-binary time | 122 passed, 0 failed, 3 ignored |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | 1.42s | seven workspace crates documented |
| `cargo build --workspace --release` | 0 | 0.03s | passed; cached release build |
| `python3 tools/check_release_consistency.py` | 0 | 1.88s | version/tag/commit consistent; 27 manifest commits reachable |

The PTY did not emit one aggregate wall duration for `cargo test`; the exact
long-running binary summaries were 290.86s for the 35-test SG suite and
351.90s for the 38-test verification suite, with the remaining binaries also
passing. The three ignored tests are explicitly named release campaigns:

- full 4x4 compact boundary differential;
- exhaustive 4x4 polygon differential;
- extended v0.9 grid/polygon differential.

No fallback or generated result was exercised by this documentation-only P0.
No generated evidence was changed. Inspection found no credential, token,
private key, or newly introduced local absolute path. The committed historical
manifest contains reproduction commands with local paths; P0 does not modify
that existing evidence.

## Result and limitations

- Result files: this report and `docs/IMPLEMENTATION_MASTER_PLAN.md`.
- Correctness disagreements: none.
- Remote divergence: none before closeout; the pushed closeout SHA was verified
  with the remote branch ref.
- Remaining limitations: all baseline gaps listed in the master plan remain;
  P0 deliberately implements no algorithmic changes.

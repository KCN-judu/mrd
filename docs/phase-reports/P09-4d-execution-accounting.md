# P09.4d - Checked Execution Accounting

`source_min_ratio::execution::Executor` forwards only the already checked
ledger's exact `Update`, `Query`, and `Detect` operations. Counters advance
only after success. Dynamic sparsification and link-cut requests are explicit
rejections with their own counter; no request falls back to an Oracle.

The focused source-min-ratio suite has 9 passing tests. Full workspace format,
tests, Clippy, rustdoc, release build, biclique bound, no-fallback audit, and
release consistency passed.

Counters are finite observed accounting only. This does not implement general
dynamic sparsification or link-cut maintenance and claims no approximation,
amortized, priority-queue, Theorem 5.1, or AN19 runtime bound.

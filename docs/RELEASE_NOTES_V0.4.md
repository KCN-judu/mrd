# v0.4.0 - Indexed Geometric Completion

Release tag: `v0.4.0-indexed-completion`

CompactOnly now defaults to `IndexedFrontierCompletion`. The backend preserves
the reference horizontal-then-vertical candidate order and exact stopping
semantics while replacing repeated global rescans with one component-local
frontier initialization per axis and local generation-based refreshes.

The release exposes `--completion-backend reference-rescan|indexed-frontier`,
keeps SG and FullyAudited reference defaults, records selected and added unit
cuts in `CompletionResult`, and reports five completion phase timings plus
candidate, scan, stale, ray, and recovery counters.

Exact differential evidence covers every nonempty binary 3x3 and 4x4 grid,
87,148 polyomino/hole fixtures, 25 adversarial fixtures, and 100,000 seeded
connected regions. Both outputs pass the cell-exact validator. No disagreement
or minimized completion regression was produced.

The final dense q=512 through q=4096 campaign measured geometric-completion
speedups from 1.541x to 4.183x. This is a measured practical improvement, not a new
asymptotic claim. Process peak RSS remains unavailable; owned storage estimates
are retained.

Remaining paper gaps are the general polygon O(n log n) SG enumerator, formal
degenerate holes, optimized Theorem 8 constants, and an almost-linear exact
flow backend.

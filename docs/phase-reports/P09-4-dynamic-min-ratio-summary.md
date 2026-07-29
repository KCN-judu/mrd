# P09.4 - Dynamic Min-Ratio Semantic Summary

P9.4a-d provide a finite source-shaped semantic chain: immutable shifted tree
branches; direct compact-cycle decoding; a public boundary over an already
checked hidden-stability ledger; and explicit `Update`/`Query`/`Detect`
accounting with unsupported-operation rejection. Production code has no
enumerating-cycle fallback.

This is not a completion of Theorem 5.1. It does not supply an approximate
cycle search, general dynamic sparsification, link-cut maintenance, or an
amortized bound. P9.3.2d proof debt remains unchanged: the backend cannot be
called `AlmostLinear` and must report `an19_runtime_verified: false`.

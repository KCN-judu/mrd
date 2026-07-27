# Polygon Recovery Crossover

The v1.3 release campaign compares four exact configurations on the same final
cut families:

- dense coordinate arrangement and dense validator;
- reference range-scan subdivision and reference slab validator;
- orthogonal-sweep subdivision and event-tree validator;
- the optional `auto` recovery policy.

The committed source data is
`results/v1.3-output-sensitive-scaling.csv` with the corresponding JSON report.
It contains 56 verified rows: fourteen boundary-native families at sizes 16,
32, 64, and 128. There were no solver errors, rectangle disagreements,
subdivision candidate traversals in the sweep backend, or boundary/resort scans
in the event validator.

Sparse retained estimates first became smaller between sizes 32 and 128 for
ten families; four tested families had no memory crossover through 128. Sparse
recovery first became faster at size 128 for six families and did not cross for
the other eight. Thus no single crossover size is claimed.

`auto` uses only coordinate counts, the dense cell-byte formula, boundary
complexity, and final segment count. It does not construct both recovery
structures. On this population its selected backend matched the measured
faster backend in every row. Timing noise between separate runs produced a
maximum recorded phase regret of 88 microseconds; the maximum retained-memory
regret was 1,696 bytes. Auto remains opt-in for v1.3; CompactOnly continues to
default to sparse subdivision.

The combined all-family size-256 reference-Oracle run exceeded a nine-minute
practical budget while remaining CPU-bound and memory-stable, so no completed
256 release row is claimed. This is recorded as a resource limit rather than a
zero or an inferred measurement.

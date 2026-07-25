# Prepared grid pipeline

CompactOnly constructs one `PreparedComponentContext` per component. The
context owns the component-local occupancy mask, occupancy prefix sums,
horizontal and vertical interior runs, boundary, and reflex coordinates
grouped by row and column. The public convenience APIs remain compatible and
prepare a context internally when they are called in isolation.

## Production path

`GridInteriorRunEnumerator` consumes the stored runs. It uses
`partition_point` to select continuous reflex-coordinate slices and does not
construct a cell `HashSet`, rescan rows or columns, or allocate a filtered
vector per run.

`IndexedFrontierCompletion` uses `DenseCutGrid` as its only mutable cut
authority. Horizontal and vertical cuts are flat component-local Boolean
arrays. Canonical certificate vectors are produced after completion by a
deterministic scan and sort; no mutable `BTreeSet` mirror is maintained.

`DenseGridRecovery` scans prepared occupancy in row-major order, uses dense
visited flags and a reusable integer queue, and proves rectangularity with
region area plus the prepared occupancy prefix sum. The final validator calls
`validate_dissection_prepared` with the same prepared object.

## Correctness references

`ReferencePairwiseEnumerator`, `ReferenceRescanCompletion`, and
`ReferenceHashBfsRecovery` remain available as independent references. Tests
compare exact chord families, selected and added unit cuts, sorted rectangles,
counts, and both ordinary and prepared validation results. Equal optimum
counts alone are not accepted.

## Complexity and scope

For local bounding-box area `A`, dense preparation, recovery, and validation
use `O(A)` storage. Grid-run enumeration remains output-sensitive in emitted
chords after preparation. These are unit-grid engineering bounds, not the
general polygon enumeration or almost-linear flow algorithms cited by the
paper.

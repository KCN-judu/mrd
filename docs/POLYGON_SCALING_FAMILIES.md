# Polygon-native scaling families

The v1.0 scaling campaign constructs polygons directly from integer boundary
coordinates. It does not rasterize a large grid and it does not infer a
complexity claim from a synthetic graph.

| Family | Purpose | Construction |
| --- | --- | --- |
| A | boundary-heavy | variable-depth top notches with many boundary edges and few aligned chords |
| B | aligned-reflex-heavy | four-sided equal-depth notch family maximizing equal-coordinate candidate pairs |
| C | hole-heavy | separated ordinary rectangular holes, with staggered variants |
| D | completion-heavy | four-sided varying-depth notches producing many selected/intersecting cuts |
| E | arrangement-heavy | doubled variable-notch family with many distinct x/y coordinates |
| F | huge-coordinate | fixed combinatorics scaled to `10^12` coordinates where `i64` is safe |
| G | clean path-tree | hole-free four-sided mixed-depth notch family, exercised through `Auto` |
| H | non-clean fallback | ordinary-hole family forcing exact 4D fallback |

For each row the generator records boundary complexity, holes, reflex count,
aligned candidate count `C`, chord count, selected/added cuts, phase timings,
reference/indexed counters, owned-allocation estimates, and exact equality
flags. The committed `size = 1,2,4,8,16` campaign contains 40 verified rows.

The indexed rows have zero Definition 7 full-boundary scans, zero global
completion candidate rebuilds, zero full cut scans, and zero rectangle-per-cell
validator tests. Reference rows retain positive scan counters where the
reference algorithm performs those operations.

## v1.1 sweep candidate-gap evidence

v1.1 reuses families B and C as explicit candidate-gap stress families rather
than inferring sweep behavior from timings. B has equal-depth notches on all
four sides, so both horizontal and vertical aligned-reflex groups grow while
the emitted chord family remains linear. C has ordinary two-dimensional holes
and a growing horizontal aligned group. The v1.1 rows record exact `C`, `q`,
the rational `C / max(1, q)`, direct reference and indexed pair iterations,
sweep event/status/output counts, phase timings, owned-allocation estimates,
and three-backend chord/cut/rectangle equality.

The structural guards require `sweep_event_count <= 2 * (n + r)` for these
ordinary-loop event records, `sweep_output_record_count == q`, and zero
aligned-pair, all-pair, Definition 7 fallback, full-boundary scan, and duplicate
output counters. These are regression checks for this implementation, not a
replacement for the source complexity argument.

## v1.2 sparse-subdivision extensions

The v1.2 generator records `|X|`, `|Y|`, `|X||Y|`, sparse vertices,
half-edges, junctions, interior cycles, dense/sparse owned estimates, cut-index
owned bytes, and completion/recovery/validation timings. New families stress
staircase-sparse, many-coordinates-few-faces, staggered ordinary-hole coordinate
cross-products, completion-heavy cuts, clean path-tree sparse output, and
ordinary-hole 4D-fallback sparse output. Every feasible row compares
line-map/dynamic and dense/sparse final geometry exactly; tables are generated
from committed evidence rather than copied into this document.

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


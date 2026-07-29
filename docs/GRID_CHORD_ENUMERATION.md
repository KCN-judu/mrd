# Grid Interior-Run Chord Enumeration

The production enumerator receives maximal runs and row/column reflex groups
from `PreparedComponentContext`. It obtains each run's coordinate slice with
`partition_point`; it does not rebuild a cell hash set, rediscover runs, or
allocate a filtered vector per run. `grid::oracle::Pairwise` remains the
independent chord-set Oracle.

This workspace supports ordinary finite unit-cell components. For a horizontal
grid line `y`, the open segment between reflex vertices `(a,y)` and `(b,y)` is
an effective chord exactly when every integer `x` in `[a,b)` has both cells
`(x,y-1)` and `(x,y)` in the component. Therefore the valid spans on a fixed
line are the maximal runs of adjacent cell columns satisfying that two-sided
test. Reflex vertices on the line are grouped by the run containing their
coordinate, and every ordered pair in a group is emitted once.

The vertical statement is identical after exchanging `x` and `y`: maximal runs
on line `x` contain every adjacent row `y` for which `(x-1,y)` and `(x,y)` are
present, and aligned reflex-vertex pairs within one run are emitted.

The proof is immediate from the reference Definition 7 predicate: its only
interior condition is the conjunction over the unit subintervals of the span.
Maximal runs partition exactly those unit subintervals, so a pair passes the
reference predicate iff both endpoints lie in one common run. Boundary
contacts and intermediate reflex vertices do not require additional filtering;
they are represented by run endpoints and the pairwise output rule. Ordinary
holes are handled independently on each grid line. Formal ornaments and
degenerate holes remain outside the supported model.

`grid::oracle::Pairwise` retains the nested reflex-pair implementation as
the correctness oracle. `grid::experiment::InteriorRuns` builds a cell mask, scans
the two-sided runs, groups aligned reflex coordinates, and emits canonical
sorted chord records. Its grid-specialized cost is `O(N + r log r + q)` up to
constant-time mask lookups, where `N` is the scanned grid area, `r` is the
number of reflex vertices, and `q` is the number of emitted chords. This is not
the general polygon `O(n log n)` Soltan--Gorpinevich sweep.

The optimized implementation is enabled for CompactOnly only after exact
chord-set differential tests against the reference enumerator. The reference
enumerator remains available for regression tests and CLI diagnostics.

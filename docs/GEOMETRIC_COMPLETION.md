# Deterministic geometric completion

This document fixes the observable completion semantics for ordinary finite
unit-cell components. `ReferenceRescanCompletion` is the authoritative Oracle;
`IndexedFrontierCompletion` is accepted only when it emits exactly the same
selected cuts, added cuts, and canonical rectangles.

## State and order

Horizontal unit cut `(x, y)` separates cells `(x, y - 1)` and `(x, y)`.
Vertical unit cut `(x, y)` separates cells `(x - 1, y)` and `(x, y)`. Effective
chords selected by the minimum-cover complement are materialized into these
unit cuts before any simple chord is considered.

Completion then executes two fixed phases: horizontal, followed by vertical.
Within a phase, vertices are ordered by increasing `y`, then increasing `x`.
Directions retain the reference order `East, North, West, South`, filtered to
the current axis. Thus horizontal ties prefer East to West and vertical ties
prefer North to South.

At a vertex, the four incident cell quadrants and four blocked rays define
connected local angle components. A direction is eligible exactly when its two
adjacent quadrants are inside, its ray is unblocked, both quadrants belong to
the same local angle component, and that component contains at least three
inside quadrants. The next simple chord is always the least eligible key in the
order above.

## Extension and stopping

A simple chord advances by integer unit segments only. Each segment must have
component cells on both sides and must not already be cut. After inserting a
prospective segment, extension stops at the first endpoint with a perpendicular
boundary edge or perpendicular cut. The stopping endpoint is included, while
the perpendicular boundary or cut is not crossed. Existing same-axis cuts and
the component boundary also stop extension. A selected candidate that inserts
no segment is an invariant failure.

The reference backend scans the entire grid vertex domain anew for every
simple chord and once more to prove each axis phase complete. Its
`full_grid_vertex_scans` metric includes those terminal unsuccessful scans.
`concave_candidate_queries` counts direction-specific eligibility tests, and
`ray_extension_unit_steps` counts inspected extension positions including the
terminal position.

The indexed backend performs one initial component-bounding-box scan per axis.
It stores eligible rays in reference order, revalidates a candidate when it is
popped, and uses generation counters for lazy invalidation. Adding a unit cut
refreshes the incident vertices; processing every inserted segment is allowed.
No subsequent full vertex scan is permitted in that phase.

## Rectangle recovery

After both phases, cells connected across uncut shared sides form final
regions. Each region must equal its integer bounding rectangle. Rectangles are
sorted canonically, and the solver-independent cell-exact validator checks
positive area, containment, non-overlap, and exact coverage.

The production indexed path stores cuts only in `DenseCutGrid` and recovers
regions with `DenseGridRecovery`. Dense recovery uses local integer indices, a
visited mask, one reusable queue, and occupancy prefix sums. The preserved
`ReferenceHashBfsRecovery` constructs hash sets and remains a differential
Oracle, not a CompactOnly production dependency.

Differential acceptance compares all four artifacts exactly:

- selected horizontal and vertical unit cuts;
- added horizontal unit cuts;
- added vertical unit cuts;
- sorted final rectangles.

Equal optimum counts alone are insufficient. Any disagreement keeps the
reference backend as the default and requires a minimized permanent fixture.

## Complexity scope

Let `A` be the component-local bounding-box area, `P` its grid-vertex count,
`L` the total number of inserted simple-chord unit segments, and `R` the number
of recovered cells. The reference risk is `O(s P + L + R)` for `s` simple
chords. The indexed target is `O(P log P + L log P + R)` with ordered frontier
operations, while preserving the exact reference policy. This is a practical
grid specialization, not a claim about general polygon completion.

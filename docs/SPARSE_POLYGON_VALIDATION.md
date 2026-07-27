# Sparse Polygon Dissection Validation

`SparseSlabValidator` validates coordinate rectangles without a two-dimensional
coverage table.  It first checks positive areas and exact total area.  It then
sweeps open vertical slabs induced by polygon and rectangle x coordinates.

The production backend applies horizontal boundary parity toggles and rectangle
coverage updates to one lazy y segment tree. Each open slab is checked from the
root without scanning boundary edges, enumerating active rectangles, or
resorting their intervals. The stable priority is overlap, uncovered interior,
then outside coverage. The v1.2 per-slab rescan remains an explicit Oracle; see
[EVENT_DRIVEN_SLAB_VALIDATOR.md](EVENT_DRIVEN_SLAB_VALIDATOR.md).

The data retained is proportional to boundary segments, rectangle events, and
event-tree nodes, not the Cartesian coordinate product. Dense arrangement
validation and the older reference coordinate scan remain selectable and are
cross-checked in FullyAudited mode.

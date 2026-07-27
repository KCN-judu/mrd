# Sparse Polygon Dissection Validation

`SparseSlabValidator` validates coordinate rectangles without a two-dimensional
coverage table.  It first checks positive areas and exact total area.  It then
sweeps open vertical slabs induced by polygon and rectangle x coordinates.

For each slab, horizontal polygon boundary crossings yield the exact parity
union of polygon-interior y intervals.  Active rectangles yield y intervals
with their source indices.  The validator rejects multiplicity greater than
one, compares the union to polygon intervals exactly, and reports the stable
first category among overlap, uncovered interior, and outside coverage.

The data retained is proportional to boundary segments, rectangle events, and
active intervals, not the Cartesian coordinate product.  Dense arrangement
validation and the older reference coordinate scan remain selectable and are
cross-checked in FullyAudited mode.

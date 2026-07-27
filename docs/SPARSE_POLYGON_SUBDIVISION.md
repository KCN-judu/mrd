# Sparse Polygon Subdivision

`SparseOrthogonalSubdivision` replaces coordinate-cell recovery for the
CompactOnly polygon path.  It receives normalized boundary segments and the
final canonical horizontal and vertical cuts, then:

1. collects endpoints and exact orthogonal crossings;
2. splits every segment at crossings and T-junctions;
3. creates paired directed half-edges for every atomic sparse segment;
4. orders outgoing edges by the four exact orthogonal directions;
5. follows the left-face successor (clockwise from the reverse edge);
6. classifies each cycle by an exact half-unit left probe against the prepared
   polygon boundary index.

No `|X| * |Y|` atomic occupancy, barrier, or difference array is allocated.
The builder retains vertices, half-edges, face cycles, junction count, and an
owned-byte estimate.  Bridges are represented by twin half-edges incident to
the same face cycle; they are not assumed to split a face.  T-junctions and
crossing cuts are explicit vertices before the half-edge graph is formed.

For an interior cycle, recovery removes consecutive collinear vertices and
accepts a rectangle only when there are exactly four bbox corners, the cycle
has the bbox perimeter, and exact signed area equals bbox area.  The recovered
rectangle total must equal polygon area; otherwise completion reports a
nonrectangular region.  Dense coordinate arrangement recovery remains the
independent oracle and is run in FullyAudited differential mode.

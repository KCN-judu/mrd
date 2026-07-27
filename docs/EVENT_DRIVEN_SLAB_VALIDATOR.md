# Event-Driven Slab Validator

The event validator preserves the v1.2 slab-rescan implementation as
`reference-slab-rescan`. It checks nonpositive rectangles, exact area overflow,
and total area mismatch before constructing the event tree.

The y universe contains polygon vertex coordinates and rectangle endpoints.
Leaves represent open elementary y intervals. At each x coordinate, events are
applied in deterministic order:

```text
PolygonToggle < RectangleEnd < RectangleStart
```

Horizontal polygon-edge endpoints toggle the parity of the y suffix beginning
at their y coordinate. Rectangle endpoints add `-1` or `+1` over `[y0,y1)`.
The operations commute on the represented state; the order exists to make
traces and future diagnostics stable.

Each tree node stores presence, minimum rectangle coverage, and maximum
coverage separately for polygon parity zero and one, plus lazy coverage and
parity tags. One root check per open x slab detects errors in this priority:

1. rectangle multiplicity greater than one;
2. polygon-interior coverage zero;
3. polygon-exterior positive coverage.

Only an error descends to the first violating leaf and scans rectangles to
recover stable rectangle IDs. Successful validation never enumerates active
rectangles or boundary edges per slab. For `E` x events, `Y` y coordinates,
and `L` reported witness descent depth, the successful path is
`O((E log Y) + number_of_slabs)`; witness recovery adds `O(log Y + R)` only on
failure. Production requires `validator_boundary_edge_scans == 0` and
`validator_active_rectangle_resorts == 0`.

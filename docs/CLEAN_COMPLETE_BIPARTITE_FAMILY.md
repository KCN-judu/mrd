# Clean Complete-Bipartite Family

The finite-grid artifact now includes an integer realization of the corrected
paper construction. For each positive `t`, the generator starts from a solid
rectangle, removes pairwise disjoint one-cell notch intervals on the left and
right, and removes paired intervals on the bottom and top. The notch depths
are distinct and the four notch bands are separated by an integer margin.

Each horizontal notch interval contributes two effective horizontal chords,
and each vertical interval contributes two effective vertical chords. The
generated component is checked by the existing reference/grid-run enumerators:

```text
|H| = |V| = 2t
|E| = 4t^2
G = K_{2t,2t}
```

The V4.1 correction is reflected directly in the integer construction: the two
vertical chords for one interval use the two distinct endpoint coordinates
`b_j^-` and `b_j^+`. The permanent regression test checks clean eligibility,
exact chord counts, complete conflict density, and path-tree solvability for
`t = 1..4`. Larger fixtures can be generated with:

```text
rect-cli generate --family clean-complete-bipartite --t 8 \
  --json /tmp/clean-k16-16.json --svg /tmp/clean-k16-16.svg
```

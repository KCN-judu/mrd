/* global console */
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = resolve(projectRoot, '..', '..');
const algorithmCommit = '093961fc6dbd39d853a18bb793160ac290ed0baf';
const fixturePath = 'test-data/polygons/scaled-complete-bipartite.json';
const witnessPath = 'results/path-tree-witnesses/witness-004-01cb5df6b16637a5.json';
const fixture = readFileSync(resolve(repositoryRoot, fixturePath));
const witnessRaw = readFileSync(resolve(repositoryRoot, witnessPath));
const sha256 = (value) => createHash('sha256').update(value).digest('hex');
const temp = mkdtempSync(resolve(tmpdir(), 'mrd-v1-data-'));

try {
  const solverOutput = resolve(temp, 'solver-output.json');
  execFileSync(resolve(repositoryRoot, 'target/release/rect-cli'), [
    'solve',
    '--solver', 'dominance-compressed',
    '--input-format', 'polygon',
    '--polygon-chords', 'sg-sweep',
    '--input', resolve(repositoryRoot, fixturePath),
    '--output', solverOutput,
  ], {cwd: repositoryRoot, stdio: 'inherit'});

  const output = JSON.parse(readFileSync(solverOutput, 'utf8'));
  const witness = JSON.parse(witnessRaw);
  const payload = output.result.certificate.payload;
  const horizontal = payload.horizontal_chords;
  const vertical = payload.vertical_chords;
  const crossings = horizontal.flatMap((h) => vertical
    .filter((v) => h.left <= v.x && v.x <= h.right && v.bottom <= h.y && h.y <= v.top)
    .map((v) => ({horizontal: h.id, vertical: v.id})));

  const internalCapacity = Math.min(horizontal.length, vertical.length) + 1;
  const nodes = [
    {id: 'source', kind: 'source'},
    ...horizontal.map((chord) => ({id: `h${chord.id}`, kind: 'horizontal', chordId: chord.id})),
    {id: 'b0', kind: 'biclique'},
    ...vertical.map((chord) => ({id: `v${chord.id}`, kind: 'vertical', chordId: chord.id})),
    {id: 'sink', kind: 'sink'},
  ];
  const arcs = [
    ...horizontal.map((chord) => ({from: 'source', to: `h${chord.id}`, capacity: 1})),
    ...horizontal.map((chord) => ({from: `h${chord.id}`, to: 'b0', capacity: internalCapacity})),
    ...vertical.map((chord) => ({from: 'b0', to: `v${chord.id}`, capacity: internalCapacity})),
    ...vertical.map((chord) => ({from: `v${chord.id}`, to: 'sink', capacity: 1})),
  ];
  const directParity = {
    horizontal: horizontal.map((h) => ({id: h.id, coordinates: [2 * h.left, -2 * h.right, 2 * h.y, -2 * h.y]})),
    vertical: vertical.map((v) => ({id: v.id, coordinates: [2 * v.x + 1, -2 * v.x + 1, 2 * v.top + 1, -2 * v.bottom + 1]})),
  };
  const diagnostics = output.result.diagnostics;
  const derived = {
    schemaVersion: 1,
    algorithmCommit,
    generatedBy: 'target/release/rect-cli',
    generatorSha256: sha256(readFileSync(resolve(repositoryRoot, 'target/release/rect-cli'))),
    fixture: {
      sourceId: 'polygon/scaled-complete-bipartite@093961f',
      path: fixturePath,
      sha256: sha256(fixture),
      polygon: {
        outer: output.polygon.outer.vertices.map(({x, y}) => [x, y]),
        holes: output.polygon.holes.map((hole) => hole.vertices.map(({x, y}) => [x, y])),
      },
      horizontalChords: horizontal,
      verticalChords: vertical,
      crossings,
      directParity,
      bicliques: [{id: 0, horizontal: horizontal.map(({id}) => id), vertical: vertical.map(({id}) => id)}],
      compressedNetwork: {nodes, arcs, internalCapacity},
      minimumVertexCover: {
        horizontal: payload.selected_horizontal,
        vertical: payload.selected_vertical,
      },
      selectedHorizontalCuts: payload.selected_horizontal_cuts,
      selectedVerticalCuts: payload.selected_vertical_cuts,
      completionCuts: {
        horizontal: payload.added_horizontal_cuts,
        vertical: payload.added_vertical_cuts,
      },
      rectangles: output.result.rectangles,
      metrics: {
        boundaryComplexity: diagnostics.boundary_complexity,
        reflexVertices: diagnostics.reflex_vertex_count,
        horizontalChords: diagnostics.horizontal_chord_count,
        verticalChords: diagnostics.vertical_chord_count,
        conflicts: crossings.length,
        bicliques: diagnostics.biclique_count,
        bicliqueSigma: diagnostics.biclique_total_vertex_occurrences,
        flowValue: payload.flow_value,
        matchingSize: diagnostics.maximum_matching_size,
        minimumVertexCoverSize: diagnostics.minimum_vertex_cover_size,
        compressedNetworkVertices: diagnostics.compressed_network_vertex_count,
        compressedNetworkArcs: diagnostics.compressed_network_arc_count,
        optimumRectangles: output.result.optimum_rectangle_count,
      },
      representation: payload.representation,
      chordEnumerator: diagnostics.polygon_chord_enumerator,
    },
    pathTree: {
      sourceId: 'path-tree/mutated-notch-074-034@093961f',
      path: witnessPath,
      sha256: sha256(witnessRaw),
      name: witness.name,
      family: witness.family,
      width: witness.width,
      height: witness.height,
      orientation: witness.orientation,
      horizontalChords: witness.horizontal_chords,
      verticalChords: witness.vertical_chords,
      dualTree: witness.dual_tree,
      compactPaths: witness.compact_paths,
      hld: witness.hld,
      bicliquePartition: witness.biclique_partition,
      metrics: witness.diagnostics,
    },
  };

  if (crossings.length !== 16 || arcs.length !== 16 || nodes.length !== 11) {
    throw new Error('V1 fixture no longer has the audited K4,4 compressed-network shape');
  }
  writeFileSync(resolve(projectRoot, 'data/animatic-math.json'), `${JSON.stringify(derived, null, 2)}\n`);
  console.log('Generated data/animatic-math.json from audited repository sources.');
} finally {
  rmSync(temp, {recursive: true, force: true});
}

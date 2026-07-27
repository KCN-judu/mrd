/* global console */
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {readFileSync} from 'node:fs';
import {dirname, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = resolve(projectRoot, '..', '..');
const manifest = JSON.parse(readFileSync(resolve(projectRoot, 'data/manifest.json'), 'utf8'));

const sha256 = (content) => createHash('sha256').update(content).digest('hex');

execFileSync('git', ['merge-base', '--is-ancestor', manifest.algorithmCommit, 'HEAD'], {
  cwd: repositoryRoot,
  stdio: 'ignore',
});

for (const source of manifest.sources) {
  const committed = execFileSync('git', ['show', `${manifest.algorithmCommit}:${source.path}`], {
    cwd: repositoryRoot,
  });
  const working = readFileSync(resolve(repositoryRoot, source.path));
  const derived = JSON.parse(readFileSync(resolve(repositoryRoot, source.derivedPath), 'utf8'));
  const provenance = source.derivedKey ? derived[source.derivedKey] : derived;

  if (sha256(committed) !== source.sha256) {
    throw new Error(`Recorded source hash is stale for ${source.id}`);
  }
  if (sha256(working) !== source.sha256) {
    throw new Error(`Working source differs from recorded source for ${source.id}`);
  }
  if (provenance.sourceId !== source.id || derived.algorithmCommit !== manifest.algorithmCommit) {
    throw new Error(`Derived provenance mismatch for ${source.id}`);
  }
  const derivedHash = provenance.sha256 ?? provenance.fixtureSha256;
  if (derivedHash !== source.sha256) {
    throw new Error(`Derived fixture hash mismatch for ${source.id}`);
  }
}

const animatic = JSON.parse(readFileSync(resolve(projectRoot, 'data/animatic-math.json'), 'utf8'));
const timeline = JSON.parse(readFileSync(resolve(projectRoot, 'script/animatic-scenes.json'), 'utf8'));
if (animatic.fixture.crossings.length !== 16 || animatic.fixture.bicliques.length !== 1) {
  throw new Error('Animatic conflict and biclique evidence no longer matches the audited K4,4 fixture');
}
if (animatic.fixture.compressedNetwork.nodes.length !== 11 || animatic.fixture.compressedNetwork.arcs.length !== 16) {
  throw new Error('Animatic compressed-network topology is stale');
}
if (animatic.fixture.rectangles.length !== 13 || animatic.fixture.metrics.optimumRectangles !== 13) {
  throw new Error('Animatic final dissection is stale');
}
let expectedStart = 0;
for (const scene of timeline.scenes) {
  if (scene.startFrame !== expectedStart || scene.endFrame <= scene.startFrame) {
    throw new Error(`Non-contiguous or invalid scene timing at ${scene.id}`);
  }
  expectedStart = scene.endFrame;
}
if (expectedStart !== timeline.durationFrames || timeline.durationFrames !== 8640) {
  throw new Error('Animatic timeline duration is not exactly 8640 frames');
}
for (const slug of ['en', 'zh-cn']) {
  const timed = JSON.parse(readFileSync(resolve(projectRoot, `script/narration-${slug}-timed.json`), 'utf8'));
  for (const caption of timed.captions) {
    if (caption.startMs < 0 || caption.endMs <= caption.startMs || caption.endMs > timed.durationMs) {
      throw new Error(`Invalid ${slug} caption boundary in ${caption.sceneId}`);
    }
  }
}

console.log(`Verified ${manifest.sources.length} production data source(s) at ${manifest.algorithmCommit}.`);

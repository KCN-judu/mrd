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

  if (sha256(committed) !== source.sha256) {
    throw new Error(`Recorded source hash is stale for ${source.id}`);
  }
  if (sha256(working) !== source.sha256) {
    throw new Error(`Working source differs from recorded source for ${source.id}`);
  }
  if (derived.sourceId !== source.id || derived.algorithmCommit !== manifest.algorithmCommit) {
    throw new Error(`Derived provenance mismatch for ${source.id}`);
  }
  if (derived.fixtureSha256 !== source.sha256) {
    throw new Error(`Derived fixture hash mismatch for ${source.id}`);
  }
}

console.log(`Verified ${manifest.sources.length} production data source(s) at ${manifest.algorithmCommit}.`);

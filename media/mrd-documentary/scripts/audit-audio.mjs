/* global console, process */
import {spawnSync} from 'node:child_process';
import {existsSync} from 'node:fs';
import {dirname, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const target = resolve(projectRoot, process.argv[2] ?? 'out/review/MRDMoodTest1080.mp4');

if (!existsSync(target)) {
  throw new Error(`Audio audit target does not exist: ${target}`);
}

const probe = spawnSync('ffprobe', [
  '-v', 'error', '-select_streams', 'a:0',
  '-show_entries', 'stream=codec_name,sample_rate,channels',
  '-of', 'default=noprint_wrappers=1', target,
], {encoding: 'utf8'});

if (probe.status !== 0 || !probe.stdout.includes('sample_rate=48000')) {
  throw new Error(`Audio stream audit failed:\n${probe.stderr || probe.stdout}`);
}

const loudness = spawnSync('ffmpeg', [
  '-hide_banner', '-nostats', '-i', target,
  '-filter_complex', 'ebur128=peak=true', '-f', 'null', '-',
], {encoding: 'utf8'});

if (loudness.status !== 0) {
  throw new Error(`Loudness audit failed:\n${loudness.stderr}`);
}

const summary = loudness.stderr.slice(loudness.stderr.lastIndexOf('Summary:'));
console.log(probe.stdout.trim());
console.log(summary.trim());

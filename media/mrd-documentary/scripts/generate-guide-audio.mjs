/* global Buffer, console */
import {mkdirSync, writeFileSync} from 'node:fs';
import {dirname, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const output = resolve(projectRoot, 'public/audio/mood-test-guide.wav');
const sampleRate = 48000;
const channels = 2;
const seconds = 17;
const sampleCount = sampleRate * seconds;

let state = 1993;
const noise = () => {
  state = (state * 1664525 + 1013904223) >>> 0;
  return state / 0xffffffff * 2 - 1;
};

const envelope = (time, start, end, attack, release) => {
  if (time < start || time > end) return 0;
  const inGain = Math.min(1, (time - start) / attack);
  const outGain = Math.min(1, (end - time) / release);
  return Math.max(0, Math.min(inGain, outGain));
};

const dataSize = sampleCount * channels * 2;
const buffer = Buffer.alloc(44 + dataSize);
buffer.write('RIFF', 0);
buffer.writeUInt32LE(36 + dataSize, 4);
buffer.write('WAVE', 8);
buffer.write('fmt ', 12);
buffer.writeUInt32LE(16, 16);
buffer.writeUInt16LE(1, 20);
buffer.writeUInt16LE(channels, 22);
buffer.writeUInt32LE(sampleRate, 24);
buffer.writeUInt32LE(sampleRate * channels * 2, 28);
buffer.writeUInt16LE(channels * 2, 32);
buffer.writeUInt16LE(16, 34);
buffer.write('data', 36);
buffer.writeUInt32LE(dataSize, 40);

let filteredNoise = 0;
for (let index = 0; index < sampleCount; index++) {
  const time = index / sampleRate;
  const rawNoise = noise();
  filteredNoise = filteredNoise * 0.94 + rawNoise * 0.06;

  const room = filteredNoise * 0.018 + Math.sin(time * Math.PI * 2 * 47) * 0.004;
  const contact = noise() * 0.075 * envelope(time, 0.9, 1.45, 0.015, 0.42);
  const pencilEnv = envelope(time, 3.55, 9.2, 0.28, 0.75);
  const pencil = (filteredNoise * 0.11 + Math.sin(time * 1650) * 0.008) *
    pencilEnv * (0.72 + 0.28 * Math.sin(time * 38));
  const lockTime = time - 15.2;
  const lock = lockTime >= 0 && lockTime < 1.4
    ? (Math.sin(lockTime * Math.PI * 2 * 698) * 0.065 +
      Math.sin(lockTime * Math.PI * 2 * 139.6) * 0.035) * Math.exp(-lockTime * 4.3)
    : 0;

  const sample = Math.max(-0.9, Math.min(0.9, room + contact + pencil + lock));
  const left = Math.round(sample * 32767);
  const right = Math.round((sample * 0.96 + room * 0.04) * 32767);
  const offset = 44 + index * 4;
  buffer.writeInt16LE(left, offset);
  buffer.writeInt16LE(right, offset + 2);
}

mkdirSync(dirname(output), {recursive: true});
writeFileSync(output, buffer);
console.log(`Generated ${output} (${sampleRate} Hz stereo, ${seconds}s).`);

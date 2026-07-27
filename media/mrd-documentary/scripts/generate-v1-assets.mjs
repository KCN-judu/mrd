/* global Buffer, console */
import {execFileSync} from 'node:child_process';
import {mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const timeline = JSON.parse(readFileSync(resolve(projectRoot, 'script/animatic-scenes.json'), 'utf8'));
const temp = mkdtempSync(resolve(tmpdir(), 'mrd-v1-assets-'));
const audioDir = resolve(projectRoot, 'public/audio');
const scriptDir = resolve(projectRoot, 'script');
const sampleRate = 48000;
const totalSeconds = timeline.durationFrames / timeline.fps;
mkdirSync(audioDir, {recursive: true});

const probeDuration = (path) => Number(execFileSync('ffprobe', [
  '-v', 'error', '-show_entries', 'format=duration', '-of', 'default=nw=1:nk=1', path,
], {encoding: 'utf8'}).trim());

const formatSrtTime = (milliseconds, separator = ',') => {
  const bounded = Math.max(0, Math.round(milliseconds));
  const hours = Math.floor(bounded / 3600000);
  const minutes = Math.floor((bounded % 3600000) / 60000);
  const seconds = Math.floor((bounded % 60000) / 1000);
  const millis = bounded % 1000;
  return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}${separator}${String(millis).padStart(3, '0')}`;
};

const writeCaptionFiles = (language, voice, captions) => {
  const slug = language === 'en' ? 'en' : 'zh-cn';
  const timed = {schemaVersion: 1, language, voice, durationMs: totalSeconds * 1000, captions};
  writeFileSync(resolve(scriptDir, `narration-${slug}-timed.json`), `${JSON.stringify(timed, null, 2)}\n`);
  const srt = captions.map((caption, index) => [
    index + 1,
    `${formatSrtTime(caption.startMs)} --> ${formatSrtTime(caption.endMs)}`,
    caption.text,
    '',
  ].join('\n')).join('\n');
  writeFileSync(resolve(scriptDir, `narration-${slug}.srt`), srt);
  const vtt = `WEBVTT\n\n${captions.map((caption) => [
    `${formatSrtTime(caption.startMs, '.')} --> ${formatSrtTime(caption.endMs, '.')}`,
    caption.text,
    '',
  ].join('\n')).join('\n')}`;
  writeFileSync(resolve(scriptDir, `narration-${slug}.vtt`), vtt);
};

const generateNarration = ({language, voice, rate}) => {
  const inputs = [];
  const filters = [];
  const captions = [];
  timeline.scenes.forEach((scene, sceneIndex) => {
    const text = scene.narration[language].join(' ');
    const aiff = resolve(temp, `${language}-${scene.id}.aiff`);
    execFileSync('/usr/bin/say', ['-v', voice, '-r', String(rate), '-o', aiff, text]);
    const rawDuration = probeDuration(aiff);
    const sceneDuration = (scene.endFrame - scene.startFrame) / timeline.fps;
    const available = sceneDuration - 1.15;
    const speed = rawDuration > available ? rawDuration / available : 1;
    const spokenDuration = rawDuration / speed;
    inputs.push('-i', aiff);
    const speedFilter = speed > 1 ? `atempo=${speed.toFixed(6)},` : '';
    filters.push(`[${sceneIndex}:a]${speedFilter}aresample=${sampleRate},aformat=sample_fmts=fltp:channel_layouts=stereo,adelay=650|650,apad,atrim=0:${sceneDuration.toFixed(6)}[s${sceneIndex}]`);

    const utterances = scene.narration[language];
    const weights = utterances.map((utterance) => Math.max(1, [...utterance].filter((character) => !/\s/u.test(character)).length));
    const totalWeight = weights.reduce((sum, weight) => sum + weight, 0);
    const gapMs = utterances.length > 1 ? 180 : 0;
    const allocatableMs = Math.max(400, spokenDuration * 1000 - gapMs * (utterances.length - 1));
    let cursorMs = scene.startFrame / timeline.fps * 1000 + 650;
    utterances.forEach((utterance, utteranceIndex) => {
      const durationMs = allocatableMs * weights[utteranceIndex] / totalWeight;
      captions.push({
        text: utterance,
        startMs: Math.round(cursorMs),
        endMs: Math.round(cursorMs + durationMs),
        timestampMs: null,
        confidence: null,
        sceneId: scene.id,
      });
      cursorMs += durationMs + gapMs;
    });
  });
  const concatInputs = timeline.scenes.map((_, index) => `[s${index}]`).join('');
  filters.push(`${concatInputs}concat=n=${timeline.scenes.length}:v=0:a=1[out]`);
  const output = resolve(audioDir, `guide-narration-${language === 'en' ? 'en' : 'zh-cn'}.m4a`);
  execFileSync('ffmpeg', [
    '-hide_banner', '-loglevel', 'error', '-y', ...inputs,
    '-filter_complex', filters.join(';'), '-map', '[out]',
    '-c:a', 'aac', '-b:a', '160k', '-ar', String(sampleRate), '-ac', '2', output,
  ]);
  writeCaptionFiles(language, voice, captions);
  console.log(`Generated ${output} with ${captions.length} timed captions.`);
};

const writeWavHeader = (buffer, sampleCount) => {
  const channels = 2;
  const dataSize = sampleCount * channels * 2;
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
};

const generateGuideBed = () => {
  const sampleCount = Math.round(sampleRate * totalSeconds);
  const wav = Buffer.alloc(44 + sampleCount * 4);
  writeWavHeader(wav, sampleCount);
  const cues = [
    {time: 18, frequency: 164.81, duration: 1.1, gain: 0.022},
    {time: 48, frequency: 220, duration: 1.6, gain: 0.024},
    {time: 78, frequency: 246.94, duration: 1.0, gain: 0.02},
    {time: 106, frequency: 261.63, duration: 0.75, gain: 0.026},
    {time: 108.55, frequency: 293.66, duration: 0.75, gain: 0.026},
    {time: 111.1, frequency: 349.23, duration: 0.75, gain: 0.026},
    {time: 113.65, frequency: 392, duration: 1.3, gain: 0.028},
    {time: 164, frequency: 98, duration: 18, gain: 0.012},
    {time: 198, frequency: 73.42, duration: 0.9, gain: 0.032},
    {time: 246, frequency: 261.63, duration: 1.1, gain: 0.026},
    {time: 247.25, frequency: 293.66, duration: 1.1, gain: 0.026},
    {time: 248.5, frequency: 349.23, duration: 1.1, gain: 0.026},
    {time: 249.75, frequency: 392, duration: 2.0, gain: 0.03},
  ];
  let noiseState = 1993;
  let filteredNoise = 0;
  for (let index = 0; index < sampleCount; index++) {
    const time = index / sampleRate;
    noiseState = (noiseState * 1664525 + 1013904223) >>> 0;
    const noise = noiseState / 0xffffffff * 2 - 1;
    filteredNoise = filteredNoise * 0.985 + noise * 0.015;
    const section = Math.floor(time / 30) % 2;
    const ambient = filteredNoise * (section === 0 ? 0.009 : 0.006) + Math.sin(time * Math.PI * 2 * 41) * 0.0025;
    let tone = 0;
    for (const cue of cues) {
      const local = time - cue.time;
      if (local >= 0 && local < cue.duration) {
        const envelope = Math.min(1, local / 0.08) * Math.min(1, (cue.duration - local) / 0.35);
        tone += Math.sin(local * Math.PI * 2 * cue.frequency) * cue.gain * envelope;
      }
    }
    const pulsePhase = time >= 164 && time < 189 ? (time - 164) % 1.25 : -1;
    const pulse = pulsePhase >= 0 && pulsePhase < 0.18
      ? Math.sin(pulsePhase * Math.PI * 2 * 92) * 0.018 * Math.exp(-pulsePhase * 20)
      : 0;
    const cutSilence = time >= 188.6 && time < 190.1 ? 0.08 : 1;
    const sample = Math.max(-0.8, Math.min(0.8, (ambient + tone + pulse) * cutSilence));
    const offset = 44 + index * 4;
    wav.writeInt16LE(Math.round(sample * 32767), offset);
    wav.writeInt16LE(Math.round((sample * 0.97 + ambient * 0.03) * 32767), offset + 2);
  }
  const raw = resolve(temp, 'animatic-guide-bed.wav');
  const output = resolve(audioDir, 'animatic-guide-bed.m4a');
  writeFileSync(raw, wav);
  execFileSync('ffmpeg', [
    '-hide_banner', '-loglevel', 'error', '-y', '-i', raw,
    '-c:a', 'aac', '-b:a', '160k', '-ar', String(sampleRate), '-ac', '2', output,
  ]);
  console.log(`Generated ${output} (${sampleRate} Hz stereo, ${totalSeconds}s).`);
};

try {
  generateNarration({language: 'en', voice: 'Samantha', rate: 168});
  generateNarration({language: 'zh-CN', voice: 'Tingting', rate: 190});
  generateGuideBed();
} finally {
  rmSync(temp, {recursive: true, force: true});
}

import {Audio} from '@remotion/media';
import React from 'react';
import {
  AbsoluteFill,
  Easing,
  interpolate,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from 'remotion';
import {PolygonBoundary} from '../math/PolygonBoundary';
import {palette, typography} from './palette';
import {SourceCitation} from './SourceCitation';

export type RenderQuality = 'draft' | 'review' | 'final';

export type MoodTestProps = {
  quality: RenderQuality;
  withAudio: boolean;
};

const eased = (frame: number, input: [number, number], output: [number, number]) =>
  interpolate(frame, input, output, {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
    easing: Easing.bezier(0.22, 1, 0.36, 1),
  });

const FilmTexture: React.FC<{opacity: number; seed: number}> = ({opacity, seed}) => (
  <svg
    aria-hidden="true"
    width="100%"
    height="100%"
    style={{position: 'absolute', inset: 0, opacity, mixBlendMode: 'soft-light'}}
  >
    <filter id={`grain-${seed}`}>
      <feTurbulence type="fractalNoise" baseFrequency="0.78" numOctaves="3" seed={seed} />
      <feColorMatrix type="saturate" values="0" />
    </filter>
    <rect width="100%" height="100%" filter={`url(#grain-${seed})`} />
  </svg>
);

const ArchiveLayer: React.FC<{frame: number; paperExit: number}> = ({frame, paperExit}) => {
  const exposure = eased(frame, [0, 78], [0.18, 1]);
  const stampOpacity = eased(frame, [28, 78], [0, 0.72]);
  const graphiteProgress = eased(frame, [105, 205], [0, 1]);
  const focus = eased(frame, [22, 78], [3.4, 0]);

  return (
    <AbsoluteFill style={{backgroundColor: '#020303', overflow: 'hidden'}}>
      <div
        style={{
          position: 'absolute',
          width: 1660,
          height: 1180,
          left: 190,
          top: -44,
          transform: `translateX(${-paperExit * 2160}px) rotate(-1.4deg) scale(${1 + paperExit * 0.08})`,
          transformOrigin: '72% 50%',
          backgroundColor: palette.archiveIvory,
          boxShadow: `0 50px 120px rgba(0,0,0,0.72), inset -42px 0 58px rgba(63,42,22,${0.18 + paperExit * 0.12})`,
          filter: `brightness(${0.52 + exposure * 0.48})`,
          willChange: 'transform, filter',
        }}
      >
        <div
          style={{
            position: 'absolute',
            inset: 0,
            backgroundColor: 'rgba(255,244,216,0.08)',
            borderRight: `2px solid ${palette.oxidizedBrown}`,
          }}
        />
        <FilmTexture opacity={0.17} seed={1993} />
        <div
          style={{
            position: 'absolute',
            left: 168,
            top: 158,
            opacity: stampOpacity,
            filter: `blur(${focus}px)`,
            color: 'rgba(62,49,34,0.58)',
            fontFamily: typography.archive,
            fontSize: 176,
            lineHeight: 1,
            letterSpacing: 0,
            textShadow: '1px 1px 0 rgba(255,255,255,0.28), -1px -1px 0 rgba(44,35,25,0.32)',
          }}
        >
          1993
        </div>
        <div
          style={{
            position: 'absolute',
            left: 178,
            top: 360,
            width: 660,
            color: 'rgba(43,38,30,0.6)',
            fontFamily: typography.archive,
            fontSize: 28,
            lineHeight: 1.45,
            letterSpacing: 0,
          }}
        >
          Minimum dissection of a rectilinear polygon
          <br />
          with arbitrary holes into rectangles
        </div>
        <svg
          viewBox="0 0 900 310"
          style={{position: 'absolute', left: 172, top: 546, width: 980, height: 340}}
        >
          <path
            d="M 20 242 C 180 196, 290 210, 418 150 S 660 112, 862 54"
            fill="none"
            stroke="rgba(49,43,34,0.72)"
            strokeWidth="3"
            strokeLinecap="round"
            pathLength="1"
            strokeDasharray="1"
            strokeDashoffset={1 - graphiteProgress}
          />
        </svg>
        <div style={{position: 'absolute', left: 1040, top: 512, width: 410, height: 270, opacity: 0.62}}>
          <PolygonBoundary
            boundaryProgress={graphiteProgress}
            chordProgress={0}
            boundaryColor={palette.graphite}
            chordColor={palette.clueGold}
            lineWidth={0.07}
          />
        </div>
        <div
          style={{
            position: 'absolute',
            right: 40,
            top: 0,
            width: 56,
            height: '100%',
            backgroundColor: 'rgba(255,244,216,0.1)',
            boxShadow: '-20px 0 30px rgba(60,39,21,0.12)',
          }}
        />
      </div>
      <div
        style={{
          position: 'absolute',
          left: 76,
          bottom: 58,
          color: 'rgba(232,236,232,0.68)',
          fontFamily: typography.modern,
          fontSize: 17,
          letterSpacing: 0,
        }}
      >
        VISUAL LANGUAGE TEST / V0
      </div>
    </AbsoluteFill>
  );
};

const MathematicalLayer: React.FC<{frame: number; reveal: number}> = ({frame, reveal}) => {
  const boundaryProgress = eased(frame, [222, 386], [0.03, 1]);
  const cameraScale = eased(frame, [210, 405], [1.34, 1]);
  const cameraY = eased(frame, [210, 405], [78, 0]);
  const chordProgress = eased(frame, [450, 484], [0, 1]);
  const citationOpacity = eased(frame, [462, 495], [0, 1]);
  const settleOpacity = eased(frame, [248, 360], [0.45, 1]);

  return (
    <AbsoluteFill
      style={{
        clipPath: `inset(0 0 0 ${(1 - reveal) * 100}%)`,
        backgroundColor: palette.mathematicalBlack,
        overflow: 'hidden',
      }}
    >
      <svg aria-hidden="true" width="100%" height="100%" style={{position: 'absolute', inset: 0}}>
        <defs>
          <linearGradient id="practical-light" x1="0" y1="0" x2="1" y2="0.7">
            <stop offset="0" stopColor={palette.deepBlue} stopOpacity="0" />
            <stop offset="0.56" stopColor={palette.slate} stopOpacity="0.13" />
            <stop offset="1" stopColor={palette.mutedCyan} stopOpacity="0.03" />
          </linearGradient>
        </defs>
        <path d="M 600 0 L 1920 0 L 1920 860 L 1280 1080 L 430 1080 Z" fill="url(#practical-light)" />
      </svg>
      <div
        style={{
          position: 'absolute',
          left: 238,
          right: 238,
          top: 176,
          bottom: 162,
          opacity: settleOpacity,
          transform: `translateY(${cameraY}px) scale(${cameraScale})`,
          transformOrigin: '50% 52%',
          willChange: 'transform, opacity',
        }}
      >
        <PolygonBoundary
          boundaryProgress={boundaryProgress}
          chordProgress={chordProgress}
          boundaryColor={palette.offWhite}
          chordColor={palette.clueGold}
        />
      </div>
      <div
        style={{
          position: 'absolute',
          left: 72,
          top: 54,
          color: palette.offWhite,
          fontFamily: typography.modern,
          fontSize: 18,
          opacity: eased(frame, [330, 395], [0, 0.72]),
          letterSpacing: 0,
        }}
      >
        AN UNFINISHED MAP INSIDE GEOMETRY
      </div>
      <SourceCitation opacity={citationOpacity} />
      <FilmTexture opacity={0.055} seed={57} />
      <div style={{position: 'absolute', inset: 0, boxShadow: 'inset 0 0 180px rgba(0,0,0,0.76)'}} />
    </AbsoluteFill>
  );
};

export const MoodTest: React.FC<MoodTestProps> = ({quality, withAudio}) => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const paperExit = eased(frame, [210, 330], [0, 1]);
  const mathReveal = eased(frame, [210, 330], [0, 1]);
  const finalFade = eased(frame, [504, 509], [0, 0.16]);

  return (
    <AbsoluteFill style={{backgroundColor: '#020303'}}>
      <ArchiveLayer frame={frame} paperExit={paperExit} />
      <MathematicalLayer frame={frame} reveal={mathReveal} />
      {quality !== 'draft' ? <FilmTexture opacity={quality === 'final' ? 0.035 : 0.024} seed={fps} /> : null}
      <AbsoluteFill style={{backgroundColor: `rgba(0,0,0,${finalFade})`}} />
      {withAudio ? <Audio src={staticFile('audio/mood-test-guide.wav')} /> : null}
    </AbsoluteFill>
  );
};

export const MoodTestPoster: React.FC = () => (
  <AbsoluteFill style={{backgroundColor: palette.mathematicalBlack}}>
    <MathematicalLayer frame={465} reveal={1} />
  </AbsoluteFill>
);

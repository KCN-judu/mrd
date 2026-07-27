import React, {useMemo} from 'react';
import data from '../../data/mood-test.json';

type PolygonBoundaryProps = {
  boundaryProgress: number;
  chordProgress: number;
  boundaryColor: string;
  chordColor: string;
  lineWidth?: number;
};

const toPath = (points: number[][]) => {
  const [first, ...rest] = points;
  return `M ${first[0]} ${-first[1]} ${rest.map(([x, y]) => `L ${x} ${-y}`).join(' ')} Z`;
};

export const PolygonBoundary: React.FC<PolygonBoundaryProps> = ({
  boundaryProgress,
  chordProgress,
  boundaryColor,
  chordColor,
  lineWidth = 0.12,
}) => {
  const boundaryPath = useMemo(() => toPath(data.polygon.outer), []);
  const chord = data.effectiveChords[data.revealedChordIndex];
  return (
    <svg viewBox="-1 -11 22 12" width="100%" height="100%" aria-label="Exact comb polygon boundary">
      <defs>
        <filter id="restrained-gold-glow" x="-70%" y="-70%" width="240%" height="240%">
          <feGaussianBlur stdDeviation="0.11" result="blur" />
          <feFlood floodColor={chordColor} floodOpacity="0.42" result="color" />
          <feComposite in="color" in2="blur" operator="in" result="soft" />
          <feMerge>
            <feMergeNode in="soft" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>
      <path
        d={boundaryPath}
        fill="rgba(120,183,181,0.025)"
        stroke={boundaryColor}
        strokeWidth={lineWidth}
        strokeLinecap="square"
        strokeLinejoin="miter"
        pathLength={1}
        strokeDasharray={1}
        strokeDashoffset={1 - boundaryProgress}
      />
      <line
        x1={chord.left}
        y1={-chord.y}
        x2={chord.right}
        y2={-chord.y}
        stroke={chordColor}
        strokeWidth={lineWidth * 1.35}
        strokeLinecap="round"
        pathLength={1}
        strokeDasharray={1}
        strokeDashoffset={1 - chordProgress}
        filter={chordProgress > 0 ? 'url(#restrained-gold-glow)' : undefined}
      />
      {[chord.left, chord.right].map((x) => (
        <circle
          key={x}
          cx={x}
          cy={-chord.y}
          r={0.11 * chordProgress}
          fill={chordColor}
        />
      ))}
    </svg>
  );
};

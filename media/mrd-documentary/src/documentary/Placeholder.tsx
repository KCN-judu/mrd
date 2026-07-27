import React from 'react';
import {AbsoluteFill, useVideoConfig} from 'remotion';
import {palette, typography} from './palette';

export const Placeholder: React.FC<{label: string}> = ({label}) => {
  const {width, height} = useVideoConfig();
  return (
    <AbsoluteFill
      style={{
        backgroundColor: palette.mathematicalBlack,
        alignItems: 'center',
        justifyContent: 'center',
        color: palette.offWhite,
        fontFamily: typography.modern,
      }}
    >
      <div style={{fontSize: Math.round(Math.min(width, height) * 0.055), letterSpacing: 0}}>{label}</div>
      <div style={{marginTop: 24, fontSize: Math.round(Math.min(width, height) * 0.018), color: palette.mutedCyan}}>
        BLOCKING COMPOSITION / PRODUCTION PHASE PENDING
      </div>
    </AbsoluteFill>
  );
};

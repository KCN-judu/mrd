import React from 'react';
import {palette, typography} from './palette';

export const SourceCitation: React.FC<{opacity: number}> = ({opacity}) => {
  return (
    <div
      style={{
        position: 'absolute',
        right: 72,
        bottom: 54,
        opacity,
        color: palette.mutedCyan,
        fontFamily: typography.modern,
        fontSize: 17,
        lineHeight: 1.45,
        textAlign: 'right',
        letterSpacing: 0,
      }}
    >
      <div style={{color: palette.offWhite}}>polygon/comb@093961f</div>
      <div>verified chord [10,4] - [12,4]</div>
    </div>
  );
};

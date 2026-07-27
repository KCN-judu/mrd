import {Audio} from '@remotion/media';
import React from 'react';
import {
  AbsoluteFill,
  Easing,
  Sequence,
  interpolate,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from 'remotion';
import mathData from '../../data/animatic-math.json';
import timelineData from '../../script/animatic-scenes.json';
import captionsEn from '../../script/narration-en-timed.json';
import captionsZh from '../../script/narration-zh-cn-timed.json';
import {palette, typography} from './palette';

export type AnimaticLanguage = 'en' | 'zh-CN';

export type AnimaticProps = {
  language: AnimaticLanguage;
  withNarration: boolean;
  withGuideBed: boolean;
  withCaptions: boolean;
};

type Scene = (typeof timelineData.scenes)[number];

const ease = (value: number) => Easing.bezier(0.22, 1, 0.36, 1)(Math.max(0, Math.min(1, value)));
const progress = (frame: number, start: number, end: number) => ease((frame - start) / Math.max(1, end - start));
const fadeWindow = (frame: number, duration: number) => Math.min(progress(frame, 0, 18), 1 - progress(frame, duration - 18, duration));
const polyline = (points: number[][]) => points.map(([x, y]) => `${x},${-y}`).join(' ');
const polygonBounds = {minX: 4, maxX: 150, minY: -188, maxY: 14};
const polygonViewBox = `${polygonBounds.minX} ${polygonBounds.minY} ${polygonBounds.maxX - polygonBounds.minX} ${polygonBounds.maxY - polygonBounds.minY}`;

const frameTypography: React.CSSProperties = {
  fontFamily: typography.modern,
  letterSpacing: 0,
};

const GridTexture: React.FC<{warm?: boolean}> = ({warm = false}) => (
  <AbsoluteFill
    style={{
      opacity: warm ? 0.1 : 0.16,
      backgroundImage: warm
        ? 'repeating-linear-gradient(0deg, rgba(62,44,25,0.18) 0, rgba(62,44,25,0.18) 1px, transparent 1px, transparent 7px)'
        : 'linear-gradient(rgba(120,183,181,0.05) 1px, transparent 1px), linear-gradient(90deg, rgba(120,183,181,0.05) 1px, transparent 1px)',
      backgroundSize: warm ? '100% 7px' : '64px 64px',
    }}
  />
);

const PolygonSvg: React.FC<{
  chordProgress?: number;
  conflictProgress?: number;
  rectangleProgress?: number;
  dim?: number;
}> = ({chordProgress = 0, conflictProgress = 0, rectangleProgress = 0, dim = 1}) => {
  const revealCount = Math.floor(chordProgress * (mathData.fixture.horizontalChords.length + mathData.fixture.verticalChords.length + 0.999));
  const completionCount = Math.floor(rectangleProgress * (mathData.fixture.completionCuts.horizontal.length + 0.999));
  return (
    <svg viewBox={polygonViewBox} width="100%" height="100%" aria-label="Audited polygon and chord geometry">
      {rectangleProgress > 0 ? mathData.fixture.rectangles.map((rectangle, index) => {
        const visible = progress(rectangleProgress, index / mathData.fixture.rectangles.length, (index + 1.5) / mathData.fixture.rectangles.length);
        return (
          <rect
            key={`${rectangle.x0}-${rectangle.y0}`}
            x={rectangle.x0}
            y={-rectangle.y1}
            width={rectangle.x1 - rectangle.x0}
            height={rectangle.y1 - rectangle.y0}
            fill={index % 3 === 0 ? 'rgba(120,183,181,0.16)' : index % 3 === 1 ? 'rgba(201,163,76,0.13)' : 'rgba(140,130,157,0.13)'}
            stroke={palette.slate}
            strokeWidth={0.45}
            opacity={visible}
          />
        );
      }) : null}
      <polygon
        points={polyline(mathData.fixture.polygon.outer)}
        fill="rgba(120,183,181,0.025)"
        stroke={palette.offWhite}
        strokeWidth={1.15}
        opacity={dim}
        strokeLinejoin="miter"
      />
      {mathData.fixture.horizontalChords.map((chord, index) => (
        <line
          key={`h-${chord.id}`}
          x1={chord.left}
          y1={-chord.y}
          x2={chord.right}
          y2={-chord.y}
          stroke={palette.mutedCyan}
          strokeWidth={1.15}
          opacity={index < revealCount ? 0.9 : 0}
        />
      ))}
      {mathData.fixture.verticalChords.map((chord, index) => (
        <line
          key={`v-${chord.id}`}
          x1={chord.x}
          y1={-chord.bottom}
          x2={chord.x}
          y2={-chord.top}
          stroke={palette.clueGold}
          strokeWidth={1.15}
          opacity={index + mathData.fixture.horizontalChords.length < revealCount ? 0.9 : 0}
        />
      ))}
      {conflictProgress > 0 ? mathData.fixture.crossings.map((crossing, index) => {
        const h = mathData.fixture.horizontalChords[crossing.horizontal];
        const v = mathData.fixture.verticalChords[crossing.vertical];
        return <circle key={`x-${index}`} cx={v.x} cy={-h.y} r={1.45} fill={palette.clueGold} opacity={progress(conflictProgress, index / 20, (index + 4) / 20)} />;
      }) : null}
      {mathData.fixture.completionCuts.horizontal.slice(0, completionCount).map((cut, index) => (
        <line key={`c-${index}`} x1={cut.left} y1={-cut.y} x2={cut.right} y2={-cut.y} stroke={palette.clueGold} strokeWidth={0.72} opacity={0.72} />
      ))}
    </svg>
  );
};

const SceneShell: React.FC<{scene: Scene; localFrame: number; children: React.ReactNode; warm?: boolean}> = ({scene, localFrame, children, warm = false}) => {
  const duration = scene.endFrame - scene.startFrame;
  return (
    <AbsoluteFill style={{backgroundColor: warm ? palette.graphite : palette.mathematicalBlack, color: palette.offWhite, ...frameTypography, overflow: 'hidden'}}>
      <GridTexture warm={warm} />
      <div style={{position: 'absolute', left: 72, top: 56, display: 'flex', alignItems: 'baseline', gap: 22, opacity: fadeWindow(localFrame, duration)}}>
        <span style={{color: warm ? palette.tungsten : palette.clueGold, fontSize: 18, fontWeight: 700}}>{scene.id}</span>
        <span style={{fontSize: 18, color: warm ? palette.archiveIvory : palette.mutedCyan}}>{scene.act}</span>
        <span style={{fontSize: 28, fontWeight: 500}}>{scene.title}</span>
      </div>
      {children}
      <div style={{position: 'absolute', left: 72, bottom: 38, fontSize: 14, color: warm ? palette.archiveIvory : palette.mutedCyan, opacity: 0.72}}>
        {scene.sourceIds.join('  /  ')}
      </div>
    </AbsoluteFill>
  );
};

const ColdOpen: React.FC<{frame: number; duration: number}> = ({frame, duration}) => {
  const reveal = progress(frame, 0, 120);
  const attempts = [0, 1, 2].map((index) => progress(frame, 160 + index * 48, 220 + index * 48) * (1 - progress(frame, 330 + index * 18, 390 + index * 18)));
  return (
    <div style={{position: 'absolute', left: 230, top: 145, width: 1460, height: 730, opacity: fadeWindow(frame, duration)}}>
      <div style={{position: 'absolute', inset: 0, opacity: reveal, filter: `drop-shadow(0 0 ${18 * reveal}px rgba(232,236,232,0.18))`}}><PolygonSvg dim={0.94} /></div>
      {attempts.map((opacity, index) => (
        <div key={index} style={{position: 'absolute', left: 1050 + index * 55, top: 200 + index * 65, width: 260, height: 150, outline: `2px solid ${index === 2 ? palette.clueGold : palette.slate}`, opacity, transform: `rotate(${index * 2 - 2}deg)`}} />
      ))}
    </div>
  );
};

const Archive: React.FC<{frame: number}> = ({frame}) => {
  const turn = progress(frame, 600, 840);
  return (
    <div style={{position: 'absolute', left: 210, top: 120, width: 1500, height: 820, perspective: 1400}}>
      <div style={{position: 'absolute', inset: 0, backgroundColor: palette.archiveIvory, color: palette.graphite, boxShadow: '0 44px 120px rgba(0,0,0,0.55)', transform: `rotateY(${-turn * 78}deg) translateX(${turn * 420}px)`, transformOrigin: 'right center', padding: '112px 130px', fontFamily: typography.archive}}>
        <div style={{fontSize: 132, color: palette.oxidizedBrown, opacity: progress(frame, 40, 150)}}>1993</div>
        <div style={{marginTop: 48, fontSize: 40, lineHeight: 1.25}}>Minimum dissection of a rectilinear polygon with arbitrary holes into rectangles</div>
        <div style={{marginTop: 28, fontSize: 27}}>Valeriu Soltan  ·  Alexei Gorpinevich</div>
        <div style={{position: 'absolute', right: 110, bottom: 92, borderTop: `2px solid ${palette.oxidizedBrown}`, width: 440, paddingTop: 18, fontSize: 22}}>Discrete & Computational Geometry 9, 57-79</div>
      </div>
      <div style={{position: 'absolute', right: 70, bottom: 80, width: 450, height: 500, opacity: turn, transform: `scale(${0.82 + turn * 0.18})`}}><PolygonSvg chordProgress={0.18} /></div>
    </div>
  );
};

const EffectiveChords: React.FC<{frame: number}> = ({frame}) => {
  const chordProgress = progress(frame, 90, 690);
  return (
    <div style={{position: 'absolute', left: 260, top: 135, width: 1400, height: 790}}>
      <PolygonSvg chordProgress={chordProgress} />
      <div style={{position: 'absolute', right: 10, top: 85, width: 310, fontSize: 20, lineHeight: 1.7, color: palette.mutedCyan}}>
        <div>{mathData.fixture.metrics.reflexVertices} reflex vertices</div>
        <div>{mathData.fixture.metrics.horizontalChords} horizontal chords</div>
        <div>{mathData.fixture.metrics.verticalChords} vertical chords</div>
        <div style={{marginTop: 20, color: palette.clueGold}}>exact SG sweep</div>
      </div>
    </div>
  );
};

const BipartiteGraph: React.FC<{frame: number; compact?: boolean; cut?: boolean}> = ({frame, compact = false, cut = false}) => {
  const leftX = compact ? 570 : 450;
  const rightX = compact ? 1350 : 1470;
  const ys = [285, 430, 575, 720];
  const edgeReveal = progress(frame, 40, 300);
  const collapse = compact ? progress(frame, 260, 620) : 0;
  return (
    <svg width="100%" height="100%" viewBox="0 0 1920 1080">
      {mathData.fixture.crossings.map((crossing, index) => {
        const y1 = ys[crossing.horizontal];
        const y2 = ys[crossing.vertical];
        const bx = 960;
        return (
          <path
            key={index}
            d={collapse > 0 ? `M ${leftX} ${y1} L ${bx} 502 L ${rightX} ${y2}` : `M ${leftX} ${y1} L ${rightX} ${y2}`}
            fill="none"
            stroke={cut && crossing.vertical === index % 4 ? palette.clueGold : palette.slate}
            strokeWidth={cut ? 3 : 1.6}
            opacity={progress(edgeReveal, index / 22, (index + 5) / 22) * (0.28 + collapse * 0.34)}
          />
        );
      })}
      {ys.map((y, index) => <circle key={`h-${index}`} cx={leftX} cy={y} r={25} fill={palette.deepBlue} stroke={palette.mutedCyan} strokeWidth={3} />)}
      {ys.map((y, index) => <circle key={`v-${index}`} cx={rightX} cy={y} r={25} fill={cut ? palette.clueGold : palette.deepBlue} stroke={cut ? palette.clueGold : palette.clueGold} strokeWidth={3} />)}
      {collapse > 0 ? <rect x={880} y={430} width={160} height={144} fill={palette.deepBlue} stroke={palette.clueGold} strokeWidth={3} opacity={collapse} /> : null}
      {collapse > 0 ? <text x={960} y={510} fill={palette.offWhite} fontSize={28} textAnchor="middle" fontFamily={typography.modern} opacity={collapse}>K4,4</text> : null}
      <text x={leftX} y={805} fill={palette.mutedCyan} fontSize={20} textAnchor="middle" fontFamily={typography.modern}>HORIZONTAL</text>
      <text x={rightX} y={805} fill={palette.clueGold} fontSize={20} textAnchor="middle" fontFamily={typography.modern}>VERTICAL</text>
    </svg>
  );
};

const ConflictScene: React.FC<{frame: number}> = ({frame}) => {
  const transform = progress(frame, 80, 570);
  return (
    <>
      <div style={{position: 'absolute', left: 70 - transform * 170, top: 170, width: 960, height: 690, opacity: 1 - transform * 0.5}}><PolygonSvg chordProgress={1} conflictProgress={transform} /></div>
      <div style={{position: 'absolute', inset: 0, opacity: transform}}><BipartiteGraph frame={frame - 80} /></div>
      <div style={{position: 'absolute', right: 80, bottom: 92, fontSize: 21, color: palette.mutedCyan}}>16 exact crossings</div>
    </>
  );
};

const DominanceScene: React.FC<{frame: number}> = ({frame}) => {
  const ribbons = [palette.mutedCyan, palette.softViolet, palette.clueGold, palette.offWhite];
  const comparisons = mathData.fixture.directParity.horizontal[0].coordinates.map((coordinate, index) => ({
    alpha: coordinate,
    beta: mathData.fixture.directParity.vertical[0].coordinates[index],
  }));
  return (
    <div style={{position: 'absolute', left: 180, right: 180, top: 155, bottom: 120}}>
      <div style={{position: 'absolute', top: 0, left: 0, fontSize: 28, color: palette.mutedCyan}}>ranked even/odd endpoint ordering in the implementation</div>
      <div style={{position: 'absolute', top: 68, left: 0, right: 0, display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 24}}>
        {comparisons.map((comparison, index) => {
          const active = progress(frame, 120 + index * 120, 215 + index * 120);
          return (
            <div key={index} style={{height: 440, position: 'relative', backgroundColor: 'rgba(9,20,26,0.7)', outline: `1px solid ${palette.slate}`, overflow: 'hidden'}}>
              <div style={{position: 'absolute', inset: 0, background: `linear-gradient(to top, transparent, ${ribbons[index]}18)`, opacity: active}} />
              <div style={{position: 'absolute', left: 28, top: 32, color: ribbons[index], fontSize: 18}}>COORDINATE {index + 1}</div>
              <div style={{position: 'absolute', left: 28, top: 152, fontSize: 38, opacity: active}}>{comparison.alpha}</div>
              <div style={{position: 'absolute', left: 50, top: 230, color: palette.clueGold, fontSize: 44, opacity: active}}>&lt;</div>
              <div style={{position: 'absolute', left: 28, top: 320, fontSize: 38, opacity: active}}>{comparison.beta}</div>
            </div>
          );
        })}
      </div>
      <div style={{position: 'absolute', left: 0, right: 0, bottom: 122, display: 'flex', justifyContent: 'space-between', alignItems: 'end'}}>
        <div style={{fontFamily: typography.archive, fontSize: 28, lineHeight: 1.5}}>
          <div>alpha(h) = (2l, -2r, 2y, -2y)</div>
          <div>beta(v) = (2x+1, -2x+1, 2t+1, -2b+1)</div>
        </div>
        <div style={{fontSize: 20, color: palette.clueGold, textAlign: 'right'}}>mathematical parity construction<br />not a literal view of 4D space</div>
      </div>
    </div>
  );
};

const CompactScene: React.FC<{frame: number}> = ({frame}) => (
  <>
    <BipartiteGraph frame={frame} compact />
    <div style={{position: 'absolute', left: 0, right: 0, bottom: 112, display: 'flex', justifyContent: 'center', gap: 70, fontSize: 22}}>
      <span>{mathData.fixture.metrics.conflicts} explicit edges</span>
      <span style={{color: palette.clueGold}}>1 exact biclique block</span>
      <span>sigma = {mathData.fixture.metrics.bicliqueSigma}</span>
    </div>
  </>
);

const NetworkScene: React.FC<{frame: number; cut?: boolean}> = ({frame, cut = false}) => {
  const columns = {source: 220, horizontal: 560, biclique: 960, vertical: 1360, sink: 1700};
  const ys = [270, 420, 570, 720];
  const getPosition = (id: string) => {
    if (id === 'source') return [columns.source, 495];
    if (id === 'sink') return [columns.sink, 495];
    if (id === 'b0') return [columns.biclique, 495];
    const index = Number(id.slice(1));
    return [id.startsWith('h') ? columns.horizontal : columns.vertical, ys[index]];
  };
  const pulse = cut ? 0 : (frame % 90) / 90;
  const cutProgress = cut ? progress(frame, 140, 310) : 0;
  return (
    <svg width="100%" height="100%" viewBox="0 0 1920 1080">
      {mathData.fixture.compressedNetwork.arcs.map((arc, index) => {
        const [x1, y1] = getPosition(arc.from);
        const [x2, y2] = getPosition(arc.to);
        return <line key={index} x1={x1} y1={y1} x2={x2} y2={y2} stroke={cut && arc.to.startsWith('v') ? palette.clueGold : palette.slate} strokeWidth={cut ? 3 : 2} opacity={0.72} />;
      })}
      {!cut ? mathData.fixture.horizontalChords.map((_, index) => {
        const t = (pulse + index * 0.13) % 1;
        const x = interpolate(t, [0, 0.33, 0.66, 1], [columns.source, columns.horizontal, columns.biclique, columns.sink]);
        const y = interpolate(t, [0, 0.33, 0.66, 1], [495, ys[index], 495, 495]);
        return <circle key={index} cx={x} cy={y} r={8} fill={palette.clueGold} opacity={0.85} />;
      }) : null}
      {mathData.fixture.compressedNetwork.nodes.map((node) => {
        const [x, y] = getPosition(node.id);
        const selected = cut && node.kind === 'vertical';
        return (
          <g key={node.id}>
            {node.kind === 'biclique' ? <rect x={x - 54} y={y - 42} width={108} height={84} fill={palette.deepBlue} stroke={palette.clueGold} strokeWidth={3} /> : <circle cx={x} cy={y} r={node.kind === 'source' || node.kind === 'sink' ? 35 : 24} fill={selected ? palette.clueGold : palette.deepBlue} stroke={selected ? palette.clueGold : palette.mutedCyan} strokeWidth={3} />}
            <text x={x} y={y + 7} textAnchor="middle" fill={selected ? palette.mathematicalBlack : palette.offWhite} fontSize={20} fontFamily={typography.modern}>{node.id.toUpperCase()}</text>
          </g>
        );
      })}
      {cut ? <line x1={1240} y1={150} x2={1240} y2={840} stroke={palette.clueGold} strokeWidth={5} opacity={cutProgress} /> : null}
      <text x={960} y={880} fill={palette.mutedCyan} fontSize={22} textAnchor="middle" fontFamily={typography.modern}>{mathData.fixture.metrics.compressedNetworkVertices} vertices  /  {mathData.fixture.metrics.compressedNetworkArcs} arcs  /  integral Dinic</text>
      {cut ? <text x={1480} y={880} fill={palette.clueGold} fontSize={24} textAnchor="middle" fontFamily={typography.modern}>minimum vertex cover = 4 vertical chords</text> : null}
    </svg>
  );
};

const PathTreeScene: React.FC<{frame: number}> = ({frame}) => {
  const positions = [[960, 250], [960, 455], [600, 700], [960, 760], [1320, 700]];
  const treeProgress = progress(frame, 60, 310);
  return (
    <svg width="100%" height="100%" viewBox="0 0 1920 1080">
      {mathData.pathTree.dualTree.edges.map((edge, index) => {
        const [x1, y1] = positions[edge.first];
        const [x2, y2] = positions[edge.second];
        return <line key={index} x1={x1} y1={y1} x2={x2} y2={y2} stroke={index === 0 ? palette.clueGold : palette.mutedCyan} strokeWidth={8} opacity={progress(treeProgress, index / 6, (index + 2) / 6)} />;
      })}
      {positions.map(([x, y], index) => <g key={index}><circle cx={x} cy={y} r={42} fill={palette.deepBlue} stroke={palette.offWhite} strokeWidth={3} /><text x={x} y={y + 8} textAnchor="middle" fill={palette.offWhite} fontFamily={typography.modern} fontSize={24}>R{index}</text></g>)}
      <path d="M 600 700 Q 780 455 960 250" fill="none" stroke={palette.softViolet} strokeWidth={12} opacity={progress(frame, 280, 460)} strokeDasharray="18 14" />
      <text x={290} y={260} fill={palette.mutedCyan} fontFamily={typography.modern} fontSize={22}>vertical-tree / horizontal-paths</text>
      <text x={290} y={305} fill={palette.offWhite} fontFamily={typography.modern} fontSize={22}>{mathData.pathTree.dualTree.region_count} exact dual regions</text>
      <text x={290} y={350} fill={palette.clueGold} fontFamily={typography.modern} fontSize={22}>heavy-light canonical intervals</text>
    </svg>
  );
};

const TimelineScene: React.FC<{frame: number}> = ({frame}) => {
  const milestones = [
    ['v0.1', 'exact grid Oracles'], ['v0.3', 'compact execution'], ['v0.5', 'path-tree'],
    ['v0.9', 'boundary-native'], ['v1.1', 'SG sweep'], ['v1.2', 'sparse subdivision'], ['v1.3', 'output-sensitive geometry'],
  ];
  const lineProgress = progress(frame, 20, 430);
  return (
    <div style={{position: 'absolute', left: 150, right: 150, top: 250, height: 500}}>
      <div style={{position: 'absolute', left: 0, right: `${(1 - lineProgress) * 100}%`, top: 220, height: 3, backgroundColor: palette.oxidizedBrown}} />
      <div style={{position: 'absolute', left: 0, top: 185, fontFamily: typography.archive, fontSize: 42, color: palette.archiveIvory}}>1993</div>
      <div style={{position: 'absolute', left: 150, right: 0, top: 0, display: 'grid', gridTemplateColumns: `repeat(${milestones.length}, 1fr)`, gap: 12}}>
        {milestones.map(([tag, label], index) => (
          <div key={tag} style={{height: 300, opacity: progress(frame, 55 + index * 45, 105 + index * 45), transform: `translateY(${index % 2 ? 100 : 0}px)`, borderLeft: `2px solid ${index === milestones.length - 1 ? palette.clueGold : palette.slate}`, paddingLeft: 18}}>
            <div style={{fontSize: 26, color: index === milestones.length - 1 ? palette.clueGold : palette.mutedCyan}}>{tag}</div>
            <div style={{fontSize: 19, lineHeight: 1.3, marginTop: 14}}>{label}</div>
          </div>
        ))}
      </div>
    </div>
  );
};

const FinalDissection: React.FC<{frame: number}> = ({frame}) => (
  <div style={{position: 'absolute', left: 245, top: 125, width: 1430, height: 800, transform: `scale(${1.08 - progress(frame, 320, 580) * 0.08})`}}>
    <PolygonSvg chordProgress={progress(frame, 20, 145)} rectangleProgress={progress(frame, 150, 500)} />
    <div style={{position: 'absolute', right: 5, bottom: 128, color: palette.clueGold, fontSize: 28}}>exact optimum  ·  {mathData.fixture.metrics.optimumRectangles} rectangles</div>
  </div>
);

const Frontier: React.FC<{frame: number}> = ({frame}) => {
  const items = [
    ['FORMAL BOUNDARY', 'represented / solving unfinished'],
    ['POINT + SEGMENT HOLES', 'not yet solved end to end'],
    ['FLOW', 'exact Dinic / not almost-linear'],
    ['COMPLEXITY', 'finite evidence / not a full proof'],
  ];
  return (
    <div style={{position: 'absolute', left: 260, right: 260, top: 185, bottom: 155, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 32}}>
      {items.map(([label, state], index) => <div key={label} style={{borderTop: `2px solid ${index === 2 ? palette.clueGold : palette.slate}`, paddingTop: 32, opacity: progress(frame, 45 + index * 70, 120 + index * 70)}}><div style={{fontSize: 25, color: palette.mutedCyan}}>{label}</div><div style={{fontSize: 34, lineHeight: 1.3, marginTop: 20}}>{state}</div></div>)}
    </div>
  );
};

const EndCard: React.FC<{frame: number}> = ({frame}) => (
  <div style={{position: 'absolute', inset: 100, outline: `2px solid ${palette.clueGold}`, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', opacity: progress(frame, 20, 80)}}>
    <div style={{fontFamily: typography.archive, fontSize: 66}}>Compact matching for</div>
    <div style={{fontFamily: typography.archive, fontSize: 74, marginTop: 8, color: palette.archiveHighlight}}>minimum rectangular dissection</div>
    <div style={{fontSize: 23, color: palette.mutedCyan, marginTop: 54}}>Soltan-Gorpinevich 1993  /  KCN-judu/mrd  /  v1.3.0 evidence</div>
  </div>
);

const SceneVisual: React.FC<{scene: Scene; localFrame: number}> = ({scene, localFrame}) => {
  const duration = scene.endFrame - scene.startFrame;
  const visual = (() => {
    switch (scene.id) {
      case 'S01': return <ColdOpen frame={localFrame} duration={duration} />;
      case 'S02': return <Archive frame={localFrame} />;
      case 'S03': return <EffectiveChords frame={localFrame} />;
      case 'S04': return <ConflictScene frame={localFrame} />;
      case 'S05': return <DominanceScene frame={localFrame} />;
      case 'S06': return <CompactScene frame={localFrame} />;
      case 'S07': return <NetworkScene frame={localFrame} />;
      case 'S08': return <NetworkScene frame={localFrame} cut />;
      case 'S09': return <PathTreeScene frame={localFrame} />;
      case 'S10': return <TimelineScene frame={localFrame} />;
      case 'S11': return <FinalDissection frame={localFrame} />;
      case 'S12': return <Frontier frame={localFrame} />;
      case 'S13': return <EndCard frame={localFrame} />;
      default: return null;
    }
  })();
  return <SceneShell scene={scene} localFrame={localFrame} warm={scene.id === 'S02' || scene.id === 'S10'}>{visual}</SceneShell>;
};

const Captions: React.FC<{language: AnimaticLanguage}> = ({language}) => {
  const frame = useCurrentFrame();
  const {fps} = useVideoConfig();
  const timeMs = frame / fps * 1000;
  const source = language === 'en' ? captionsEn.captions : captionsZh.captions;
  const active = source.find((caption) => caption.startMs <= timeMs && timeMs < caption.endMs);
  if (!active) return null;
  return (
    <div style={{position: 'absolute', left: 250, right: 250, bottom: 76, display: 'flex', justifyContent: 'center', pointerEvents: 'none'}}>
      <div style={{maxWidth: 1320, minHeight: 62, padding: '14px 24px', backgroundColor: 'rgba(2,5,7,0.88)', color: palette.offWhite, fontFamily: typography.modern, fontSize: language === 'en' ? 27 : 30, lineHeight: 1.25, textAlign: 'center', outline: '1px solid rgba(120,183,181,0.26)'}}>{active.text}</div>
    </div>
  );
};

const TimedScene: React.FC<{scene: Scene}> = ({scene}) => {
  const localFrame = useCurrentFrame();
  return <SceneVisual scene={scene} localFrame={localFrame} />;
};

export const Animatic: React.FC<AnimaticProps> = ({language, withNarration, withGuideBed, withCaptions}) => {
  return (
    <AbsoluteFill style={{backgroundColor: palette.mathematicalBlack}}>
      {timelineData.scenes.map((scene) => (
        <Sequence key={scene.id} from={scene.startFrame} durationInFrames={scene.endFrame - scene.startFrame} premountFor={30}>
          <TimedScene scene={scene} />
        </Sequence>
      ))}
      {withGuideBed ? <Audio src={staticFile('audio/animatic-guide-bed.m4a')} volume={0.52} /> : null}
      {withNarration ? <Audio src={staticFile(language === 'en' ? 'audio/guide-narration-en.m4a' : 'audio/guide-narration-zh-cn.m4a')} volume={1} /> : null}
      {withCaptions ? <Captions language={language} /> : null}
    </AbsoluteFill>
  );
};

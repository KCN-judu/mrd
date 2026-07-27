# Compiled Production Specification

## Toolchain decision

- Package manager: npm, because the repository has no existing JavaScript package manager.
- Bootstrap: current official `create-video@latest` blank TypeScript template, adapted in place.
- Locked package family: Remotion `4.0.499`, React, TypeScript.
- Three.js integration: `@remotion/three` `4.0.499` with compatible React Three Fiber and Three.js, reserved for genuinely spatial later scenes.
- Mood test medium: SVG and HTML/CSS only; 3D would add render cost without improving this shot.
- Audio: `@remotion/media`, local 48 kHz WAV through `staticFile()`.
- Rendering: local Remotion CLI with no live API dependency.

The official documentation and npm registry were checked on 2026-07-27. The
current Three integration requires `ThreeCanvas` with explicit dimensions,
frame-driven motion, `Sequence layout="none"` inside the canvas, and ANGLE for
rendering. These constraints are recorded now for V2/V3.

## Compositions

Register the required final composition IDs immediately, even while most use a
clearly labelled blocking placeholder. Also register `MRDMoodTest1080` as the
only V0 production composition and `MRDMoodTestStill` for review.

| ID | Size | Duration | V0 status |
| --- | --- | ---: | --- |
| MRDDocumentary4K | 3840x2160 | 8640 frames provisional | blocking placeholder |
| MRDDocumentary1080 | 1920x1080 | 8640 frames provisional | blocking placeholder |
| MRDTeaser60 | 1920x1080 | 1800 frames | blocking placeholder |
| MRDTeaser20 | 1920x1080 | 600 frames | blocking placeholder |
| MRDSilentLoop | 1920x1080 | 600 frames | blocking placeholder |
| MRDThumbnail | 3840x2160 | still | blocking placeholder |
| MRDMoodTest1080 | 1920x1080 | 510 frames | implemented in V0 |
| MRDMoodTestStill | 1920x1080 | still | implemented in V0 |

## Visual system

Use centralized constants for palette, typography, easing, lens-like depth,
glow ceiling, grain, and render quality. System fonts only in V0: Georgia for
archive text and Helvetica/Arial for metadata. This avoids font redistribution
and network loading.

The paper is a full-frame material field, not a card. The polygon is an exact
SVG path derived from `polygon/comb@093961f`. Grain is deterministic SVG/CSS
texture with fixed seeds. All motion derives from `useCurrentFrame()` and the
composition fps. No CSS keyframes, transitions, random runtime values, or wall
clock state.

## Audio system

Generate an original 17-second WAV at 48 kHz stereo from deterministic synthesis:
low room tone, filtered paper friction, pencil stroke, and one restrained lock.
No melody, sample library, or third-party sound is used in V0.

## Data contract

`data/manifest.json` records algorithm SHA, fixture source path, fixture hash,
and mood-test source ID. `npm run data:verify` must fail on a missing file, hash
mismatch, or algorithm SHA mismatch. This is a minimal V0 provenance check, not
the complete V2 exporter.

## Quality props

All video compositions accept `quality: draft | review | final`. V0 uses the
prop to reduce texture density only; dimensions remain composition-defined.

## External library decision

Remotion is required for deterministic frame rendering. `@remotion/media` is
required for audio. Three.js is intentionally not used in the mood test. No
animation, texture, sound, or stock-asset package is used.

## V1 animatic architecture

`script/animatic-scenes.json` is the authoritative 13-scene timeline. Every
scene records frame bounds, shot IDs, source IDs, and bilingual narration.
`MRDAnimatic1080` renders those contiguous sequences at 1920x1080, 30 fps, and
8,640 frames. English guide narration, guide bed, and English captions are on
by default; props select Chinese narration/captions or disable individual
audio/text layers.

`scripts/export-v1-data.mjs` regenerates `data/animatic-math.json` from the
recorded v1.3 release binary, `scaled-complete-bipartite.json`, and the committed
`mutated-notch-074-034` path-tree witness. The blocking visual chain uses:

- 36 boundary vertices and 16 reflex vertices;
- four horizontal and four vertical effective chords;
- all 16 exact cross-orientation conflicts;
- one exact `K4,4` biclique with sigma 8;
- an 11-node, 16-arc compressed flow network with matching/cut value 4;
- a five-region path-tree witness;
- 13 exact output rectangles.

The dominance scene shows the source-backed even/odd mathematical construction
for one crossing pair while explicitly stating that general polygon input uses
ranked coordinates. It does not claim direct raw-coordinate parity is the
implemented general backend.

`scripts/generate-v1-assets.mjs` uses macOS system voices only for timing guides,
exports the Remotion caption JSON shape plus SRT/VTT, and creates an original
deterministic guide bed. These files are not approved final-public narration or
music. The final voice and licensed/final score remain V5 work.

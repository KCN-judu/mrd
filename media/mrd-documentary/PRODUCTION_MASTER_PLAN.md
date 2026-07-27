# MRD Documentary Production Master Plan

This file is the persistent source of truth for the documentary production.

## Production status

- Algorithm repository SHA: `093961fc6dbd39d853a18bb793160ac290ed0baf`
- Algorithm release represented: `v1.3.0-output-sensitive-sparse-geometry`
- Video-project branch: `codex/mrd-documentary`
- Current production phase: `V1 - Storyboard and animatic`
- Current phase state: `rendered`
- Target master duration: 4 minutes 48 seconds, subject to narration timing
- Master format: 3840x2160, 30 fps, stereo 48 kHz
- Review format: 1920x1080 H.264
- Last completed phase pushed SHA: `4faf416f5fe5d009813d444e07084e6cbe213ea4`
- Remote verification: `origin/codex/mrd-documentary` equalled local branch at the V0 transition
- Existing unrelated worktree content: `tmp/`; preserve and exclude from video commits

## Global narrative treatment

The film treats the algorithm as an unfinished map hidden inside geometry. A
1993 paper supplies the first clue. Exact geometric structures reveal a trail
through effective chords, conflict, four ordered coordinates, biclique blocks,
compressed flow, a minimum cut, and the final optimal dissection. The visual
world moves from warm archival material to cool, exact mathematical space while
retaining tactile light, restrained camera movement, and sharp diagrams.

The film is an investigative mathematical documentary, not a product launch,
code slideshow, neon montage, or imitation of a named creator. The unfinished
formal-boundary and almost-linear-flow frontier remains visible at the end.

## Global visual rules

- Archive: paper ivory, graphite, oxidized brown, tungsten amber, shallow focus.
- Mathematics: near-black blue, off-white geometry, muted cyan, limited gold.
- Implementation: charcoal, cool neutral light, sparse cyan and soft violet.
- Gold and glow indicate a discovered invariant or active relation only.
- Mathematical lines and labels stay exact and sharp.
- Camera motion is motivated by objects and varies by narrative function.
- No arbitrary particles, permanent bloom, generic cards, or template transitions.
- No literal claim to display four-dimensional Euclidean space.

## Global mathematical rules

- Every claim must be registered in `docs/FACT_CHECK.md`.
- Every algorithm scene must name a stable source-data identifier.
- Direct-coordinate parity must not be presented as the general implemented path.
- Dinic must not be described as deterministic almost-linear max flow.
- Formal ornaments, point holes, and segment holes must not be shown as solved.
- Experimental agreement is evidence, not proof of the full theoretical bound.
- Production visuals must use repository fixtures or source-backed constructions.

## Global sound rules

- Separate dialogue, music, effects, archive ambience, interaction, and room-tone stems.
- All production audio is 48 kHz stereo unless a mono source is intentionally retained.
- The four-coordinate leitmotif uses four notes, fragmented before the dominance scene.
- Sound bridges are structural; whooshes are not attached to routine motion.
- Web master target: approximately -14 LUFS integrated, at most -1 dBTP.
- Guide audio must be clearly labelled and must not contain copyrighted commercial music.

## Global copyright rules

- Only self-generated, repository-generated, user-provided, public-domain, or compatibly licensed assets may appear.
- The complete 1993 paper page scans are research sources only and are not redistribution assets.
- Historical pages are represented with self-generated facsimiles and short bibliographic facts.
- Font files are not committed unless their redistribution licence is verified.
- Every asset must be recorded in `ASSET_LEDGER.md` before public use.

## Scene list

| ID | Time | Scene | Data or evidence | State |
| --- | --- | --- | --- | --- |
| S01 | 00:00-00:18 | Cold open: how many rectangles? | `polygon/scaled-complete-bipartite@093961f` | rendered |
| S02 | 00:18-00:48 | 1993 archive | Soltan-Gorpinevich 1993, pp. 57-79 | rendered |
| S03 | 00:48-01:18 | Effective chords | `polygon/scaled-complete-bipartite@093961f` | rendered |
| S04 | 01:18-01:46 | Geometry becomes conflict | 16 exact crossing edges from same fixture | rendered |
| S05 | 01:46-02:18 | Endpoint-preserving 4D dominance | `rect-dominance` ranked parity embedding | rendered |
| S06 | 02:18-02:44 | Dense conflict, compact blocks | exact `K4,4` block, sigma 8 | rendered |
| S07 | 02:44-03:09 | Matching through compressed flow | 11 nodes, 16 arcs, integral capacities | rendered |
| S08 | 03:09-03:26 | Minimum cut | exact vertical cover of size 4 | rendered |
| S09 | 03:26-03:48 | Clean hole-free path tree | `path-tree/mutated-notch-074-034@093961f` | rendered |
| S10 | 03:48-04:05 | Implementation timeline | tags and release evidence | rendered |
| S11 | 04:05-04:25 | Final verified dissection | 13 exact solver rectangles | rendered |
| S12 | 04:25-04:40 | Unfinished frontier | documented limitations | rendered |
| S13 | 04:40-04:48 | End card | current verified version and citations | rendered |

## Narration status

- English narrative concept: approved by V0 treatment
- Chinese narrative concept: approved by V0 treatment
- English provisional script: first V1 pass drafted
- Chinese provisional script: first V1 pass drafted
- English guide narration: generated with system `Samantha`, 48 kHz stereo
- Chinese guide narration: generated with system `Tingting`, 48 kHz stereo
- Timed JSON: 38 cues per language, generated and verified
- SRT/VTT: English and Chinese generated and verified

## Asset inventory

- Local research scan: `tmp/pdfs/soltan-gorpinevich-1993.pdf`; research only, not commit eligible.
- Mood-test geometry: `test-data/polygons/comb.json`; repository-generated MIT/Apache-2.0 project asset.
- Mood-test paper texture: procedurally generated in Remotion; commit eligible.
- Mood-test room tone and page/pencil bridge: self-generated synthesis; commit eligible.
- V1 hero fixture: `scaled-complete-bipartite.json`; repository-generated and commit eligible.
- V1 path-tree witness: `mutated-notch-074-034`; committed repository evidence.
- V1 system TTS: guide-only timing assets; not approved final-public narration.
- V1 guide bed: deterministic original synthesis; commit eligible, provisional.
- Final archive photography, music, narration, and sound effects: not yet acquired.

## Music and sound plan

The historical act uses sparse prepared-piano-like guide tones, bowed texture,
paper movement, and room tone. Mathematical explanation reduces melody and uses
dry, quiet transients at exact events. The four-note motif becomes complete in
the dominance scene, thins during flow, pauses at the minimum cut, and resolves
softly with the final dissection. Detailed timing lives in `audio/cue-sheet.md`
and `audio/beat-map.json`.

## Mathematical verification status

- Repository version and release tag: verified.
- 1993 title, authors, venue, pages, and source theorem: verified from local scan.
- Current ordinary-polygon scope: verified from README and limitations.
- Ranked endpoint-safe parity embedding: verified from algorithm documentation.
- Theorem 8 biclique partition and compressed Dinic path: verified from source docs.
- v1.3 release population claims: verified from committed release evidence.
- Supplied current Version 4.1 paper: referenced by repository docs but no local file located; review required before quoting it directly.
- Rust workspace compile gate: `cargo test --workspace --no-run` passes on the V1 parent state.

## Render status

- Mood test 1080p: rendered and audited at `out/review/MRDMoodTest1080.mp4`
- V1 animatic 1080p: rendered and audited at `out/animatic/MRDAnimatic1080.mp4`
- V1 low-resolution sample: rendered at `out/animatic/MRDAnimaticSample.mp4`
- V1 all-scene contact sheet: `out/animatic/all-scenes-contact-sheet.png`
- Isolated prototypes: not rendered
- Main review master: not rendered
- Delivery masters: not rendered

## Review notes

- V0 mood test must prove tactile paper, an exact boundary, a motivated camera transition, one restrained glow, and a sound bridge.
- The test must not imply that `comb.json` is the final hero fixture.
- Review must include extracted frames, contact sheet, `ffprobe`, and an audio stream check.
- V1 review passed all 13 midpoint stills, exact 8,640-frame count, black-frame
  detection, caption boundaries, 48 kHz stereo audio, and guide loudness/peak audit.

## Commits

- `4faf416f5fe5d009813d444e07084e6cbe213ea4` - V0 production plan, fact check, Remotion baseline, and audited mood test

## Blockers

- Final licensed music and final narration voices are not selected.
- The current Version 4.1 paper mentioned in repository documentation is not present as a local file.

## Final deliverables

- `MRDDocumentary4K`
- `MRDDocumentary1080`
- `MRDTeaser60`
- `MRDTeaser20`
- `MRDSilentLoop`
- `MRDThumbnail`
- clean music-and-effects version
- dialogue, music, and effects stems
- English and Chinese timed narration and SRT/VTT subtitles
- cue sheet, beat map, mix notes, checksums, and final production report

## Phase V0 - Audit and treatment

State: `complete`

Deliverables: repository and source audit, fact-check register, audience brief,
treatment, structure, storyboard, beat sheet, package decision, project skeleton,
17-second mood test, 1080p review render, QA report, commit, and push.

### Mandatory transition after this phase

After this phase has been reviewed, committed, and pushed:

1. Fetch the remote repository.
2. Verify the remote branch SHA equals local HEAD.
3. Reopen `media/mrd-documentary/PRODUCTION_MASTER_PLAN.md`.
4. Reread the complete global visual, mathematical, sound, and copyright rules.
5. Reread the complete next phase.
6. Update the current phase and pushed SHA.
7. Continue automatically unless a hard blocker has been persisted.

## Phase V1 - Storyboard and animatic

State: `rendered`

Deliverables: complete written storyboard, shot list, time-coded animatic,
provisional bilingual narration, blocking visuals, guide audio, and
`MRDAnimatic1080.mp4`. Review narrative clarity, mathematical sequence,
duration, pacing, and non-specialist comprehension.

Implementation and review result: complete pending commit/push closeout. The
authoritative timeline is `script/animatic-scenes.json`; English and Chinese
guide tracks and caption sidecars are generated; the exact fixture chain and
path-tree witness are recorded in `data/animatic-math.json`; the 1080p master
and sample passed the V1 QA record in `qa.md`.

### Mandatory transition after this phase

After this phase has been reviewed, committed, and pushed:

1. Fetch the remote repository.
2. Verify the remote branch SHA equals local HEAD.
3. Reopen `media/mrd-documentary/PRODUCTION_MASTER_PLAN.md`.
4. Reread the complete global visual, mathematical, sound, and copyright rules.
5. Reread the complete next phase.
6. Update the current phase and pushed SHA.
7. Continue automatically unless a hard blocker has been persisted.

## Phase V2 - Mathematical scene prototypes

State: `planned`

Deliverables: polished, isolated, data-audited prototypes for effective chords,
conflict transformation, four-coordinate dominance, biclique compression,
compressed flow and minimum cut, path tree, and sparse subdivision.

### Mandatory transition after this phase

After this phase has been reviewed, committed, and pushed:

1. Fetch the remote repository.
2. Verify the remote branch SHA equals local HEAD.
3. Reopen `media/mrd-documentary/PRODUCTION_MASTER_PLAN.md`.
4. Reread the complete global visual, mathematical, sound, and copyright rules.
5. Reread the complete next phase.
6. Update the current phase and pushed SHA.
7. Continue automatically unless a hard blocker has been persisted.

## Phase V3 - Documentary visual world

State: `planned`

Deliverables: archive environment, paper artifact, historical timeline, spatial
transitions, camera system, materials, lighting, grain, and lens treatment.

### Mandatory transition after this phase

After this phase has been reviewed, committed, and pushed:

1. Fetch the remote repository.
2. Verify the remote branch SHA equals local HEAD.
3. Reopen `media/mrd-documentary/PRODUCTION_MASTER_PLAN.md`.
4. Reread the complete global visual, mathematical, sound, and copyright rules.
5. Reread the complete next phase.
6. Update the current phase and pushed SHA.
7. Continue automatically unless a hard blocker has been persisted.

## Phase V4 - Full visual assembly

State: `planned`

Deliverables: final-timing visual assembly and silent or guide-audio review
master, followed by text, geometry, camera, color, transition, motion, aliasing,
and clipping reviews.

### Mandatory transition after this phase

After this phase has been reviewed, committed, and pushed:

1. Fetch the remote repository.
2. Verify the remote branch SHA equals local HEAD.
3. Reopen `media/mrd-documentary/PRODUCTION_MASTER_PLAN.md`.
4. Reread the complete global visual, mathematical, sound, and copyright rules.
5. Reread the complete next phase.
6. Update the current phase and pushed SHA.
7. Continue automatically unless a hard blocker has been persisted.

## Phase V5 - Sound, music, and narration

State: `planned`

Deliverables: bilingual narration, music, ambience, designed effects, required
sound bridges, key sync, mix automation, separate stems, and loudness audit.

### Mandatory transition after this phase

After this phase has been reviewed, committed, and pushed:

1. Fetch the remote repository.
2. Verify the remote branch SHA equals local HEAD.
3. Reopen `media/mrd-documentary/PRODUCTION_MASTER_PLAN.md`.
4. Reread the complete global visual, mathematical, sound, and copyright rules.
5. Reread the complete next phase.
6. Update the current phase and pushed SHA.
7. Continue automatically unless a hard blocker has been persisted.

## Phase V6 - Final editorial audit

State: `planned`

Deliverables: factual, mathematical, version, copyright, subtitle, timing,
licence, intelligibility, frame, clipping, spelling, citation, and end-card audit,
plus review candidates.

### Mandatory transition after this phase

After this phase has been reviewed, committed, and pushed:

1. Fetch the remote repository.
2. Verify the remote branch SHA equals local HEAD.
3. Reopen `media/mrd-documentary/PRODUCTION_MASTER_PLAN.md`.
4. Reread the complete global visual, mathematical, sound, and copyright rules.
5. Reread the complete next phase.
6. Update the current phase and pushed SHA.
7. Continue automatically unless a hard blocker has been persisted.

## Phase V7 - Delivery

State: `planned`

Deliverables: 4K and 1080p masters, 60-second and 20-second cuts, silent loop,
thumbnail, clean M&E version, stems, subtitles, cue sheet, checksums, and strict
final production report.

### Mandatory transition after this phase

After this phase has been reviewed, committed, and pushed:

1. Fetch the remote repository.
2. Verify the remote branch SHA equals local HEAD.
3. Reopen `media/mrd-documentary/PRODUCTION_MASTER_PLAN.md`.
4. Reread the complete global visual, mathematical, sound, and copyright rules.
5. Reread the complete next phase.
6. Update the current phase and pushed SHA.
7. Continue automatically unless a hard blocker has been persisted.

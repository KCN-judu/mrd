# Quality Assurance Log

## V0 mood test - review 1

- Composition: `MRDMoodTest1080`
- Render: `out/review/MRDMoodTest1080.mp4`
- Contact sheet: `out/review/MRDMoodTest-contact-sheet.png`
- Representative still: `out/review/MRDMoodTestStill.png`
- Low-resolution sample: `out/review/MRDMoodTestSample.mp4`

### Findings and fixes

1. The first SVG review showed subpixel, discontinuous polygon strokes because
   `vector-effect="non-scaling-stroke"` combined with normalized dash lengths.
   Removed the vector effect and normalized both boundary and chord paths to a
   path length of one. Re-rendered frames 150, 390, and 465.
2. The paper scene initially contained a simplified polygon doodle. Replaced it
   with the exact `polygon/comb@093961f` boundary.
3. The still script targeted frame 465 on a one-frame `Still` composition.
   Changed it to render frame 465 from `MRDMoodTest1080` and added a dedicated
   static poster component for the registered still compositions.

### Visual review

- Paper material, shallow-focus impression, and asymmetrical framing: PASS.
- Exact polygon is continuous, sharp, unclipped, and legible: PASS.
- Page-edge/pencil transition is motivated and frame-safe: PASS.
- Restrained gold chord reveal appears only once: PASS.
- Source citation remains inside the 8 percent safe area: PASS.
- No incoherent overlap, text overflow, black frames, or arbitrary template transition: PASS.
- Contact-sheet comparison to `storyboard.md`: PASS.

### Technical review

- Frame count: 510.
- Video: H.264, 1920x1080, 30 fps.
- Audio: AAC stereo, 48 kHz.
- Container duration: 17.045333 seconds; the AAC tail accounts for the 45 ms over picture duration.
- Black-frame detection at 0.2 second minimum and 0.02 pixel threshold: no black intervals reported.
- Guide integrated loudness: -33.9 LUFS.
- Guide true peak: -19.2 dBFS.
- Production data provenance: PASS at `093961fc6dbd39d853a18bb793160ac290ed0baf`.
- Production dependency audit: 0 known production vulnerabilities.
- TypeScript, ESLint, and documentary tests: PASS.
- Fresh Rust workspace compile: FAIL due to pre-existing uncommitted derives in
  `crates/rect-core/src/formal_polygon.rs` requiring serialization support on
  `GeometryError`. The documentary does not modify this file; fixture evidence
  uses the preserved v1.3 release binary recorded in `data/manifest.json`.

### Checksums

- `MRDMoodTest1080.mp4`: `e1fe5c051f22bc0c339c2cbe002ab5c16a9ae85fbd05d992589869f5bd8e9610`
- `MRDMoodTestStill.png`: `f5ce118f4a4a70772d34ef59c55eda3bd2f6912b4625735841e4e0c0800cf85b`
- `MRDMoodTestSample.mp4`: `133429b7297fcb57d22a8afa1889281d8349b022a9bd104eea643e4788f796e2`

V0 render review result: PASS with the documented unrelated Rust build exception.

## V1 animatic - review 1

- Composition: `MRDAnimatic1080`
- Full render: `out/animatic/MRDAnimatic1080.mp4`
- Low-resolution sample: `out/animatic/MRDAnimaticSample.mp4`
- All-scene contact sheet: `out/animatic/all-scenes-contact-sheet.png`
- Scene stills: `out/animatic/stills/S01.png` through `S13.png`

### Findings and fixes

1. The first contact sheet exposed an inverted SVG viewBox that clipped most of
   the tall fixture. Corrected the viewBox and rerendered S01, S03, S05, and S11.
2. Dominance formulas initially occupied the subtitle band. Moved them upward
   and rerendered the scene.
3. The S11 optimum metric initially sat beneath the subtitle band. Moved it into
   the active visual field and rerendered the full master.
4. Verbose full renders were interrupted by the execution channel's per-frame
   output volume. The production command now uses two workers and error-only
   logging; the resulting master completed with all 8,640 frames.

### Visual and editorial review

- Every scene S01-S13 has a distinct blocking transformation and midpoint still: PASS.
- Archive, mathematical, modern, and unfinished-frontier palettes remain distinct: PASS.
- Exact polygon, chord, graph, network, cut, tree, and rectangle data are legible: PASS.
- Captions remain inside the lower safe band without incoherent overlap: PASS.
- Source IDs remain discreet and inside frame: PASS.
- Narrative order matches the approved structure and provisional narration: PASS.
- No arbitrary template transition, literal 4D claim, or unsupported metric: PASS.

### Mathematical review

- `scaled-complete-bipartite.json` SHA-256 and release-binary SHA-256: verified.
- 36 boundary vertices, 16 reflex vertices, 4+4 chords, 16 conflicts: verified.
- One `K4,4` biclique, sigma 8, flow/matching/cut value 4: verified.
- Compressed network has 11 vertices and 16 arcs: verified.
- Final dissection contains the solver's 13 exact rectangles: verified.
- Path-tree scene uses committed witness `mutated-notch-074-034`: verified.
- Dinic and formal-boundary limitations remain explicit: verified.

### Technical review

- Frame count: 8,640; 1920x1080 H.264 at 30 fps.
- Container duration: 288.042667 seconds; picture duration is exactly 288 seconds.
- Audio: AAC stereo, 48 kHz.
- Black-frame detection at 0.15 second / 0.02 threshold: no intervals reported.
- Guide mix: -16.54 LUFS integrated, -4.34 dBTP, 3.20 LU range.
- English captions: 38 valid cues; Chinese captions: 38 valid cues.
- TypeScript, ESLint, data verification, and documentary tests: PASS.
- Production data sources verified at `093961fc6dbd39d853a18bb793160ac290ed0baf`.

### Checksums

- `MRDAnimatic1080.mp4`: `b2e03a9565edeb0bfad57a6783bc3761d7fc17d678ce93fef6aee763c2383b18`
- `MRDAnimaticSample.mp4`: `f233a5fb864dd4917a9c7c393adacad4571efd05fab02ffbb1ad1b17395d467e`
- English guide narration: `84018db11d80092e70a1410cc2d8ab1646601c923b3a2c64b8218d6f418c7544`
- Chinese guide narration: `806a31d708a473e3bb661737246107c5c94f618b33c15db3f77d8a97a1a049a9`
- Guide bed: `fedd633426edd8b7724ff8e7d1a8182fd83c6681f24e49e6421cc4d2e4c6ea39`

V1 animatic review result: PASS for blocking, timing, mathematical sequence,
guide narration, subtitles, and audiovisual delivery. Final narration and
licensed/final music remain later-phase work.

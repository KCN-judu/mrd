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

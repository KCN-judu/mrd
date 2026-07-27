# Delivery Log

## V0 review delivery

Render command:

```bash
npm run render:review
```

- Output: `out/review/MRDMoodTest1080.mp4`
- Codec: H.264 video, AAC audio
- Picture: 1920x1080, 30 fps, 510 frames
- Audio: 48 kHz stereo original guide ambience and sound bridge
- QC: PASS; see `qa.md`
- Distribution status: local review artifact only; `out/` is intentionally not committed

This is not a final-film delivery. It contains no narration, licensed music, or
final mix and must not be published as the completed documentary.

## V1 animatic review delivery

Render command:

```bash
npm run render:animatic
```

- Output: `out/animatic/MRDAnimatic1080.mp4`
- Codec: H.264 video, AAC audio
- Picture: 1920x1080, 30 fps, 8,640 frames
- Audio: English guide narration plus original guide bed, stereo 48 kHz
- Runtime: 288.042667 seconds including AAC tail
- Subtitle sidecars: English and Chinese SRT, VTT, and timed JSON
- QC: PASS; see the V1 section in `qa.md`
- Distribution status: internal animatic review only

The system voices and guide score are explicitly provisional. This file must
not be represented as the final-public documentary master.

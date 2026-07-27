# Mix Notes

## V0

- Guide-only original synthesis; no final music or narration.
- 48 kHz stereo WAV source embedded in the review render.
- Keep the room tone near the noise floor and the clue lock dry and restrained.
- Pencil friction must continue across the visual page-edge transition.
- Audit integrated loudness and true peak, but do not normalize the guide test to trailer loudness.

## Final target

- approximately -14 LUFS integrated for web master
- true peak no higher than -1 dBTP
- narration consistently intelligible
- natural music ducking, no brick-wall limiting

## V1 animatic

- English and Chinese system TTS are guide-only timing references.
- Dialogue and bed are separate 48 kHz stereo AAC assets in `public/audio/`.
- Remotion mixes narration at unity and the guide bed at 0.52.
- Rendered English animatic measures -16.54 LUFS integrated and -4.34 dBTP.
- This intentionally leaves headroom for later music, effects, and final voice;
  V5 will perform the final -14 LUFS / -1 dBTP normalization and stem mix.
- The bed contains no commercial sample, stock music, or external sound library.

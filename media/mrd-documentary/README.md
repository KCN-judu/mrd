# MRD Documentary

Remotion production for *Compact Matching for Minimum Rectangular Dissection*.
The persistent production status and phase gates live in
`PRODUCTION_MASTER_PLAN.md`.

## Requirements

- Node.js 20 or newer
- npm
- FFmpeg and ffprobe for delivery audits

## Setup

```bash
npm install
npm run audio:generate
npm run data:verify
npm run dev
```

## V0 review

```bash
npm run typecheck
npm run lint
npm run test
npm run render:stills
npm run render:sample
npm run render:review
npm run audio:audit
```

The V0 review outputs are written to `out/review/`. Final compositions are
registered from the start but remain visibly labelled blocking compositions
until their production phase is complete.

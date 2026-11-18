# Review Lab

Phone-first review inbox for fourfour UI states, built on the shared labkit package. Rules, card format and the full workflow: the global `lab-review` skill. Read it before adding a card, reading notes or locking a decision.

## Layout

- `lab.html`, `lab.ts`, `labkit.config.json` labkit entry (`index.html` is a copy so `/` works)
- `data/cards/<id>.json` card, `data/cards/<id>/notes/*.json` picks and notes, `data/cards/<id>/media/` screenshots, gifs, mp4s
- `data/locked/` locks, `data/logo.svg` header logo (the app icon source)
- `scripts/review-shot.ts` fourfour-only capture tool (`hub:shot`)
- `scenarios/` capture scenarios per branch (`ov-1028.ts`) and the card commands that go with them (`ov-1028.cards.sh`)

## Commands (run in `lab/`)

```
npm install
npm run build
npm run lab:review
npm run hub:card -- ...
npm run hub:notes
npm run hub:lock -- --kind <kind> --title ... --summary ...
```

`hub:lock` takes `--kind --title --summary` (the old one took a card id and marked it done). Closing a card is status `done` (see the skill).

### Capture media from a fourfour branch

`hub:shot` builds the branch's `app/` with its own node_modules, serves it on a spare port, boots it against the smoke Tauri mock (`app/smoke/tauri-mock.ts`, `fixture.ts`) in headless Chromium at 1440x900, and runs the scenarios in a file. The branch needs the smoke harness and `app/node_modules`.

```
npm run hub:shot -- --app <worktree or app dir> --scenarios scenarios/<branch>.ts [--only a,b] [--id card-id] [--tracks N] [--port 5399]
```

- Output goes straight to `data/cards/<card id>/media/` (the scenario name, or `--id`): png screenshots, mp4 videos (H.264) and optional gifs. mp4 and gif need ffmpeg
- `--tracks N` sets the fixture track count (use it for scroll scenarios, for example 150)
- A scenario is an async function in `scenarios/<branch>.ts`. `boot({ storage, holdLoad, video, viewport, state })` starts a fresh app, `shot(label, { clip })` writes a png, `emit(event, payload)` fires a Tauri event, `finishVideo(label, { gif })` stops recording and converts it. Copy `scenarios/ov-1028.ts` as a starting point
- Smoke hooks such as `__smokeHoldLoad`, `__smokeHoldImport` and `__smokeEmit` are described in `app/smoke/tauri-mock.ts`

Then post the card with media urls of the form `/data/cards/<id>/media/<file>` (see `scenarios/ov-1028.cards.sh`). Option keys are lowercase. Commit `lab/data`.

### Review server

Port 4220, bound to 127.0.0.1:

```
cd lab && npm run lab:review
```

`vite build` then `vite preview`. The server reads `data/` on every request, so new cards and picks show up on Refresh without a rebuild. Rebuild only after changing `lab.ts` or `labkit.config.json`.

lab-infra row (`~/dev/projects/lab-infra/ports.tsv`, tab separated):

```
fourfour	review	4220	dev/projects/fourfour/lab	out=$HOME/.cache/preview/fourfour-lab; npx vite build --outDir $out --emptyOutDir && npx vite preview --outDir $out --host 127.0.0.1 --port $PORT --strictPort
```

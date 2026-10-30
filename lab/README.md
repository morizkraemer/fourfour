# Review Lab

Phone-first review inbox for fourfour UI states. Cards are media first (png, gif, mp4 side by side), one tap picks an option, an optional note goes with it. Picks are stored as JSON in this folder and committed.

## Layout

- `data/cards/<id>.json` card, `data/cards/<id>/notes/*.json` picks and notes, `data/.read.json` notes already printed by `hub:notes`
- `media/<id>/` screenshots, gifs, mp4s (copied in by `hub:card`)
- `src/` inbox page and card model, `server/` JSON store and the API plugin, `scripts/` CLIs
- `scenarios/` capture scenarios per branch (`ov-1028.ts`) and the card commands that go with them (`ov-1028.cards.sh`)
- `.capture/` scratch output of `hub:shot` (gitignored)

## Commands (run in `lab/`)

```
npm install
npm test
npm run build
```

### Add a card from a fourfour branch

1. Capture. `hub:shot` builds the branch's `app/` with its own node_modules, serves it on a spare port, boots it against the smoke Tauri mock (`app/smoke/tauri-mock.ts`, `fixture.ts`) in headless Chromium at 1440x900, and runs the scenarios in a file. The branch needs the smoke harness and `app/node_modules`.

```
npm run hub:shot -- --app <worktree or app dir> --scenarios scenarios/<branch>.ts [--only a,b] [--id card-id] [--tracks N] [--port 5399]
```

   - Output goes to `.capture/<scenario name>/` (or `.capture/<card-id>` with `--id`): png screenshots, mp4 videos (H.264, phone friendly) and optional gifs. mp4 and gif need ffmpeg on the box
   - `--tracks N` sets the fixture track count (use it for scroll scenarios, for example 150)
   - A scenario is an async function in `scenarios/<branch>.ts`. `boot({ storage, holdLoad, video, viewport, state })` starts a fresh app, `shot(label, { clip })` writes a png, `emit(event, payload)` fires a Tauri event, `finishVideo(label, { gif })` stops recording and converts it. Copy `scenarios/ov-1028.ts` as a starting point
   - Smoke hooks such as `__smokeHoldLoad`, `__smokeHoldImport` and `__smokeEmit` are described in `app/smoke/tauri-mock.ts`

2. Post the card with labelled images:

```
npm run hub:card -- --id <branch>-<topic> --title "Top-bar status chip" --question "Keep as is?" \
  --media "idle=.capture/<name>/1-idle.png" --media "analysing=.capture/<name>/2-analysing.png" \
  --options "K=Keep,C=Change (note)"
```

   `scenarios/ov-1028.cards.sh` is a full example. Use short option keys (one or two letters): the key is shown as a badge next to the label.

3. Commit `lab/data` and `lab/media`. The review server reads them on every request, no rebuild needed.

### hub:card flags

- `--title` required, `--question` the one-line ask
- `--media [label=]path` repeat for several, png jpg gif webp mp4 webm, swipe order = argument order. No label gives a, b, c
- `--options a,b,c` or `a=Wide,b=Tight` (key=label). Without options the card takes a note only
- `--url` optional link shown under the media
- `--id slug` fixed id. Posting an existing id starts the next round on the same card: new media and question, earlier picks stay on disk but no longer count

### Read his answers

```
npm run hub:notes            # unread picks and notes, then marks them read
npm run hub:notes -- --all   # everything again
```

### Lock a decided card

```
npm run hub:lock -- <card-id>   # leaves the inbox
```

### Review server

Port 4220, bound to 127.0.0.1:

```
cd lab && npm run review
```

`vite build` then `vite preview`. The server reads `data/` and `media/` on every request, so new cards and picks show up on Refresh without a rebuild. Rebuild only after changing `src/`.

lab-infra row (`~/dev/projects/lab-infra/ports.tsv`, tab separated):

```
fourfour	review	4220	dev/projects/fourfour/lab	out=$HOME/.cache/preview/fourfour-lab; npx vite build --outDir $out --emptyOutDir && npx vite preview --outDir $out --host 127.0.0.1 --port $PORT --strictPort
```

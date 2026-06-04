# Todo

## 2026-06-04 Migrate Prototyping To Viewport Package

- [x] Copy project-owned artboards into `prototyping/viewport/artboards`.
- [x] Copy project-owned design-system into `prototyping/viewport/design-system`.
- [x] Add `prototyping/viewport.config.js` with fourfour layers and a fresh storage namespace.
- [x] Install/use the local `viewport` package and switch `npm run dev` to the package CLI.
- [x] Remove duplicated prototype engine files after the package CLI renders all artboards.
- [x] Verify headlessly: package CLI render, `screen-browse`, camera persistence, and artboard HMR.

### Review (2026-06-04)
- Checkpoint commit before migration: `26c9a5b chore: checkpoint fourfour prototype state`.
- Moved prototype content to `prototyping/viewport/artboards` and `prototyping/viewport/design-system`.
- Added `prototyping/viewport.config.js` with `storageNamespace: 'fourfour'`.
- Switched `npm run dev` to `viewport --port 5180` via the local `file:../../viewport` package.
- Removed the old duplicated `prototyping/src` canvas/content, `index.html`, and `vite.config.js`.
- Headless smoke after clean restart: 20 cards rendered, `screen-browse` present, camera persisted across reload, artboard HMR updated in place, no console/page errors.

## 2026-06-04 First-Screen UI — Modules + Browse Composite

Goal: build the six main-window **modules** matching the Pencil reusable components, then compose
them into a **Browse first-screen composite**. Pencil (`designs/fourfour_design_system.pen`) is truth.
Canonical layout = "main layout v3" (1440×900): `[sidebar 240 | table fill | detail ~278]` / player 96 / statusbar 24.

Audit findings — primitives to fix/add FIRST (bottom-up):
- [ ] **track-row**: extend `TRACK_COLUMNS` + `renderCell` for the Pencil cell types it lacks —
      `fav` (14px toggle box), `wave` (mini-waveform), `cover` (18px thumb), `key` (camelot-colored mono).
      Keep text/tag cells working so existing artboards don't break.
- [ ] **waveform** (NEW primitive): Canvas-2D `createWaveform({peaks?,width,height,variant}) → {element,update}`.
      Used by track-row `wave` cell + player module.
- [ ] **camelot key colors**: shared token-mapped helper for the 12 hues (track-row key cell + detail pane).

Modules to build (layer 'module', own width/padding/gap; export reusable builders):
- [ ] `module-sidebar` (240w) · `module-table` (fill) · `module-detail-pane` (278w) ·
      `module-player` (96h) · `module-statusbar` (24h) · `module-global-header` (search, for completeness).

Composite:
- [ ] `screen-browse` (layer 'composite'): pin 1440×900, `overflow:hidden`, flattened slots (no composite spacing).
      Rows: main-area `[sidebar | table | detail]` / player / statusbar. Compose module builders AS-IS.

Verify: `cd prototyping && npm run build` (no errors) + eyeball each card vs Pencil node screenshots.

### Review (2026-06-04)
- Primitives: added `waveform` (Canvas-2D faux peaks) + `keyColor` util + 12 `--ff-key-*` tokens
  (mirrored in design_system.md). Extended `track-row` with `fav`/`wave`/`cover`/`key` cell types
  (non-breaking: default TRACK_COLUMNS untouched; modules pass their own column set).
- Modules built (all export a builder for the composite): `module-sidebar` (240w, traffic lights +
  Favorites/Library/Playlists + USB pinned bottom), `module-table` (Pencil column set, 12 rows w/
  states), `module-detail-pane` (278w), `module-player` (full-width compact), `module-statusbar`,
  `module-global-header` (search; not wired into v3 composite).
- Composite: `screen-browse` pins 1440×900, flattened slots, `[sidebar|table|detail]`/player/status.
- Build: ✓ 180 modules transformed, no errors.
- VERIFICATION GAP: no visual check run (per instruction "no visual tests / claude-in-chrome").
  Needs eyeball in `npm run dev` — the "Screen · Browse" + 6 "Module ·" cards — vs Pencil.

## 2026-06-03 Prototyping Canvas Blank Fix

- [ ] Confirm `CLAUDE.md` no longer requires version bumps for every edit.
- [ ] Replace the Konva-backed prototype canvas with a DOM/CSS transform canvas so `#world` stays mounted.
- [ ] Remove the unused Konva dependency from the prototyping package manifest and lockfile.
- [ ] Verify with build and non-visual checks only.

## Prototyping Canvas Review

- Pending.

## 2026-04-22 Key Detection Benchmark

- [x] Document Beatport/Rekordbox baseline and Essentia benchmark results.
- [x] Document local setup and commands for rerunning key-only benchmarks.
- [x] Check generated benchmark artifacts and ignore rules.
- [x] Keep `essentia` as the only new key-detection dependency.
- [x] Commit and push the feature branch after verification.

## Review

- `essentia_key_bgate` scored 54.0% exact and 68.9% exact-or-adjacent on the 598-track clean Beatport subset.
- Rekordbox scored 47% exact and 55% exact-or-adjacent on the user's broader 698-track Beatport run.
- This satisfies the current requirement: open-source key detection should at least match Rekordbox for this project.

## 2026-04-22 Analysis CLI README

- [x] Add an LLM-oriented entrypoint README for the analysis CLI.
- [x] Document setup, commands, backend variants, architecture, artifact layout, and verification.
- [x] Commit and push the README update.

## 2026-04-22 Merge Analysis CLI Into Master

- [x] Merge `origin/feat/analysis-cli` into `master`.
- [x] Preserve the newer master waveform/Pioneer analysis stack.
- [x] Route compatibility analysis through the final stack.
- [x] Keep `python -m fourfour_analysis analyze ... --json` compatible with the Tauri caller.
- [x] Verify Python tests, CLI smoke checks, and Rust workspace compile.

## Merge Review

- `fourfour-analyze` uses the final production stack.
- `fourfour-benchmark` keeps key-only benchmarking and `--no-waveform` controls.
- `python -m fourfour_analysis analyze` returns Pioneer waveform fields required by `pioneer-test-ui`.
- Waveform implementation stays on the newer master stack.

## 2026-04-22 Single Analysis CLI Contract

- [x] Make `fourfour-analyze <file> --json` emit the complete single-track analysis object.
- [x] Include BPM, key, energy, beats, cue points, waveform preview/color/peaks, and Pioneer 3-band waveform fields.
- [x] Keep `python -m fourfour_analysis analyze ... --json` as a compatibility wrapper returning a list.
- [x] Update CLI tests and README docs.

## Single-File Beatport Smoke Test

- Track: `5152629 Bob Moses - Far From the Tree (Original Mix).mp3`
- Beatport key label: `E minor` / `9A`
- Rekordbox result: `Em` / `9A`, BPM `111.0`
- Previous wrong Lexicon-style BPM path: key `9A`, BPM `175.0`, energy `8`, no extractor errors.
- Corrected DeepRhythm production path: key `9A`, BPM `111.0`, energy `5`, no extractor errors.
- Output shape is correct: preview `400`, color `2000`, peaks `2000`, Pioneer detail `18000`, Pioneer overview `1200`.
- Beats/cues are intentionally empty until the separate beatgrid/first-beat analyzer is integrated.

## 2026-04-22 Correct Final Stack

- [x] Replace Lexicon-style production BPM/energy with DeepRhythm BPM + librosa energy.
- [x] Keep Essentia `bgate` as the production key detector.
- [x] Keep the current Pioneer waveform analyzer in the CLI orchestrator.
- [x] Leave `lexicon_port` registered only as a benchmark/reference backend.

## 2026-04-22 CLI Batch Benchmark Script

- [x] Promote the ad hoc Beatport 50 runner into a tracked reusable script.
- [x] Add `--tmux` mode that opens a detached pane and lets it close when the run finishes.
- [x] Support arbitrary audio folders in the batch runner.
- [x] Split `benchmark/` gitignore rules: track scripts, manifests, baselines, and docs; ignore datasets, results, logs, cache, and archives.
- [x] Document script usage for agents and humans.

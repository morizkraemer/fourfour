# Waveform Bug Handoff

_Last updated: 2026-06-04 — bug still open after partial fixes_

## Symptoms (reporter)

1. **Track table (mini waveform column)** — Analyzed tracks sometimes show a flat placeholder (`----` / minimal bars) until the row is **clicked** (selection → detail path). BPM/key may already show as analyzed.
2. **Player strip (bottom)** — Some tracks show a **large, scrambled** color waveform that does **not** match the audio. Others look fine.

Both issues were reported **still present** after the fixes listed in [Attempted fixes](#attempted-fixes-2026-06-04).

---

## Two separate data paths (do not conflate)

| Surface | Component | Data source | Encoding |
|---------|-----------|-------------|----------|
| **List row** | `TrackRow` → `Waveform.svelte` (`variant="mini"`) | `track.peaks` on library store object | Normalized floats: `(byte & 0x1F) / 31.0` from 400-byte PWAV preview |
| **Detail pane** | `DetailPane` → `Waveform.svelte` (`variant="strip"`) | Same `track.peaks` | Same |
| **Player strip** | `WaveformStrip` → `WaveformRenderer` | `player.track.colorData` **or** `player.track.previewBytes` | **Color**: `{ amp, r, g, b }` per sample (0–1). **Mono**: raw 400 bytes, decoded in renderer with `byte & 0x1f` + whiteness bits |

List/detail never use `waveform_color`. Player prefers color when valid, else mono preview bytes.

---

## End-to-end pipeline

```
Python fourfour_analysis
  → waveform_preview[400]
  → pioneer_3band_detail (≈ duration×150)
  → pioneer_3band_overview (1200)
       ↓
pioneer-test-ui run_track_analysis (main.rs)
  → lib.set_analysis(track_id, AnalysisResult)
  → emit analysis-progress (after DB write — fixed 2026-06-04)
       ↓
Tauri get_analysis_data(track_id)
  → waveform_preview: Vec<u8> from DB
  → waveform_color: JSON from color_waveform_for_player() (overview downsampled, max 2048)
       ↓
app getAnalysisData() + in-memory cache
  → sanitizeAnalysisPayload() (analysis-data.ts)
       ↓
┌────────────────────────────┬──────────────────────────────┐
│ ensureTrackPeaksLoaded()   │ loadPlayerTrack()            │
│ → track.peaks              │ → previewBytes + colorData   │
│ → bumpLibraryTracks()      │ → WaveformRenderer           │
└────────────────────────────┴──────────────────────────────┘
```

---

## Key files

| Area | Path |
|------|------|
| IPC + cache | `app/src/services/tauri.svelte.ts` — `getAnalysisData`, `peekAnalysisData`, `analysisCache` |
| Validation | `app/src/services/analysis-data.ts` — `isValidAnalysisPayload`, `sanitizeAnalysisPayload`, `isValidColorWaveform` |
| List peaks load | `app/src/stores/library.svelte.ts` — `ensureTrackPeaksLoaded`, `bumpLibraryTracks`, `loadState` forEach, `handleAnalysisProgressEvent` |
| Row UI | `app/src/modules/TrackTable.svelte` — `TrackRow` + `rowState()` / `isTrackAnalyzing` |
| Detail trigger | `app/src/modules/Detail.svelte` — `$effect` → `ensureTrackPeaksLoaded` |
| Player load | `app/src/stores/player.svelte.ts` — `loadPlayerTrack`, `applyAnalysisToTrack`, `peekAnalysisData` |
| Player UI | `app/src/modules/Player.svelte`, `prototyping/.../WaveformStrip.svelte` |
| Canvas draw | `prototyping/.../waveform-renderer.ts` — `#renderColor`, `#renderMono` |
| Mini canvas | `prototyping/.../Waveform.svelte` |
| Backend command | `pioneer-test-ui/src/main.rs` — `get_analysis_data`, `run_track_analysis`, `color_waveform_for_player` |
| Analysis output | `analysis/src/fourfour_analysis/analyze.py`, `waveform.py` |
| DB storage | Library crate used by `pioneer-test-ui` (set_analysis / get_analysis) |

---

## Attempted fixes (2026-06-04)

### List / reactivity

- **`ensureTrackPeaksLoaded(track)`** — single inflight loader; used from `loadState`, `Detail`, and per-track refresh after analyze.
- **`bumpLibraryTracks()`** — `library.tracks = [...library.tracks]` after mutating `track.peaks` so Svelte list rows re-render.
- **Detail** — removed `peaksLoaded = true` without peaks (old code blocked background fetch).
- **Load condition** — `track.analyzed && !track.peaks?.length` instead of `!track.peaksLoaded`.

### IPC / cache / races

- **`getAnalysisData`** — no longer returns random mock data on failure; throws and does not cache invalid payloads.
- **`peekAnalysisData`** — evicts invalid cache entries.
- **`analysis-progress` emit** moved to **after** `set_analysis()` in `main.rs` (was emitted before DB write).
- **`refreshTrackAnalysisDisplay`** — retries up to 4× with backoff after progress events.

### Player / color

- **`sanitizeAnalysisPayload`** — strips invalid `waveform_color`, keeps 400-byte preview.
- **`color_waveform_for_player`** — always prefers **overview** (1200) downsampled, not raw detail slice.
- **`applyAnalysisToTrack`** — only sets `colorData` if `isValidColorWaveform()`.
- **`WaveformRenderer.setColorData`** — rejects out-of-range samples.

### Analyzing UX (related)

- Placeholder sine animation on mini waveform while `analyzingTrackIds` includes track (`Waveform.animating`).

**Reporter: issues unchanged** — treat fixes as necessary but insufficient; continue from hypotheses below.

### Structural fix (2026-06-04, second pass) — addresses list issue at the root

Confirmed **A1 + A4 + missing-ANLZ placeholder** were the real list-flat causes:

- **`has_analysis` now on `TrackInfo`** (`dto.rs`, `build_track_infos` in `main.rs`). `analyzed`
  in `mapTrackInfoToLocal` uses it instead of `tempo > 0`.
- **Preview embedded at load**: `build_track_infos` reads each analyzed track's 400-byte preview
  and ships it in `TrackInfo.waveform_preview`; `mapTrackInfoToLocal` sets `track.peaks` directly.
  Removed the N+1 `ensureTrackPeaksLoaded` loop from `loadState` → no more object-replacement race.
- **Placeholder rejection**: all-zero preview (missing ANLZ) dropped in backend filter +
  `isValidWaveformPreview` (now requires a non-zero 5-bit height byte).
- **Cues still lazy-load**: `ensureTrackPeaksLoaded` gates on new `analysisLoaded` flag (Detail pane).

Builds green (cargo + npm). **Player "scrambled color" still unverified on hardware** — most likely
stale pre-fix ANLZ (B1/B4); re-analyze a bad track to confirm. That's the remaining open thread.

### Mini vs player consistency (2026-06-04, third pass)

Symptom: a track shows a mini list waveform but a flat player strip, or vice versa.

Root cause: the two surfaces draw from different ANLZ sections — mini = DAT/PWAV mono preview,
player = EXT color (with mono fallback). Both come from the same `sf.read` decode in Python, so
they're consistent at the source, **except** when a stored DAT has a degenerate/zero PWAV section
while the color EXT is intact → player renders color, mini (mono-only) renders nothing.

Fix (`main.rs`): single `effective_preview(&AnalysisResult)` helper, used by **both**
`build_track_infos` (list embed) and `get_analysis_data` (player) — returns the real PWAV when it
carries signal, else a 400-byte preview **synthesized from the color overview**
(`mono_preview_from_color`), else `None`. Now any track with *either* a real preview or color data
paints in both surfaces. Cargo build green.

---

## Hypotheses still worth testing

### A. List previews empty until click

| # | Hypothesis | How to verify |
|---|------------|----------------|
| A1 | **`track.analyzed` false** while DB has analysis (`analyzed` = `tempo > 0` in `mapTrackInfoToLocal` only) | Compare `loadState` track `raw.tempo` vs `get_analysis_data` success for same `id` |
| A2 | **`ensureTrackPeaksLoaded` fails silently** (IPC error, no analysis row) | Log in catch; check Settings IPC diagnostic / `lastTauriError` |
| A3 | **`bumpLibraryTracks` insufficient** — row still not repainting | Log when peaks set; inspect whether `TrackRow`/`Waveform` `$effect` runs without selection |
| A4 | **`loadState` replaces track objects** after peaks were set on old references | Trace timing: `loadState` at end of `analyzeTracks` vs in-flight `ensureTrackPeaksLoaded` |
| A5 | **Click path works** only because selection runs a **second** fetch that succeeds | Diff network/logs: fetch on boot vs on select for same track id |
| A6 | **Waveform canvas** not redraw when `peaks` prop updates (effect dependency bug) | Breakpoint in `Waveform.svelte` `$effect`; force `peaks` reference change |

### B. Player strip scrambled

| # | Hypothesis | How to verify |
|---|------------|----------------|
| B1 | **Stale bad data in SQLite** from earlier analysis runs (before color/overview fix) | Re-analyze one bad track; compare `get_analysis_data` JSON in Rust log or temporary dump command |
| B2 | **`waveform_color` length ≠ duration mapping** (overview 1200 vs wrong `duration_ms`) | Log `colorData.length` and `duration_ms` per track; scrub and see if playhead position matches spectral features |
| B3 | **Double normalization** — Python 0–1 bands re-scaled in Rust `waveform_color_json` incorrectly for some peaks | Dump raw `[low,mid,high]` from DB vs JSON sent to frontend for one good and one bad track |
| B4 | **Still serving cached garbage** from an older app session (before mock removal) | Quit app fully, delete in-memory cache is per session — also try `invalidateAnalysisCache()` + re-fetch; consider clearing analysis blob in DB for one track |
| B5 | **Color mode when preview is correct** — force mono: temporarily pass `colorData={null}` in `Player.svelte`; if mono looks correct, bug is in 3-band path only |
| B6 | **`WaveformRenderer` zoom/offset** state stuck from previous track | `trackKey` effect calls `resetView()` — verify `player.track.id` changes on selection |

---

## Reproduce

```bash
cd app && npm run build
cd pioneer-test-ui && ./dev.sh   # or: cargo tauri dev
```

1. Library with **mix of analyzed and new** tracks (or import folder → analyze all).
2. **Without clicking rows**, scan wave column — note which rows stay flat despite BPM filled in.
3. Click a flat row — note if mini waveform appears immediately.
4. Select a **bad** player track → play → compare visual to audio; try another track on same album/format.
5. Re-analyze one bad track (context menu) → reload player — did strip fix?

---

## Debug commands / snippets

**IPC diagnostic** — Settings gear → shows `lastTauriError` pattern.

**Browser devtools** (Tauri webview): watch console for:

- `ensureTrackPeaksLoaded failed:`
- `Failed to refresh analysis display:`
- `get_analysis_data fail:` (in store / settings)

**Temporary logging** (suggested for next agent):

```typescript
// library.svelte.ts ensureTrackPeaksLoaded success branch
console.debug('[peaks]', id, track.peaks?.length);

// player.svelte.ts applyAnalysisToTrack
console.debug('[player wf]', trackId, {
  previewLen: analysis.waveform_preview?.length,
  colorLen: analysis.waveform_color?.length,
  durationMs: analysis.duration_ms,
});
```

**Rust**: log in `get_analysis_data` — `waveform_preview.len()`, `waveform_color.len()`, `duration_ms`.

**Legacy reference UI** (known-good renderer): `pioneer-test-ui/frontend/app.js` `showWaveform` / `renderColorWaveform` — double-click track in vanilla test UI if still available.

---

## Known sharp edges

- **`analyzed` flag** on UI track is `tempo > 0`, not “has analysis blob”. Mismatch → no peak fetch.
- **`loadState()`** calls `invalidateAnalysisCache()` and **replaces** entire `library.tracks` array — races with background peak loads.
- **Parallel analysis** (3 Python workers) — multiple tracks complete out of order; progress message matches by **filename** (`Analyzed {file_name} (n/m)`).
- **No analysis** → `get_analysis_data` returns Err `"No analysis data for this track"` (no mock anymore).
- **DB path**: `~/Library/Application Support/com.pioneer.test-ui/library.db` (see main handoff).

---

## Suggested next steps (priority)

1. **Prove where list breaks** — add short-lived logging on `ensureTrackPeaksLoaded` success/fail + `bumpLibraryTracks`; confirm whether peaks exist on store object while UI is flat (reactivity) vs fetch never runs (`analyzed` / IPC).
2. **Prove player data** — for one scrambled + one good track, dump `get_analysis_data` JSON to file (temporary Tauri command or log) and compare `waveform_preview` + first/last 5 `waveform_color` entries.
3. **A/B mono-only player** — force `colorData={null}`; if mono matches audio, fix 3-band pipeline (Python → Rust → renderer), not preview bytes.
4. **Bulk heal** — “Re-analyze all” or invalidate stored color waveforms in DB for tracks analyzed before overview-only player path.
5. **Structural fix** — store `peaks` on `TrackInfo` from backend in `loadState` / `build_track_infos` so list doesn’t depend on N+1 `get_analysis_data` calls and object mutation reactivity.

---

## Related docs

- `docs/session/handoff.md` — general app/session state
- `docs/player-strip-plan.md` — player waveform architecture
- `docs/analysis-pipeline-handoff.md` — Python analysis sidecar
- `pioneer-usb-writer/reference-code/PIONEER.md` — PWAV 400-byte format

---

## Branch / tree note

Work done on dirty `feat/svelte-ui` (or current branch) across `app/`, `prototyping/viewport/design-system/`, `pioneer-test-ui/src/main.rs`. Rebuild **both** frontend and Tauri after Rust changes:

```bash
cd app && npm run build
cd pioneer-test-ui && cargo build
```

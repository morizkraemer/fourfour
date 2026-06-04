# Session Handoff

_Last updated: 2026-06-04 (evening)_

## Where we are

**Phase 6 (Tauri wiring) is largely done** on branch `feat/svelte-ui` (dirty tree). The Svelte app in `app/` is the production frontend for `pioneer-test-ui` (`tauri.conf` → `app/dist`, dev on :5200).

### Working now
- Persistent SQLite library at `~/Library/Application Support/com.pioneer.test-ui/library.db` — empty after reset; import via **Import Folder** in `EmptyState` or header (when tracks exist).
- Cover art: `Uint8Array` fix in blob creation; background fetch on `loadState()` for all tracks with `has_artwork`.
- Mini waveforms: from `get_analysis_data` → 400-byte ANLZ preview (`(byte & 0x1F) / 31.0`); flat `----` line when unanalyzed (`Waveform` with `peaks: null`).
- Post-import **Analyze Tracks** modal (design-system `Dialog` + `Button` actions).
- Empty library: `EmptyState` in table, player “No Track Loaded”, detail hidden when nothing selected.
- Favorites 1/2 seeded + “Add favorite”; playlists filtered in sidebar.
- Resizable table columns (`TABLE_COLUMNS` in `library.svelte.ts`).
- Settings gear → `Dialog` with DB path / version / IPC diagnostic (not full `SettingsPage` artboard yet).

### Not wired yet
- **Playback polish**: tick is wall-clock based (not sample-locked); beat grid/zoom not on strip yet.
- **Search**: header search is visual only.
- **Curate / USB write flows**: partial (Sync/Wipe when USB volume selected).
- **Full settings UI**: prototype has `createSettingsPage` (panels artboard); app uses a small modal.
- **Waveform player strip**: `WaveformRenderer` + rodio playback wired (see `docs/player-strip-plan.md`).

## Recent UI alignment (this session)
- Added shared **`Dialog.svelte`** + `dialog-native.css` in `prototyping/viewport/design-system/`; exported from `$ds`.
- Replaced bespoke `<dialog class="ff-playlist-dialog">` / `ff-native-dialog` in `App.svelte`, `Sidebar.svelte`, `GlobalHeader.svelte` with `$ds` `Dialog` + `Button`.
- Window: `tauri.conf.json` — `title: ""`, `decorations: false`; fake traffic lights in sidebar + `env(titlebar-area-height)` padding on sidebar chrome and global header so search band aligns with macOS controls.
- Fixed missing `changeLibraryPath` import in `library.svelte.ts`.

## Architecture reminder
- **Primitives**: shared under `prototyping/viewport/design-system/`, imported via `$ds` → `svelte.js`.
- **Modules**: app-only in `app/src/modules/` (interactive).
- **Canvas**: static reference in `prototyping/viewport/artboards/`.

## Verify
```bash
cd app && npm run build
cd pioneer-test-ui && ./dev.sh   # or cargo tauri dev
```

## Immediate next steps (user priority order)
1. Eyeball empty library → import folder → confirm covers load in table without selection; waveforms flat until Analyze.
2. Verify **player strip** — select analyzed track, play/scrub; expand player artboard later.
3. Port **SettingsPage** from prototype when ready (replace minimal settings dialog).
4. Beat grid + zoom on strip (port remainder of `frontend/app.js` waveform UX).

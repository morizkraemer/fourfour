# USB sync: UI contract and engine changes

**Date:** 2026-10-15
**Status:** UI built against mocks on branch `ui-sync-flow`. The engine side below is not built. Owner of the engine: morizkraemer.

## UX model

A stick holds only the playlists the DJ ticks for it (plus any loose tracks dragged onto it), remembered per stick by volume name. Pressing Sync never writes straight away: it opens a plan that lists the tracks to add, the tracks to remove (by name, and only tracks fourfour itself wrote), the unchanged count, the tracks skipped because they are not analyzed (with an "Analyze first" action), and the size against the free space. Only after the DJ confirms does the engine write. While it writes, the stick header shows a progress bar; afterwards it keeps the report counts and "Safe to eject" until dismissed. Files fourfour did not put on the stick are never deleted. Wipe uses the same kind of dialog and states exactly what it removes.

## Where the UI code lives

| Piece | File |
|---|---|
| Contract types, command wrappers, event listeners | `app/src/services/tauri.svelte.ts` |
| Browser-mode mock engine (stateful fake sticks) | `app/src/mocks/usb.ts`, `app/src/mocks/library.ts` |
| Frontend plan (stand-in for the dry run) | `app/src/services/usbPlan.ts` |
| Per-stick state, sync, wipe, eject | `app/src/stores/usb.svelte.ts` |
| Stick header, playlist checklist | `app/src/modules/UsbPanel.svelte` |
| Plan and wipe dialogs | `app/src/modules/UsbDialogs.svelte` |

The UI asks the engine what it supports (`usb_engine_capabilities`). Anything the engine reports as unsupported is handled honestly in the UI (a notice in the plan dialog, a different wipe warning, "Capacity unavailable"). Implement a feature, flip its flag, and the UI uses it with no other change.

## Commands and events the UI expects

All errors are rejected `String`s (as today); the UI shows the text as is, so write them for the DJ ("Not enough space on X: needs 1.2 GB, 100 MB free. Nothing was written.").

### Commands

| Command | Arguments | Returns | Notes |
|---|---|---|---|
| `usb_engine_capabilities` | none | `UsbEngineCapabilities` | Missing command is treated as all `false`. |
| `get_mounted_volumes` | none | `string[]` | Existing. Must list removable drives only (see below). |
| `get_volume_capacity` | `{ path: string }` | `VolumeCapacity` | Error or missing command shows "Capacity unavailable". |
| `read_usb_state` | `{ path: string }` | `UsbStateResponse \| null` | Existing. `null` means no Pioneer library on the stick. Gains optional fields below. |
| `plan_usb_sync` | `{ volume: string, playlistIds: number[], trackIds?: number[] }` | `UsbSyncPlan` | Dry run, writes nothing. Called only when `dry_run_plan` is true; otherwise the UI computes the plan itself. |
| `write_usb` | `{ outputDir: string, playlistIds: number[], trackIds?: number[] }` | `SyncReport` | Scope is exactly these playlists plus loose tracks. Today it takes `playlists: PlaylistInput[]` and syncs the whole library. |
| `wipe_usb` | `{ path: string }` | `void` | Existing. See the guard below. |
| `eject_volume` | `{ path: string }` | `void` | Existing. |

### Types (JSON, snake_case as in Rust)

```ts
interface UsbEngineCapabilities {
  playlist_scope: boolean;       // write_usb honours playlistIds / trackIds
  dry_run_plan: boolean;         // plan_usb_sync exists
  progress_events: boolean;      // write-progress is emitted
  capacity: boolean;             // get_volume_capacity exists and write_usb checks space first
  ownership_manifest: boolean;   // stick records what fourfour wrote; delete and wipe respect it
}

interface VolumeCapacity { total_bytes: number; free_bytes: number }

interface UsbTrackInfo {         // existing fields plus two optional ones
  id: number; title: string; artist: string; album: string; genre: string;
  key: string; bpm: number; duration: number; usb_path: string;
  size_bytes?: number;           // audio file size on the stick
  written_by_fourfour?: boolean; // from the manifest; absent means unknown
}

interface UsbStateResponse {
  tracks: UsbTrackInfo[];
  playlists: { id: number; name: string; track_count: number }[];
  last_sync?: { at: string; report: SyncReport } | null; // from the manifest, ISO 8601
}

interface SyncReport {           // existing
  tracks_added: number; tracks_updated: number; tracks_replaced: number;
  tracks_removed: number; tracks_unchanged: number;
}

interface UsbPlanTrack { track_id: number | null; title: string; artist: string; size_bytes: number | null }

interface UsbSyncPlan {
  playlist_count: number;
  add: UsbPlanTrack[];
  remove: UsbPlanTrack[];        // only tracks the manifest says fourfour wrote
  unchanged_count: number;
  skipped_unanalyzed: UsbPlanTrack[]; // track_id is the library id so the UI can analyze them
  foreign_kept_count: number;    // files on the stick fourfour did not write; left alone
  bytes_to_add: number;
  bytes_to_free: number | null;  // null when sizes of removed files are unknown
  free_bytes: number | null;
  fits: boolean | null;          // bytes_to_add - bytes_to_free <= free_bytes
}

interface UsbProgress {
  volume: string;
  phase: 'copying' | 'writing-database' | 'cleaning-up';
  current: number;               // step counter, 1-based
  total: number;                 // total steps for this run
  message: string;               // shown as is, e.g. "Copying Solomun - Hyperion"
}
```

### Events

| Event | Payload | When |
|---|---|---|
| `write-progress` | `UsbProgress` | Repeatedly while `write_usb` runs. Optional; without it the UI shows an indeterminate bar. |
| `write-complete` | `SyncReport` | Existing. Once, on success. The UI also uses the command's return value and ignores whichever arrives second. |
| `write-error` | `{ volume: string, message: string }` | Once, if `write_usb` fails after it started. The rejected command carries the same message. |

### Error cases the UI already handles

| Case | Expected result |
|---|---|
| Stick too small | `write_usb` rejects before copying anything: "Not enough space on {name}: needs {x}, {y} free. Nothing was written." |
| Stick removed or I/O error mid-write | `write_usb` rejects and emits `write-error`. The UI re-reads the stick afterwards, so a partial write must leave a readable state. |
| `plan_usb_sync` fails | The plan dialog shows the message and cannot sync. |
| `eject_volume` fails (busy) | Shown in the stick header as "Eject failed." |
| `wipe_usb` fails | Shown inside the wipe dialog. |
| Another write already running | The UI blocks a second sync; the engine should also reject. |

## Engine changes needed

1. **Playlist scope.** `write_usb` takes `playlistIds` and `trackIds` and writes exactly those tracks and playlists. Unanalyzed tracks are skipped as today, and the plan reports them.
   - Current behaviour to fix: `sync_playlists_to_library` deletes every playlist in the library database and recreates them from what the frontend sends. If the UI sent only the chosen playlists, the others would vanish from the library. Until this changes, the UI sends all playlists, and the plan dialog says "Playlist-only sync needs the engine update. This build writes your whole library and every playlist." The engine should read playlists from its own database by id and never replace them from a sync call.
2. **Ownership tracking.** Write a manifest on the stick (for example `/PIONEER/fourfour-manifest.json`) listing every file fourfour wrote (stick path, library track id, size, hash or mtime) plus `last_sync`. Sync deletes only files in the manifest. `read_usb_state` sets `written_by_fourfour` and `last_sync` from it. Sticks without a manifest report `written_by_fourfour` absent, and sync must not delete anything on them until the DJ has done one manifest-creating sync (open question 2).
3. **Dry-run plan.** `plan_usb_sync` returns `UsbSyncPlan` using the same diff code as `write_usb` (`writer/sync.rs`), so the plan and the write cannot disagree. The frontend plan in `usbPlan.ts` matches tracks by artist and title and trusts `written_by_fourfour` when present; it is a stand-in only.
4. **Progress events.** Emit `write-progress` per copied track and per phase. One `total` per run, `current` counts steps.
5. **Capacity check.** `get_volume_capacity`, and a check inside `write_usb` before the first copy: needed bytes (adds minus frees) against free bytes, with a small safety margin for the database and ANLZ files.
6. **Removable-drive filter for `get_mounted_volumes`.** Today it lists all of `/Volumes` on macOS (including "Macintosh HD") and `/media`, `/mnt` on Linux. Only removable or external volumes should appear, since the UI offers Eject and Wipe on every entry.
7. **`wipe_usb` guard.** Today it deletes the whole `PIONEER/` and `Contents/` directories, including files fourfour did not write. With a manifest it should delete only manifest files and empty directories. Refuse system volumes, and refuse a path that is not a mounted removable volume from `get_mounted_volumes`. Until the manifest exists, the UI warning says exactly this ("deletes the PIONEER and Contents folders ... including files fourfour didn't write").

## Mock behaviour (browser mode only)

The mock engine in `app/src/mocks/usb.ts` reports every capability as true except `dry_run_plan` (so the frontend plan is exercised). It has two sticks: "SanDisk 64GB" with fourfour tracks, one track whose library entry is gone, and one foreign file; and "VAULT666" with 100 MB free so the "won't fit" state shows. Opening the page with `?mock=usb-fail` makes the next sync fail mid-write.

In the Tauri app none of this is mocked: the existing commands are called, the missing ones are reported as unsupported, and failures reach the user as text.

## Open questions

1. **Loose tracks.** Dragging tracks onto a stick adds them to a "Loose tracks" list for that stick. Should the engine keep them as a playlist on the stick, or as tracks with no playlist?
2. **Sticks without a manifest.** Old fourfour sticks have no manifest. First sync after the update: treat everything the matcher recognises as fourfour's (current frontend guess), or refuse to delete anything and only add?
3. **Volume key.** The UI keys the per-stick selection by volume name, which collides for two sticks with the same label. Is there a stable id (filesystem UUID) the engine can return from `get_mounted_volumes`?
4. **Persistent (ejected) sticks.** `vision.md` describes sticks you can queue changes for while ejected. The UI lists only mounted sticks. Wanted?
5. **Progress granularity.** One step per track is assumed. Is per-byte progress for a single large FLAC needed?
6. **Wipe scope after the manifest.** Should "Wipe" also remove the Pioneer database when foreign files remain in `Contents/`, which would orphan them on CDJs?
7. **Updated and replaced counts.** The UI shows `tracks_updated` and `tracks_replaced` as one "updated" number in the last-synced line and separately in the result. Fine, or should the plan also list them?

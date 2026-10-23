# Multi-USB sync

**Date:** 2026-10-23
**Status:** Backend built on branch `q25-multi-usb` (from `q24-rb-import`). No UI. `write_usb` is unchanged.
**Code:** `pioneer-library/src/multi_sync.rs`, command `sync_usb_many` in `pioneer-test-ui/src/main.rs`, tests `pioneer-library/tests/multi_sync.rs`, bench `pioneer-library/benches/multi_sync_bench.rs`.

## What it does

Sync two or three sticks in one call. The library is read once, each distinct selection is prepared once, and every stick is written on its own thread. One failing stick never stops the others.

```
sync_usb_many(targets)                       async Tauri command
  lock library  ->  prepare_multi_sync()     read analyzed tracks + playlists once
  unlock        ->  PreparedMultiSync        plain data, no DB handle, no lock
                    .run(sink)               one thread per stick
                      per stick: claim root -> free-space check -> writer sync_usb -> report
```

- `SyncTarget { volume, selection }`. `SyncSelection` is `WholeLibrary` or `Playlists { playlist_ids, track_ids }` (library ids). Both work today; playlist scope is done in this layer (filter, then the same dense-id remap `write_usb` uses), so the UI can pass per-stick playlist choices without waiting for a writer change.
- Targets with equal selections share one prepared copy (`Arc`), so three sticks with the same playlists prepare the track list once.
- The library lock is held only while reading. The slow part (copying, databases) runs in `spawn_blocking` with no lock, so browsing and analysis stay responsive during a sync.
- `sync_usb_many` does not touch library playlists (unlike `write_usb`, which deletes and recreates them from the frontend). It reads them from the database by id.
- Returns one `StickReport` per target, in the order given.

## Event contract

Fits `docs/ui/usb-sync-spec.md` but is a separate pair of events because the spec's `write-progress` has no per-stick identity beyond `volume` and is tied to single-stick `write_usb`. Mapping below.

| Event | Payload | When |
|---|---|---|
| `usb-sync-progress` | `{ volume, phase, done, total, message }` | On each phase change and whenever `done` changes. |
| `usb-sync-stick-report` | `StickReport` | Once per stick when it ends, Done or Failed. |

`volume` is the folder name of the stick, as in the spec. `phase` is one of `checking-space`, `copying`, `writing-database`, `done`, `failed`. `done`/`total` count tracks. On `failed`, `message` is the DJ-readable error. Spec mapping: `copying` = `copying`, `writing-database` = `writing-database`; `cleaning-up` has no equivalent yet (see writer proposals), `current` is called `done` here.

```ts
interface StickReport {
  volume: string; path: string;
  status: 'done' | 'failed';
  report: SyncReport | null;     // existing shape, only when done
  error: string | null;          // DJ-readable, only when failed
  skipped_unanalyzed: number;    // selected tracks without analysis
  elapsed_ms: number;
}
type SyncTarget = { volume: string; selection:
  | { kind: 'whole_library' }
  | { kind: 'playlists'; playlist_ids: number[]; track_ids?: number[] } };
```

The command rejects as a whole (rejected `String`) for: no targets, the same root twice (also via symlink), one root inside another. Everything else is per stick and shows up as `status: 'failed'`.

**Progress is approximate.** The writer has no progress callback, so a watcher thread per stick counts how many of the stick's selected audio files exist (every 250 ms). Already-present files count as done, so a resync jumps straight to `writing-database`. Exact per-track progress needs a writer callback (proposals).

## Safety properties (and the test that covers each)

| Property | How | Test |
|---|---|---|
| 3 sticks get identical, valid exports | Same prepared data, writer called per stick; compared via `read_usb_state`, audio bytes, all ANLZ bytes | `three_sticks_in_parallel_get_identical_exports` |
| One failing stick leaves the others complete | Per-thread result; read-only stick fails, others read back valid | `failing_stick_leaves_the_others_complete` |
| A writer panic or error does not stop others | `thread::join` error becomes a failed report; the watcher stops via a drop guard | `panicking_and_erroring_writers_do_not_stop_other_sticks` |
| Duplicate / overlapping / symlinked roots rejected | Canonicalized, compared by path components, before anything starts | `duplicate_and_overlapping_roots_are_rejected` |
| Free space checked before the first write | `statvfs` (libc) against bytes to add plus margin; nothing written on refusal | `stick_without_room_is_refused_before_anything_is_written`, `real_statvfs_reports_free_space` |
| No second sync on a root already syncing | Process-wide claim per canonical root, released on drop (also on panic) | not tested separately; inside one call overlap is rejected up front |
| Playlist scope writes only the chosen tracks and playlists | Filter in the library layer | `playlist_selection_writes_only_chosen_tracks_and_playlists` |
| Incremental resync removes dropped tracks | Writer diff, unchanged | `second_run_is_incremental_and_removes_dropped_tracks` |

Free-space rule: needed = sizes of tracks not already on the stick (same path and size) + 16 MiB + 128 KiB per track (databases, ANLZ, artwork). Space freed by removals is not credited, so a full stick being rotated may be refused when it would fit; this is deliberate until the writer can tell which removals are safe (ownership manifest). Error text follows the spec: `Not enough space on {name}: needs {x}, {y} free. Nothing was written.` Free space is read with `libc::statvfs` (libc is MIT or Apache-2.0, already in the lockfile through other crates; on non-unix the check passes).

Not covered: removed sticks mid-write (needs hardware), per-file ownership (`never delete files fourfour didn't write` is still the writer's behaviour, see the spec's manifest section).

## Numbers

Synthetic library: short generated WAVs (3 s, about 265 KB each), 16-bit mono 44.1 kHz, analyzed with a fixed fake analysis, written into temp dirs. Machine: `dev` VM, shared with other agents (a release build ran during parts of the runs), so treat ±15% as noise. No FAT image: mounting needs root. So the cyanbox FAT-close effect could not be measured here. `cargo bench -p pioneer-library --bench multi_sync_bench` reproduces (env `BENCH_DIR`, `BENCH_TRACKS`, `BENCH_SECONDS`, `BENCH_WRITER=files`).

**1000 tracks, 265 MB audio, copy + ANLZ only (`BENCH_WRITER=files`)**, wall seconds. CPU is process user+sys. Kernel write is `write_bytes` from `/proc/self/io` (dirty pages queued).

| Dir | Sticks | Serial s | Concurrent s | Speedup | CPU s (conc.) | Written MB |
|---|---|---|---|---|---|---|
| tmpfs `/dev/shm` | 1 | 0.58 | 0.58 | 1.00x | 0.57 | 289 (no disk) |
| tmpfs | 2 | 1.42 | 0.61 | 2.34x | 1.09 | 578 |
| tmpfs | 3 | 2.66 | 0.90 | 2.96x | 1.66 | 866 |
| disk (ext4 virtio) | 1 | 1.13 | 0.86 | 1.31x | 0.77 | 295 |
| disk | 2 | 1.97 | 1.11 | 1.78x | 1.53 | 590 |
| disk | 3 | 3.33 | 1.37 | 2.44x | 2.52 | 885 |

**150 tracks, 44 MB, full writer (PDB + OneLibrary included)**, concurrent vs serial: tmpfs 1/2/3 sticks 0.77/0.77/0.77 s concurrent vs 0.77/1.53/2.32 s serial (1.0x/2.0x/3.0x); disk 1.02/1.27/1.29 s vs 1.28/2.05/3.32 s (1.25x/1.61x/2.57x).

Findings:
- Three sticks take 1.5x as long as one on tmpfs (1000 tracks) and 1.6x on disk, against 3x serial. The loss on disk is the shared page cache and device queue; real sticks are separate devices, so the real gain should be closer to the tmpfs rows, but that is unverified.
- CPU is not the limit: 3 sticks use about 1.8 cores on a 4-core box, and cpu/wall reaches 1.8-2.1.
- Reading once: `prepare_multi_sync` for 1000 tracks and 3 targets takes 54 ms. The audio file itself is still read once per stick by `std::fs::copy` inside the writer; the second and third read come from the page cache. A true single read needs a writer change (proposals).
- Where time goes with 1000 tracks: plain audio copy 0.37 s (tmpfs) of 0.58 s; the rest is ANLZ writing. With the full writer at 150 tracks, 0.77 s total against about 0.05 s of audio copy, so databases and ANLZ dominate on a fast disk, not copying.
- FAT-close effect: not measurable without FAT. Arithmetic from cyanbox (about 0.1 s per close on FAT): a stick takes 3 file closes per track (audio, DAT, EXT) plus artwork, so 1000 tracks is at least 3000 closes, around 300 s serial per stick. Syncing sticks concurrently overlaps these waits across devices, which is the main reason this layer exists. Within one stick, a close pool would cut that by up to 16x (cyanbox: 200 files from about 20 s to about 1 s). This is the largest likely win, but only a real stick can confirm it (needs a Mac or Linux run with a FAT stick).

### Writer bug found while benchmarking (blocks libraries over about 200 tracks)

`export.pdb` creation panics for roughly 200 or more tracks: `pdb.rs:170 slice index starts at 65360 but ends at 168`. `split_into_pages` takes `offsets: &[u16]` for a whole table's heap, so the offsets wrap once the tracks heap passes 64 KiB (about 200 rows at 330 bytes with my short synthetic tags; real tags are longer, so likely earlier). 100 tracks pass, 200 and 300 fail on all single-stick runs, so this is independent of concurrency (`write_usb` and `sync_usb` call the same code). In `write_usb` the panic aborts the command; in `sync_usb_many` it is contained as a failed stick report ("Sync thread panicked."). Fix belongs in the brother's lane: track offsets as `usize` (or per-page) and rebase only when building each chunk. This also means the bench's 300-1000 track runs use `BENCH_WRITER=files`, which skips the databases; the full writer was benchmarked at 150 tracks.

## Proposed writer changes (for morizkraemer; not made)

1. **Fix the u16 offset overflow in `pdb.rs::split_into_pages`** (above). Highest priority; real libraries are well over 200 tracks.
2. **Close/finalise thread pool for small files** (cyanbox, BSD-2, `src/export/export.c`): hand finished ANLZ `.DAT`/`.EXT`/artwork files and audio file closes to a pool of about 16 threads. Expected: FAT stick time for 1000 tracks from minutes to tens of seconds. Needs a measurement on a real FAT stick before and after; the numbers above are the harness for it.
3. **Progress callback**: `sync_usb_with(…, &dyn Fn(SyncStep))` with steps copying (track index), writing-database, cleaning-up. Replaces the file-count watcher and gives the spec's exact `current/total/message`.
4. **Read once, write many**: an API that takes one open audio source and several destination roots, or a prepared byte buffer, so a track is read once for N sticks. Gain is small while the page cache holds, larger for big libraries that exceed RAM or slow source disks.
5. **Return the planned bytes**: expose the sync plan (adds, sizes, removals) before writing, so the free-space check can credit removals and the UI's dry run (`plan_usb_sync`) uses the same code, as the spec asks.
6. **Cancel flag** checked between tracks, and an ownership manifest (spec section "Ownership tracking") so removal only deletes files fourfour wrote.

## What the UI will need (not built)

- Call `sync_usb_many` with one `SyncTarget` per ticked stick; listen to `usb-sync-progress` keyed by `volume` and `usb-sync-stick-report`. Because `volume` is the folder name, two sticks with the same label would be indistinguishable in events; the spec's open question 3 (stable id from `get_mounted_volumes`) now matters. Quick fix on the backend if wanted: add the full `path` to the progress payload.
- A per-stick state machine: idle, checking space, copying `done/total`, writing database, done (Safe to eject) or failed (message, retry). Failed sticks must not block the others' "Safe to eject".
- A combined "Sync all" button and an overall progress summary; the plan dialog per stick still needs `plan_usb_sync` (not built).
- Show `skipped_unanalyzed` per stick with "Analyze first".
- Disable Eject/Wipe on a stick while it syncs (the engine rejects a second sync on the same root, but not wipe).
- `capabilities`: add a `multi_sync` flag in `usb_engine_capabilities` when that command is added.

## Reuse notes

- rbxport `sync_devices`: ideas only (GPL): build once, per-stick threads, failures isolated, duplicate roots rejected. No code copied.
- cyanbox pool and encode/write pipeline (BSD-2): not copied; proposed above. Attribution is needed when it is.
- baken `volume.rs` free-space check (MIT): not copied; `statvfs` is about ten lines.

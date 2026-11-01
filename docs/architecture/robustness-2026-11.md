# Import and analysis robustness, November 2026

Beta readiness (`r12-robust`): strangers will point the app at messy music folders. Goal: import, analysis and the library store must not crash, hang or poison the library on bad input, and good files next to a bad one still import and analyse. Branch `r12-robust` on `ov-1028`.

## Tests

Fixtures are generated at test time (nothing copied from real music) by `fourfour-analysis-rs/tests/messy_fixtures/mod.rs`: a 6 s kick loop as WAV and AIFF (hand written) and MP3, FLAC, M4A (ffmpeg when installed; those cases are skipped without it), then damaged copies. Tests report per-case outcomes with `--nocapture`.

| Test file | What it drives |
|---|---|
| `fourfour-analysis-rs/tests/messy_audio.rs` | `decode_mono` and the MP3 frame scan on about 65 damaged files; with weights (`FOURFOUR_MODELS_DIR`, `--include-ignored`) also the full `Analyzer` |
| `pioneer-test-ui/src/robustness_tests.rs` | The real pipeline: `survey_files` -> `scan_paths` -> `plan_for_library` -> add to a file-backed library -> `run_with_runner` analysis -> track list build. Real `Analyzer` when weights are found, a decode-and-waveform engine otherwise |
| `pioneer-library/tests/robust_store.rs` | Library rows with extreme values, duplicate index with odd text, relocation in looped folders, damaged `library.db`, export of awkward paths (ignored repro) |

## Cases

Every case also checks: good neighbours imported and fully analysed, analysis run finished with `errors == failed rows`, track list and duplicate candidates still build, and the walk and the run end inside a time budget.

| Case | Before | After |
|---|---|---|
| Zero-byte and 1-byte files, all 7 extensions | skipped, counted unreadable | same |
| Truncated (header only, 20/40/90%), corrupted middle or start, random bytes, MP3/FLAC/WAV/AIFF/M4A | decoded partly (flagged) or error row, run goes on | same |
| Wrong extension (WAV named .mp3/.flac/.m4a, MP3 named .wav, text named .wav), unsupported (.ogg, .opus, .wma, .jpg, .txt, no extension) | imported when it decodes, else error; unsupported counted | same |
| Huge or odd tags: 3 MB titles, bad UTF-8, emoji, RTL override, NUL, lone UTF-16 surrogate, lying frame and chunk sizes, garbage and huge-claim covers | 3 MB title reached the library (and every list payload) | text clamped to 500 chars, NUL removed, `usb_path` artist folder follows; see "Not fixed" for tags the reader rejects |
| Unicode, emoji, RTL, combining and 240-byte names; 3.5 kB deep path; tabs, newlines, quotes | imports and analyses | same |
| Non-UTF-8 file name | row stored with a lossy `U+FFFD` path, never analysable, not reported | skipped, counted unreadable |
| Symlink loops (self, parent, mutual, dangling, 45-deep file chain, dropped loop) | walk ends | same |
| Folders that link to each other (8 folders, every one links to the other 7) | **109,600 paths in 35 s**; 10 folders would be millions | 16 paths, instant |
| `chmod 000` file | skipped, counted unreadable | same |
| `chmod 000` and no-list folders | silently dropped, nothing reported | counted unreadable |
| Read-only folders | imports, analyses, nothing written into them | same |
| File deleted between survey and scan | skipped, counted unreadable | same |
| File deleted or replaced by garbage between import and analysis | fails alone, health check says missing | same |
| Track removed from the library while queued for analysis | **whole run aborted** with "Track not found for analysis storage"; later tracks never analysed | that track fails, run finishes |
| A panic in the tag reader | would end the whole import (a scan thread panics, `read_parallel` panics with it) | that file is skipped |
| Zero sample rate WAV through the waveform decoders (symphonia 0.5) | **panic** in `TimeBase::new` while probing | error |
| Duplicates: same bytes under two names, hard link, symlink to file, one file dropped three times, re-import of a folder | one row per path; copies of library tracks flagged | same |
| One-file folder, zero-audio folder, empty folder, missing path, hidden-only folder, nothing dropped | clean plan, no analysis run | same |
| 3 GB sparse junk `.mp3` | bounded but slow: 18-35 s in the tag reader (debug build), 14 s in decode | not fixed, see below |
| FIFO and `/dev/zero` symlinks named `.wav`/`.mp3` | skipped (not regular files) | same |
| Damaged cover images (garbage, cut PNG, 65535 x 65535 header) | thumbnail falls back to the raw bytes | same |
| Library rows: 3 MB title, NUL, empty title, zero sample rate, `u64::MAX` size, 10 h duration | stored and read back | same |
| Batch with one repeated path | whole batch rejected, nothing half added | same |
| Damaged `library.db` | `open` returns an error, file untouched; half a database does not panic | same |

## Fixes

- `pioneer-test-ui/src/import_plan.rs`: scan survey counts unreadable folders and non-UTF-8 audio paths as unreadable (existing "N audio files couldn't be read" line); symlinked folders are followed at most once per drop; tag reads run under `catch_unwind` and their text is clamped.
- `pioneer-test-ui/src/analysis_run.rs`: a result that cannot be stored (track removed meanwhile, lock poisoned) is that track's failure, not the run's.
- `pioneer-test-ui/src/band_analysis.rs`: symphonia probe and codec construction run under `catch_unwind`.
- `run_with_runner` is `pub(crate)` for the tests.

## Not fixed

- **USB export, writer lane, repro `export_keeps_every_track_when_artist_and_file_name_repeat` (ignored in `robust_store.rs`):** `usb_path` is `/Contents/<artist>/<file name>`, so two tracks with the same artist and file name (any untagged `01 Intro.mp3` from two folders) share one path. The stick gets one audio file and one ANLZ folder for both tracks; one of them plays the other's audio. Fix belongs in `scanner::read_track_metadata` or the writer (unique suffix).
- **Tags the reader rejects drop the file:** lofty fails on invalid UTF-8 in an ID3 UTF-8 frame, lone UTF-16 surrogates and lying tag sizes, so `read_track_metadata` errors and the file is skipped as unreadable although the audio plays. Suggest `ParsingMode::Relaxed` or a properties-only retry in `scanner.rs` (writer lane, read-only here).
- **Huge junk files:** the tag reader and decoder scan an MP3 with no valid frames to its end (about 10 s per GB). Bounded by file size; needs a scan cap in lofty/symphonia use.
- **Analysis failures are not stored** (no schema field): a file that cannot be decoded is retried at every launch while "analyse new tracks" is on. Cheap for undecodable files; needs a column to remember it.
- **Library open recovery:** a library that fails to open leaves the app running with an empty list and the error in the status bar and Settings > Advanced (see "Library open safety"). There is no screen to pick another library, restore a backup or start fresh from that state yet; that is a product call.
- **Duplicates inside one drop are not flagged** (only against the library), and symlinked folders or files import as separate rows under their link paths. Aliases of the same file count twice.
- `normalise` path keys fold case, so on a case-sensitive volume `A.wav` next to `a.wav` counts as already in the library.
- A WAV cut to 20% analyses as complete with BPM 0; the decoder's `truncated` flag is not shown anywhere.

## Library open safety

Branch `r27-dbsafe` from `ov-1028`. Tests: `cargo test -p pioneer-library --test robust_store` (`a_library_*`, `an_unreadable_version_*`, `a_damaged_header_*`, `a_locked_library_*`, `a_database_file_with_no_tables_*`) and `cargo test -p pioneer-test-ui --bin pioneer-test-ui startup_`.

| Case | Before | After |
|---|---|---|
| `schema_version` row deleted, tracks present | opened, stamped as v7 without running migrations | error, nothing written, tracks untouched |
| `schema_version` table dropped, tracks present | opened, created the table, stamped v7 | error, no table created |
| Version stored as text (`'garbage'`) | opened, inserted a second version row | error, the one row stays |
| Version newer than the app (99) | error "Unsupported schema version 99 (expected 7)" | error "saved by a newer version of the app ... update the app to open it"; not downgraded |
| Damaged header (first 16 bytes) | error, file untouched | same |
| Locked by another process (`locking_mode=EXCLUSIVE`) | error by luck (the init DDL also hit the lock) | error from the version read; opens once the lock is gone |
| Empty file or a database with no tables | fresh library | same; the fresh schema is now created in one transaction, so a crash mid-init cannot leave tables without a version row |
| Startup with a damaged `library.db` | `expect` panics in `setup`, no window | window opens, `load_state` returns "Couldn't open the library at <path>: <reason>", status bar shows "Failed to load library", the full text is in Settings > Advanced; the file is not touched and no side files appear |

`schema::stored_version` returns `None` only when the database has no tables at all; tables without a readable version is an error. In the failed state the app does not manage `SharedLibrary`, so every library command (import, analysis, sync, backup restore, reset, change library) returns Tauri's "state not managed" error instead of acting; none of them can write to or delete the damaged file. The file health check and cover fill do not start.

Left for a recovery screen (Merlin's call): offering "choose another library", "restore a backup" and "start a new library (keep the old file aside)" from the failed state, and a clearer message than "Failed to load library" in the status bar. `change_library_path` and `restore_backup` need the library state today, so they would need a small change to work from the failed state.

## Run

`cargo test -p pioneer-test-ui --bin pioneer-test-ui robustness -- --nocapture` (about 15 s with the decode engine, 5 min with the real analyzer and `FOURFOUR_MODELS_DIR=~/.cache/fourfour-models`); `cargo test -p fourfour-analysis-rs --test messy_audio -- --include-ignored` with weights; `cargo test -p pioneer-library --test robust_store`. Permission cases skip as root.

## Platforms 2026-11

Branch `r20-winlinux` from `ov-1028`. Linux `dev` box (Debian 13, headless, load 45-60), 2026-11-01.

**Linux bundle** (`fourfour-analysis-rs/scripts/bundle-app.sh --ci --bundles deb,appimage`, tauri-cli 2.10.1, weights from the local model cache): built in about 25 min at load 50 (cold target dir).

| Artifact | Size |
|---|---:|
| `fourfour_0.1.0_amd64.deb` | 116.4 MB |
| `fourfour_0.1.0_amd64.AppImage` | 216.1 MB |
| inside the deb: `pioneer-test-ui` / `beat_this.onnx` | 45 MB / 81 MB |

The weights land in `/usr/lib/fourfour/models/` (deb and AppImage), which is Tauri's `resource_dir` on Linux, so the runtime lookup (`locate_models_in`) finds them. `.deb` Depends is only `libwebkit2gtk-4.1-0, libgtk-3-0`.

**Smoke** (deb extracted with `dpkg-deb -x` into a scratch dir, `HOME` and `XDG_*` in a temp dir, 50-track synthetic library made with the bundled `bench_startup gen`, `xvfb-run`): the app opened the library, `load_state` returned 50 tracks, first row at 10.5 s and interactive at 19.7 s on the loaded box (software GL), still alive at 25 s, then SIGTERM (exit 143; the app has no headless quit, so there is no exit 0 to check). stderr has only AT-SPI and ALSA "no card" lines plus `audio output unavailable at startup, will retry on play`, as expected without a sound card. The AppImage was run only for `--self-check` (`APPIMAGE_EXTRACT_AND_RUN=1`), not as a window.

**Fixed here**

- `--self-check` looked for the weights in the macOS layout (`../Resources/models`) and failed on Linux; it now uses `../lib/fourfour/models` on Linux and `<exe dir>/models` on Windows. Verified on the extracted deb: `self-check ok ... weights loaded`. The Windows path is not verified (NSIS layout assumed from Tauri's resource rules).
- Tests that use `std::os::unix` (permissions, symlinks, fifos) are gated with `cfg(unix)` (`pioneer-library/tests/multi_sync.rs`, `robust_store.rs`, `robustness_tests` in `pioneer-test-ui`, `make_tree_writable` in `messy_fixtures`), so `cargo check --workspace --all-targets` passes for Windows.

**Windows (compile check only, no run)**: `x86_64-pc-windows-gnu` was added with `rustup target add` (the MSVC target also installs, but the C parts need a Windows C toolchain). No mingw on the box, so the C and resource steps were stood in for: `zig cc -target x86_64-windows-gnu` for the C and C++ build scripts (stretch), the sqlcipher build script skipped through a `--config` override with the pre-generated bindings (it needs Windows OpenSSL headers), and a no-op `windres`. With that, `cargo check --workspace --all-targets` is clean for all five crates (one dead-code warning, `prioritize_with_runner`, shared with Linux). That proves the Rust compiles for Windows, not that it links, bundles with NSIS, or runs; OpenSSL and the sqlcipher C build on Windows are exercised only by the CI Windows job.

**Runtime risks found**

- (fixed, r25) The `.deb` shipped the dev binaries `bench_analysis`, `bench_startup`, `waveform_*` in `/usr/bin` next to the app; the bundler picks up every bin of the crate.
- (fixed, r25) `.deb` had no `libasound2` Depends although cpal links ALSA; on a machine without it the app fails to start.
- (fixed, r25: `GetDriveTypeW == DRIVE_REMOVABLE`) Windows `is_removable` returned true for every drive, so internal data drives show up as USB sticks (the system drive is refused, other fixed drives are not; no home-overlap check on Windows).
- (fixed, r25: `udisksctl unmount`, `umount` fallback on Linux; clear error on Windows) `eject_volume` ran `umount` on Linux (needs root for most mounts; `udisksctl unmount` is the user-level call) and also on Windows, where it does not exist, so eject always errors there.
- `reveal_in_finder` on Linux opens the parent folder through `xdg-open` (no selection); fine, but absent in minimal installs.
- `rekordbox_default_master_db` only checks `~/Library/Pioneer/...` and `%APPDATA%\Pioneer\...`; a Linux home never matches (no Rekordbox there, so harmless).
- (fixed, r25: `pathBaseName` in `app/src/lib/pathName.ts`) Frontend `path.split('/').pop()` without backslash normalising: `app/src/services/tauri.svelte.ts` lines about 524 and 964 (dropped track title, stick label). On Windows the full path shows as the name; other label helpers already normalise.
- Writer lane, not touched: `pioneer-usb-writer/src/writer/pdb.rs` matches the extension case-sensitively (`Some("mp3")`), so `TRACK.MP3` gets file type 0 in the legacy PDB (the scanner and `onelibrary.rs` lowercase first). Windows users often have upper-case extensions.
- Writer lane: `usb_path` is built from artist and file name (known repeat bug above); on Windows or exFAT sticks the reserved names (`CON`, `NUL`) and trailing dots would also matter, not checked.
- Already handled, checked: `/Volumes` is macOS-only behind `cfg`; Linux volume detection reads `/proc/mounts`; Windows lists drive letters; the audio extension filters in the app lowercase before matching; symlink and `statvfs` code is `cfg(unix)` with a stub for others (`statvfs_available_bytes` is a stub returning unlimited space on Windows, so the free-space guard in multi-USB sync does nothing there; `volume_capacity` is unix-only).

**Fixed in r25-package**

- Dev binaries: all eight `[[bin]]` targets other than the app (`bench_*`, `waveform_*`) have `required-features = ["dev-bins"]` and `autobins = false` in `pioneer-test-ui/Cargo.toml`, so `cargo tauri build` produces only `pioneer-test-ui` for the deb, AppImage, `.app` and NSIS. Dev use: `cargo build --release --features pioneer-test-ui/dev-bins --bin bench_startup`. `scripts/perf.sh` and `waveform-dev` pass the feature; `perf-night.sh` gets `bench_analysis` from the `perf.sh` build in the same target dir (it no longer comes from the bundle build); `check-dmg.sh` only used the app binary. Old doc commands with a bare `--bin bench_startup` now fail with cargo's "requires the features: dev-bins" message. `cargo check --workspace --all-targets` skips those bins unless the feature is given.
- Deb Depends: `libasound2t64 | libasound2` and `libssl3t64 | libssl3` (sqlcipher links `libcrypto` dynamically, `libsqlite3-sys` `bundled-sqlcipher` without vendored OpenSSL), on top of Tauri's webkit and gtk entries. Config only: no deb was rebuilt, no `ldd` of a release binary was available on the box, so the list comes from the crates (cpal, libsqlite3-sys build script) and is not proven by `dpkg-deb -I`.
- Windows `is_removable` now asks `GetDriveTypeW` (removable drives only; USB hard disks that report as fixed are no longer listed). Not compiled for Windows here.
- Linux eject unmounts through `udisksctl unmount --block-device <dev>` (device from `/proc/mounts`), falling back to `umount` if `udisksctl` is missing; Windows returns an explicit "not supported" error instead of running `umount`.
- Frontend: `pathBaseName` handles `\` and `/` and trailing separators; used at the three spots in `tauri.svelte.ts` (mock dropped track title, mock stick id, capacity volume label); unit tests in `pathName.test.ts`.

**For the writer/sync lane (not touched)**

- `pioneer-library/src/multi_sync.rs:696` `statvfs_available_bytes` Windows stub returns unlimited space, and `:721` `volume_capacity` is a stub on non-unix: the multi-USB free-space guard does nothing on Windows (suggest `GetDiskFreeSpaceExW`).
- `pioneer-usb-writer/src/writer/pdb.rs` case-sensitive extension match (`Some("mp3")`), see above.
- `usb_path` reserved Windows names and trailing dots, see above.

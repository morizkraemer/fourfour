# Platforms 2026-11 (xplat)

Branch `xplat` from master (eb236d3), dev box (Debian 13), 2026-11-21. Builds on the r20 notes in `robustness-2026-11.md` ("Platforms 2026-11").

## Linux

| Check | Result |
|---|---|
| `cargo test --workspace` | 649 passed, 0 failed, 24 ignored before the new tests; 652 passed, 0 failed, 25 ignored after |
| `cargo check -p pioneer-test-ui --features dev-bins --bins` | clean (11 warnings, dead code) |
| `flock ... cargo build -p pioneer-test-ui --release` | finished in 5 min 12 s |
| `tauri build` bundle | not run: verified in r28 (deb, `--self-check`), nothing here touches that path |

No Linux-only failures found.

## Windows (compile checks, no linking)

Targets installed: `x86_64-pc-windows-gnu`, `x86_64-pc-windows-msvc`. No mingw, no MSVC, no xwin, no clang on the box.

| Command | Result |
|---|---|
| `cargo check -p pioneer-usb-writer --target x86_64-pc-windows-msvc` (plain) | stops in `libsqlite3-sys`: cc-rs wants `lib.exe` |
| same with `--target x86_64-pc-windows-gnu` (plain) | stops in `libsqlite3-sys`: cc-rs wants `x86_64-w64-mingw32-gcc` |
| gnu, with stand-ins (below), `cargo check --workspace --all-targets` | clean after the fix below; 11 dead-code/unused warnings, same class as Linux |
| msvc, same stand-ins | not possible: `zig cc -target x86_64-windows-msvc` has no MSVC headers (`intrin.h` chain fails on sqlite3.c) |

Stand-ins used (all in the scratchpad, nothing installed): `CC`/`CXX` = `zig cc`/`zig c++ -target x86_64-windows-gnu` wrappers that drop cc-rs's `--target=` flag; `AR` = `zig ar`; `CFLAGS=-I<copy of the host openssl headers>` so the SQLCipher amalgamation finds `openssl/crypto.h`; a no-op `x86_64-w64-mingw32-windres` on `PATH` for `tauri-winres`.

Which crates need a C toolchain and why:

- `pioneer-usb-writer` and everything depending on it (`pioneer-library`, `fourfour-analysis-rs`, `pioneer-test-ui`): `libsqlite3-sys` with `bundled-sqlcipher` compiles the SQLCipher amalgamation and needs OpenSSL headers and `libcrypto` for the target.
- `fourfour-stretch`: `cc` builds the Signalsmith C++ wrapper.
- `pioneer-test-ui`: `tauri-winres` needs `windres` (gnu) or `rc.exe` (msvc) in its build script.
- `ort` (feature, macOS/CoreML path only) is not built on Windows by default.

### Fixed

- `pioneer-test-ui/src/waveform_detail.rs`: `free_space_of` used `std::os::unix` and `libc::statvfs` unconditionally, so the Windows build failed to compile. Now `cfg(unix)` as before plus a `cfg(windows)` version using `GetDiskFreeSpaceExW` (declared like `GetDriveTypeW` in `volumes.rs`, no new crate). Unit test: free space of the temp dir is known and positive, a missing folder gives `None` (runs on Linux, covers the shared contract).
- `pioneer-test-ui/src/main.rs` test module: `use super::eject_failure` and the test that calls it did not compile on Windows (`eject_failure` is `cfg(not(windows))`), so `cargo test --workspace` in the CI Windows job could never build. Test-only `cfg(not(target_os = "windows"))` on the import and the test; the eject code itself is untouched. This sits in Mo's eject area: drop the two attributes if Mo prefers to own it, but then the Windows CI job fails to compile its tests.
- `pioneer-test-ui/src/import_plan.rs` `is_hidden`: also skips `$RECYCLE.BIN` and `System Volume Information` (case-insensitive), which sit at the root of Windows-formatted drives and are unreadable. Test added.
- `.gitattributes`: `*.sh text eol=lf` (see Open item 4).
- `pioneer-library/src/rekordbox/remap.rs`: test that a `C:\Music` prefix maps `C:/Music/...` and `c:\music\...` onto a POSIX prefix and not `D:`.

## Audit

Already correct, no change:

- Extensions in scan and import are compared case-insensitively (`scanner.rs`, `import_plan::file_kind`, `mp3.rs`); symphonia's `Hint` extension is case-insensitive.
- NFC/NFD: `duplicates.rs` and `rekordbox/remap.rs` fold to NFC plus lowercase; the remap and the case-insensitive resolver have NFD tests.
- Rekordbox XML `Location`: `file://localhost/C:/...` drive letters handled in `location_to_path`, with a test.
- `canonicalize` is only used for symlink-loop detection in import; stored paths come straight from `read_dir`, so no `\\?\` prefixes leak into `source_path`.
- `HOME`, `Library/...` uses: `app_log.rs` has Windows and Linux branches; `rekordbox_default_master_db` tries `APPDATA`; the CoreML cache dir (`ort_beat.rs`) is `cfg(macos)` only; `heap.rs` is glibc-gated; `analysis_priority.rs` has unix/linux/macos branches. No `std::env::home_dir`.
- `dev_bin/*` and `waveform_*` binaries hardcode `HOME` and `~/Library/Pioneer`: dev tools, macOS-only by intent, not bundled.

Open, not changed (need a decision or are not ours; item 4 was fixed):

1. Library `tracks.source_path` is `UNIQUE` on exact bytes. The same file reached through an NFC and an NFD spelling (HFS+/exFAT sticks, a Windows library synced to a Mac) becomes two tracks. Content-level `duplicates.rs` catches them after the fact. Fixing it means normalising the key, which is a schema/library DB decision (must ask).
2. `tauri.conf.json` `beforeDevCommand` runs `sh -c 'pkill -f ...'`: `tauri dev` on Windows will not work; `tauri build` is unaffected. Dev-only.
3. Max path length: Rust std adds the `\\?\` prefix for long absolute paths in `fs` calls, but the Tauri exe has no `longPathAware` manifest and Explorer/`explorer /select,` still limit to 260. Not tested (no Windows).
4. Line endings: nothing in our code writes text files that Windows tools read except the Q62 playlist exports (see below). A Windows checkout with the runner default `core.autocrlf=true` would turn `scripts/*.sh` into CRLF and break `bundle-app.sh` in the CI Windows job. Fixed here: `.gitattributes` with `*.sh text eol=lf`.
5. File locking: SQLite does its own locking; no single-instance guard, so two app instances share one library DB. Not Windows-specific.

### Mo's lane, repro only

- `pioneer-usb-writer/tests/xplat_repro.rs` `pdb_file_type_ignores_extension_case` (`#[ignore]` with reason): `write_pdb` matches `Some("mp3")` etc. case-sensitively, so a `Track.MP3` source gets `file_type` 0x00 while the scanner and `onelibrary.rs` (`to_lowercase`) accept it. Fails today when run with `--include-ignored`; fix is a lowercase of the extension in `pdb.rs`.
- `multi_sync.rs` `statvfs_available_bytes` returns `u64::MAX` on non-unix (never reports a full stick on Windows); `volume_capacity` has a non-unix branch too. Read-only for me, noted for Mo.
- `eject_volume` on Windows returns an error string ("not supported yet"). Known (STATE brother's lane).

### Branch drain/Q62-20261121 (playlist export, not edited)

- M3U8 and text exports write `\n`, CSV `\r\n`; fine for Windows players.
- Rekordbox XML: `path.replace('\\', "/")` then `file://localhost/` + percent-encoding; `C:\Music\a.mp3` gives `file://localhost/C:/Music/a.mp3`, matching what `location_to_path` reads back. UNC paths (`\\server\share\a.mp3`) become `file://localhost//server/share/a.mp3`; untested against Rekordbox.
- Traktor NML does the same backslash replacement; drive letter handling for `VOLUME`/`DIR` was not checked against Traktor.

## CI

`.github/workflows/ci.yml` and `docs/process/ci.md` come from `r63-ci-v3` (f0827da) unchanged except `--locked` on the cargo build, test, check and notices steps.

Re-checked against the current tree: five jobs (linux, notices, webkit-smoke, macos, windows); all referenced scripts exist (`bundle-app.sh`, `fetch-models.sh`, `scripts/mac/build-dmg.sh`, `check-dmg.sh`, `scripts/release/third-party-notices.py`, `THIRD-PARTY-NOTICES.txt`); npm scripts `test`, `check`, `build`, `smoke` exist; Playwright config reads `FF_SMOKE_*`; tauri 2.10.3 in `Cargo.lock` matches `TAURI_CLI_VERSION ~2.10`; Node 22 matches dev (22.23); workspace has the same five crates (`cargo ... --workspace` needs no edit); deb `Depends` in `tauri.conf.json` matches the greps. YAML parses (PyYAML). `actionlint` is not installed, so no schema lint; read by hand instead. Never pushed or triggered.

Windows job risk added by this audit: the test module compile error above (fixed here). The remaining Windows risks are the ones `ci.md` already lists (MSVC link of sqlcipher/stretch, libcrypto location, NSIS paths).

## First CI run (draft PR 40)

Runs 44882486110, 44885111038, 44885693770 (xplat 5b8bdd5 to 8adf2de). Fixed and pushed: notices regenerated with the repo script; Windows weights test accepted only the Unix error text; vendored OpenSSL on Windows (Strawberry Perl via `OPENSSL_SRC_PERL`, NASM on PATH, choco OpenSSL step removed); `macos-14` is not offered to the repo (job cancelled in 1 s), now `macos-15`; chromium smoke races (`player-empty` readback warning, palette typing, menu arrows). Webkit smoke passes.

Committed after 8adf2de, not pushed (push budget used): `ffmpeg` install on all three OS (the robustness fixtures encode mp3 and flac with it; `wrong_extension_and_unsupported_files` failed on Linux and macOS without it); `CaseInsensitiveResolver` now returns the on-disk spelling (on case-insensitive filesystems `exists()` is true for any spelling, so a Rekordbox import could add a second track next to one stored with other casing, the Windows failure of `existing_tracks_keep_user_edits...`); test fixes for path separators, deep path length on macOS (PATH_MAX 1024) and the OS-specific missing-file message.

Open, not fixed: `tempo_tests::extreme_ratios_stay_finite_bounded_and_on_pitch` is 0.25 to 0.56 cents over the 5 cent limit on the macOS (arm64) and Windows runners and passes on Linux; Windows `volumes::tests::root_is_refused` and `stick_sync::capacity_of_a_fake_volume` (Mo's lane); Windows `playback::tests::channels_stay_paired_through_underruns` checked 0 frames; the Linux deb steps and the Windows NSIS bundle were never reached.

### Round 2 decisions

- `extreme_ratios_stay_finite_bounded_and_on_pitch`: pitch limit raised from 5 to 6 cents (orchestrator decision; measured worst 5.56 on macOS arm64, 5.25 on Windows x64, inaudible); the numbers are in the assertion message.
- `capacity_of_a_fake_volume` and `root_is_refused` are `#[cfg_attr(windows, ignore)]` with the reason pointing to issue #39; Mo's code is unchanged.
- `channels_stay_paired_through_underruns` uses the fake audio backend (no device involved). On the Windows runner the render loop ran before the decoder thread had filled the ring, so every frame was an underrun and 0 frames were checked. The loop now sleeps 1 ms after an all-silent render; the engine is not touched.

## Windows libcrypto (r65)

SQLCipher links `libcrypto` dynamically on Windows, so an installed app needs `libcrypto-3-x64.dll`. Option, not applied because it adds a Cargo.lock entry and cannot be verified without Windows:

```toml
# pioneer-usb-writer/Cargo.toml
[target.'cfg(windows)'.dependencies]
rusqlite = { version = "0.33", features = ["bundled-sqlcipher-vendored-openssl"] }
```

Checked here: `cargo metadata --offline` resolves it, and the only lock change is a new `openssl-src 300.6.1+3.6.3` package (already in the local registry; `cc` is already locked). Linux and macOS builds do not compile it. Costs: Windows build needs Perl (and NASM or a slower no-asm build) to compile OpenSSL, +5-10 min on the CI job; the CI step "OpenSSL for sqlcipher" and its `OPENSSL_*` variables become unnecessary (set `OPENSSL_NO_VENDOR` is not needed, vendored wins when the feature is on); Windows releases must add OpenSSL's Apache-2.0 notice to `THIRD-PARTY-NOTICES.txt` (the generator filters on `aarch64-apple-darwin`, so it will not list it by itself). Alternative with no lock change: ship `libcrypto-3-x64.dll` from the runner's OpenSSL via a Tauri bundle resource (needs a Windows machine to verify).

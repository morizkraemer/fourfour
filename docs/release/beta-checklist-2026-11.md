# Public macOS beta checklist (2026-11-02)

Goal (STATE.md "Ship target"): a `.dmg` strangers can install, that scans a folder, analyzes it in Rust and exports a USB that plays on a CDJ-3000.

Base: `ov-1028` (cf422ca). Evidence is file:line on that branch, doc links, branch names and "Waiting" numbers from `docs/STATE.md` on `master` (ddcb8c5; the STATE.md on `ov-1028` is older and lacks items 15-37). Docs-only run: nothing was built, run or installed. Claims about a signed build or a real Mac are marked "untested".

Status words: **ready** (nothing to do), **needs Merlin** (decision, Apple/GitHub account, listening or visual check), **needs brother** (morizkraemer: writer, sync, hardware), **needs work** (an agent can do it, no decision needed).

## Summary

| # | Area | Status | Next step | Owner |
|---|---|---|---|---|
| 1 | App identity (bundle id, name, trademark) | needs Merlin | Pick neutral id and public name (Waiting 24) | Merlin |
| 2 | Data location and migration | needs work | One-time copy of old app data to the new id | agent, after 1 |
| 3 | Signing, hardened runtime, notarization | needs Merlin | Apple Developer account and Developer ID cert, then wire Tauri signing | Merlin, then agent |
| 4 | Entitlements | needs work | Start with none; test a signed build on the Mac | agent, Mac night |
| 5 | Gatekeeper first run, min macOS, arch | ready (unsigned) | Add "Apple Silicon, macOS 13.4+" to the install note | agent |
| 6 | Licences and NOTICE | needs work | Add LICENSE, full third-party notice, fix About credits, drop Python fallback | agent; licence choice Merlin |
| 7 | Key model licence and quality | needs Merlin | Accept 56% clean model for beta, or hold (Waiting 4, 30) | Merlin |
| 8 | Privacy | ready | No network at run time in our code; state it after a Mac check | agent |
| 9 | Crash reporting, logs | partly done (r48) | File log and panic hook landed; still needed: "Copy diagnostics" and "Reveal logs" UI, Merlin confirms no-upload | agent |
| 10 | Auto-update | ready (none) | Confirm: no updater in beta 1 | Merlin |
| 11 | Backup and restore | needs work | Land r38 recovery panel (Waiting 37); migration test | Merlin (land), agent |
| 12 | USB export safety | needs brother | CDJ-3000 run of current export; manifest-aware wipe | brother |
| 13 | Analysis quality bars | needs Merlin | Bar-1 listen check (Waiting 16); key call (7) | Merlin |
| 14 | Open Waiting items touching beta | needs Merlin | Land parked -v2 branches (33-37) after one Mac check | Merlin |
| 15 | CI | needs Merlin | OK to push `r35-ci` for a first run (Waiting 15) | Merlin |
| 16 | User docs (README, install, FAQ) | needs work | Write user README and FAQ | agent |
| 17 | Support channel, release hosting | needs Merlin | Pick tracker, contact, dmg host, repo visibility | Merlin |

## Top 5 blockers

1. Bundle id `com.pioneer.test-ui` and "Pioneer" naming (Waiting 24), plus the data migration that follows from changing it.
2. No Apple signing or notarization: strangers hit the Gatekeeper wall. Needs a paid Apple Developer account.
3. Analysis bars: key 56.3% clean vs 60% bar (Waiting 4, 30); bar-1 listen check still open (Waiting 16).
4. CDJ-3000 never saw the current export (cue names and colours, multi-page PDB, folders, 3-band waveforms: Waiting 0, 12; `docs/architecture/cdj-checklist-2026-10.md`), and wipe is not manifest-aware.
5. No LICENSE file, incomplete third-party notice and About credits, no first CI run (Waiting 15).

---

## 1. App identity - needs Merlin

Evidence:
- `pioneer-test-ui/tauri.conf.json:3` identifier `com.pioneer.test-ui`; `:2` productName `fourfour`. The Cargo package and binary names (`pioneer-test-ui`, in the r35-ci deb check `usr/bin/pioneer-test-ui`) and crate names (`pioneer-usb-writer`, `pioneer-library`) still say "pioneer".
- "Pioneer" is a trademark (Waiting 24). Naming the format in text ("Pioneer CDJ-compatible", "Wipe all Pioneer library contents" at `app/src/menus/context-menus.ts:162`, import copy at `app/src/modules/settings/LibraryPage.svelte:58`) is a different question from using it in the bundle id, crate names or product name. Not legal advice; Merlin decides whether to ask someone.
- Rekordbox (AlphaTheta) and CDJ are also third-party marks used in copy and in the import feature. Same question.
- Working title "fourfour" is temporary (memory: no name-based icons or branding). `app/src/modules/settings/AboutPage.svelte:11-13` shows "fourfour" and the tagline "Sync, play and organize your DJ library."
- `tauri.conf.json` sets no `bundle.category`, `copyright`, `publisher` or `shortDescription`.

Next steps:
1. Merlin: pick a neutral reverse-DNS id (needs a domain or handle he controls) and the public name, or ship the beta under the working title with a "working title" note.
2. Agent, after the pick: change `identifier`, set `category` (public.app-category.music), `copyright`; rename the bundle binary and deb path; swap "Pioneer" in user-facing strings where it is not nominative; update `scripts/mac/*`, `docs/process/beta-install-macos.md`, CI paths.
3. Brother: the crate names `pioneer-usb-writer` and `pioneer-library` are his lane and only visible in source; rename only if the repo goes public under the new name.

## 2. Data locations and migration - needs work (blocked by 1)

Where data lives (keyed on the bundle id through Tauri's `app_data_dir`):
- `~/Library/Application Support/<id>/`: `library.db` (`pioneer-test-ui/src/main.rs:1318`), `config.json` (`:1311`), `ui.json` (`backup_commands.rs:16`), `waveform-detail/` cache (`main.rs:887`), `Backups/` safety copies (`backup_commands.rs:19,175`), analysis failure memo and analysis order side file (`main.rs:309-313`, Waiting 32), optional `models/`.
- The webview's localStorage lives under `~/Library/WebKit/<id>/` (WKWebView default; not verified on a Mac). UI preferences held there are lost on an id change.
- Not keyed on the id: the CoreML compiled-model cache `~/Library/Caches/fourfour-coreml/` (`fourfour-analysis-rs/src/ort_beat.rs:74`). A custom library path is stored as an absolute path in `config.json` (`main.rs:1357-1365`) and survives.
- Merlin's installed copy is under the old id. Changing the id makes the new build start with an empty library; the old folder is not deleted.

Next step: on first start of the new id, if the old folder exists and the new one does not, copy (not move) `library.db`, `config.json`, `ui.json`, `waveform-detail/`; show one notice; leave the old folder. Test with a library at the current schema (`pioneer-library/src/lib.rs` migrations, v7).

## 3. Signing, notarization, hardened runtime - needs Merlin (account), then agent

What the repo has today:
- No signing config. `scripts/mac/build-dmg.sh:15` explicitly unsets `APPLE_SIGNING_IDENTITY`, `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`. `tauri.conf.json` has no `bundle.macOS.signingIdentity`, `entitlements` or `hardenedRuntime`.
- The dmg is ad-hoc signed. `scripts/mac/check-dmg.sh:34-47` passes "unsigned or ad-hoc" and fails on any `Authority=`, so it would fail a Developer ID build and needs a signed mode.
- ONNX Runtime is linked statically, not shipped as a dylib: `docs/architecture/perf-run-2026-11.md:439` (ort-sys downloads a static `libonnxruntime.a`) and `:645` (built app has only the app binary in `Contents/MacOS`, 59 MB, `minos 13.4`). CoreML is a system framework. So signing is one Mach-O plus resources, not the dylib tree that `docs/analysis/packaging-decision.md:49` feared for the Python option. `ort`'s `copy-dylibs` feature is set (`fourfour-analysis-rs/Cargo.toml:30,36`) but nothing extra was found in the checked dmg; recheck with `otool -L` and `ls Contents/Frameworks` on the next dmg (untested here).
- Dev binaries are no longer bundled (moved to `src/dev_bin/`, `perf-run-2026-11.md:645`).

What Apple needs (general knowledge, not tested in this repo; follow Tauri v2 macOS signing docs):
1. Apple Developer Program membership (paid, yearly) and a "Developer ID Application" certificate exported as .p12. No App Store, no sandbox: the app writes to arbitrary USB volumes and reads arbitrary music folders, so it stays unsandboxed.
2. Hardened runtime and a secure timestamp on every Mach-O (Tauri enables hardened runtime when signing).
3. Notarization with `notarytool`, using either Apple ID + app-specific password + team id, or an App Store Connect API key (Tauri reads `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` or `APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_PATH`), then stapling the ticket to the app and the dmg.
4. In CI: base64 certificate and password as GitHub secrets and a temporary keychain on the runner. `r35-ci` has no secrets by design (`docs/process/ci.md` on that branch).
5. Verify with `codesign --verify --deep --strict`, `spctl -a -vv`, `xcrun stapler validate`, then download the dmg on a second Mac or a fresh user so the quarantine flag is set, and open it normally.

Next steps:
1. Merlin: decide whether the public beta waits for signing (rec: yes; use unsigned plus the install note only for a small closed tester group). If yes, create the account and certificate himself. No agent touches Apple accounts. Secrets go in as GitHub secrets only.
2. Agent: add `hardenedRuntime`, an `entitlements` file and signing env to the Mac build path; add a signed mode to `check-dmg.sh` (Authority present, `spctl` accepted, stapled); notarize in a script run on the Mac; keep the unsigned path for local builds.
3. Mac night: first signed and notarized dmg, `check-dmg.sh`, open it from quarantine on a clean user.

## 4. Entitlements - needs work

- None exist (no `.entitlements` file, none in `tauri.conf.json`).
- Expected need under hardened runtime: probably none. No JIT in the Rust, ORT and rten path; WKWebView's JIT runs in the system WebContent process; audio output (cpal/CoreAudio) needs no entitlement; files come from user-picked folders in an unsandboxed app; CoreML compiling in-process and writing `~/Library/Caches/fourfour-coreml/` should not need one. All of this is expectation, untested on a signed build.
- If the signed build fails, first candidates: `com.apple.security.cs.allow-jit` or `allow-unsigned-executable-memory` (only if CoreML/ORT needs them), `disable-library-validation` (only if a dylib is added later). Add only what a failing test proves.
- No Info.plist usage strings are needed for audio output or user-picked folders. macOS shows its own prompt for removable volumes; if USB writes fail on a clean Mac, look there first (untested; Merlin's Mac already has permissions granted).

Next step: signed build on the Mac with an empty entitlements file, then run analysis (CoreML on, and off via `FOURFOUR_BEAT_RUNTIME=rten`), play audio, export to a USB, import a Rekordbox library.

## 5. Gatekeeper first run, min macOS, architecture - ready (unsigned); note needs work

- First run: `docs/process/beta-install-macos.md` covers right-click Open, `xattr -dr com.apple.quarantine`, "Open Anyway" on macOS 15+, and `--self-check`. Fine for testers; rewrite to "open normally" once signed.
- Minimum macOS 13.4: `tauri.conf.json:45`; binary `minos 13.4` and `LSMinimumSystemVersion` 13.4 (`perf-run-2026-11.md:645`), because the CoreML ONNX Runtime objects need it (`:539`).
- Apple Silicon only: `build-dmg.sh` builds natively on the Mac (`fourfour_0.1.0_aarch64.dmg`); pyke ships no `x86_64-apple-darwin` ONNX Runtime (`perf-run-2026-11.md:439`); the r35-ci macOS job asserts arm64. The install note says neither "Apple Silicon only" nor "macOS 13.4 or newer". An Intel Mac user gets no explanation.
- Size: 97 MB dmg with CoreML and weights (`perf-run-2026-11.md:645`); `ship-dmg.sh:7` caps copies at 300 MB.
- Version: 0.1.0 from `pioneer-test-ui/Cargo.toml` (r44-version). Pick the beta version string and where it is bumped.

Next step (agent): add the requirements line to the install note and README; Merlin picks the version string.

## 6. Licences and NOTICE - needs work (choices need Merlin)

What exists:
- `NOTICE` lists baken (MIT, full text), the FSL10K key templates, and format-fact references (Booth, cyanbox; no code copied).
- Bundled next to the weights (`pioneer-test-ui/tauri.bundle-weights.conf.json`): `LICENSE-beat-this.txt` (MIT, JKU and danigb) and `LICENSE-key-model.txt` (CC BY 4.0 dataset and CC BY 3.0 uploader credits, 309 loops, 122 uploaders), both from `scripts/mac/`. `check-dmg.sh` asserts both are present and that the key file credits "Ramires".
- `fourfour-stretch/vendor/LICENSE.txt` and `signalsmith-linear/LICENSE.txt` (MIT) are in the source tree.
- `fourfour-analysis-rs/Cargo.toml:6` says `MIT OR Apache-2.0`; `fourfour-stretch` says MIT; `pioneer-usb-writer`, `pioneer-library` and `pioneer-test-ui` declare no licence.

What is missing:
1. No repo `LICENSE` file, although AGENTS.md calls fourfour "open-source". Merlin and brother pick the licence.
2. **Built on branch `r54-notices` (not wired in):** `python3 -I scripts/release/third-party-notices.py <out>` writes the notices for the 403 Rust crates linked into the macOS release build (`aarch64-apple-darwin`, no dev-deps, no dev-bins, plus the `coreml`/ort crates read from `Cargo.lock`) and the hand-kept non-cargo items in `scripts/release/third-party-extras.json`. Generated copy: `pioneer-test-ui/resources/THIRD-PARTY-NOTICES.txt`. Still to do, both user-visible, so left for Merlin: add it to `bundle.resources` (`tauri.bundle-weights.conf.json`) and add an About row. Regenerate after any dependency change. Manual standard-text fallback (crate ships no licence file) for 66 crates, listed in the file: all `symphonia*` 0.5.x, `rten*`, `lofty`, `objc2*`, `phf*`, `unic-*`, `fax*`, `profiling*`, `block2`, `dispatch2`, `audio-core`, `alloc-stdlib`, `convert_case`, `dasp_sample`, `flatbuffers`, `fxhash`, `mac`, `match_token`, `ogg_pager`, `realfft`, `selectors`, `simd_helpers`, `zune-inflate`. Original item: NOTICE and the app do not list: ONNX Runtime (MIT, statically linked, so its notice must ship), `ort`, `rten` and `lofty` (MIT OR Apache-2.0), Signalsmith Stretch (MIT), symphonia 0.5 and 0.6 (MPL-2.0, file-level copyleft: source availability for symphonia), SQLCipher via `bundled-sqlcipher` in `pioneer-usb-writer/Cargo.toml` (BSD-style, notice required; crypto backend on macOS believed to be CommonCrypto, unverified), Tauri, `tauri-plugin-dialog` and the rest of the 683 locked packages (`Cargo.lock`), Geist fonts (SIL OFL 1.1; `app/src/assets/fonts/OFL.txt` is in the source tree but ships nowhere a user can read it), Svelte runtime in `app/dist` (MIT).
3. About credits are two rows: Beat This! and beat-this-rs (`AboutPage.svelte:12-15`). No key model attribution (CC BY needs visible credit; the file is in Resources but nothing links to it), no "Third-party notices" row.
4. **Substituted on branch `r54-notices`:** cargo-about and cargo-deny stay uninstalled; the stdlib script above replaces them and fails the run (exit 1) on any licence outside the allow list. A `deny.toml` for CI is still open. Original item: No tooling: `cargo-deny`, `cargo-about`, `cargo-license` and `cargo-audit` are not installed on the box (`~/.cargo/bin` holds only cargo, clippy, fmt, tauri, rustup shims) and the repo has no `deny.toml` or `about.toml`. Not installed here, per the rules. Next step: install them in a scratch `CARGO_HOME` on a CI runner or a Mac night, commit `deny.toml` (allow MIT, Apache-2.0, BSD-2/3, ISC, MPL-2.0, Unicode-3.0, Zlib, CC0), generate `THIRD-PARTY-NOTICES.html` into the bundle resources. npm: `app/package.json` has only `@tauri-apps/api` and `plugin-dialog` as runtime dependencies, so a short list is enough.
5. Beat This! weights: MIT, but upstream says some training files are copyrighted or under limited Creative Commons licences and users must assess impact (`docs/analysis/rust-status-2026-10-28.md` section O9, "Licence"). This is the authors' disclaimer, not a restriction, and it is the one open legal risk on the analysis path. Merlin decides (rec: accept for the beta, state it in the notices).
6. Rekordbox import and Pioneer's OneLibrary SQLCipher key (AGENTS.md "OneLibrary encryption", used by the writer): interoperability use of a widely published key. Merlin decides whether it needs a look before public distribution. Not legal advice.
7. Python fallback: `pioneer-test-ui/src/analysis_python.rs:7-17` still shells out to `analysis/.venv/bin/python` or `python3`, and `analysis_backend.rs:230` falls back to it when the Rust analyzer fails. That path leads to the AGPL Essentia and DeepRhythm stack (`packaging-decision.md`). A stranger's `python3` cannot import `fourfour_analysis`, so it normally errors, but a beta should not carry a path to AGPL code. Remove it or compile it out of release builds (small agent task).

## 7. Key model licence and quality - needs Merlin

- Shipped key stage is `cyankey::SHIPPED = &CLEAN_FSL` (`fourfour-analysis-rs/src/cyankey.rs:59`): templates fit only on CC0 and CC BY FSL10K loops. Accepted per rec on 2026-10-31 (Waiting 4).
- Cost: Beatport test 56.3% exact vs 60.3% for the cyanbox templates (GiantSteps MTG labels, unlicensed). Bar is >=60% (STATE "Direction"). Dev 59.4% vs 66.4%. Source: `docs/analysis/key-licence-2026-10-31.md`.
- Clean v2 on FMA clips: 57.3% test, within noise (Waiting 30, branch `b14-cleankey60`, rec leave unlanded). Earlier tries (Q46, Q49) topped out at 55.8%. Conclusion on record: clean 60% is out of reach without about 1,500 permissively labelled tracks.
- Attribution duty: `LICENSE-key-model.txt` ships (see 6); About does not link it.

Decision for Merlin: ship the beta with 56% key and say so, or hold the public beta until a label source exists. Rec: ship, with honest copy ("key is a suggestion; edit it in the inspector"). Either way: a one-line key accuracy statement in README and FAQ, and credits in About.

## 8. Privacy - ready (document it)

Checked in code on `ov-1028`:
- No HTTP client use in our Rust source: a grep for `reqwest|ureq|hyper|TcpStream|updater|sentry|telemetry|posthog` over `pioneer-test-ui/`, `pioneer-library/`, `pioneer-usb-writer/`, `fourfour-analysis-rs/`, `fourfour-stretch/` finds only a localhost URL in the dev bin `waveform_dev.rs`. `app/src` has no remote `fetch` (only test harnesses fetch local fixtures); fonts are bundled woff2, no CDN.
- `reqwest` and `hyper` are in `Cargo.lock` as dependencies of `tauri` itself, not called by us. `ureq` and `native-tls` come from `ort-sys` and run at build time only (ONNX Runtime download, `perf-run-2026-11.md:439`). Whether `reqwest` is linked into the release binary was not checked (needs a build).
- No updater, log or telemetry plugin: the only Tauri plugins in `Cargo.lock` are dialog and fs. `pioneer-test-ui/capabilities/default.json` allows `core:default`, dialog open/save, events and window drag; `remote.urls` are localhost dev servers.
- No CSP set (`tauri.conf.json` has no `app.security.csp`). Low risk with no remote content, but set one before a public build.
- `tauri` is built with the `devtools` feature (`pioneer-test-ui/Cargo.toml:18`), so release builds expose the web inspector. Decide: keep for beta debugging or drop.

Next steps (agent): state "fourfour makes no network connections; your music and library stay on your Mac" in README and About only after confirming with `nettop` or Little Snitch on the real app during a Mac night (not done here); add a CSP; Merlin decides on devtools.

## 9. Crash reporting and logs - needs work

- Built (r48-logs): a rotating log `fourfour.log` plus `fourfour.1.log` and `fourfour.2.log` (5 MB each, std only, no new dependency) in Tauri's `app_log_dir` (`~/Library/Logs/com.pioneer.test-ui/` on macOS, `~/.local/share/com.pioneer.test-ui/logs/` on Linux, `%LOCALAPPDATA%\com.pioneer.test-ui\logs` on Windows; the folder follows the bundle id, see 1). Module `pioneer-test-ui/src/app_log.rs`. It gets every debug-panel line (`debug_log`), the former `eprintln!` warnings, panics from any thread (message, location, version, backtrace) and frontend `console.error`, uncaught errors and unhandled rejections (rate limited, 20 per 10 s). The command `log_directory` returns the folder for a later "Reveal logs" button; no UI was added.
- Still open: the error text "see debug log" (`main.rs`, `all_tracks_failed`) should point at the log folder once the Reveal button exists. A panic before Tauri's setup (during `build`) is not in the file.
- Library open failure: `open_startup_library` (`main.rs:1434`) returns a typed error; the recovery panel is on `r38-recovery-v2` (Waiting 37), not landed.
- Analysis failures are memoised (`pioneer-test-ui/src/analysis_failures.rs`); their messages reach the log through the debug panel line. macOS also writes its own crash report to `~/Library/Logs/DiagnosticReports/` for hard crashes (signals, aborts), which the panic hook does not see.

Next steps (agent): a "Copy diagnostics" button in About (app version, macOS version, chip, library size, last 200 log lines; no file names unless the user ticks it). No automatic upload. Merlin confirms no-upload.

## 10. Auto-update - ready (there is none)

- Not present: no `tauri-plugin-updater` in `Cargo.lock` or `pioneer-test-ui/Cargo.toml`, no updater config in `tauri.conf.json`, no update check in `main.rs`.
- Adding it means: the plugin (adds network, breaks the "no network" statement in 8), a minisign update keypair (separate from Apple signing), a signed `latest.json` hosted somewhere, and a capability entry. Needs signing (3) first.
- Rec for beta 1: no updater; testers re-download; the version is shown in About (`AboutPage.svelte:12`). Revisit after the first signed release.

Needs Merlin: confirm "no updater in beta 1".

## 11. Backup and restore - needs work

Built:
- Library backup and restore are in the app: `pioneer-library/src/backup.rs` (tar with manifest, optional audio; `export_backup` `:394`, `unpack_backup` `:244`, `install_backup` `:348`; refuses a backup from a newer format `:187`) and `pioneer-test-ui/src/backup_commands.rs` (progress events, safety copy under `Backups/` before restore at `:19,175`, relinking of audio paths).
- Schema migrations are additive and tested (v5 to v7: Q42, Waiting 9 and 12). The planned schema bump (Waiting 31) needs a migration test with a library written by the beta.

Missing for a beta:
1. Recovery when `library.db` does not open: `r38-recovery-v2` (Waiting 37; copy says "fourfour", swap for "the app").
2. A migration promise: every beta version opens older libraries; downgrade is refused with a message. Add a test that opens a library written by this beta's schema.
3. User-facing text: where backups go, how to restore, what is not backed up (audio is optional).

Item 2 is built on branch `r51-migration-test`: the library already refused a newer schema with `LibraryFailure::NewerVersion` (mapped to `ErrorKind::LibraryNewerVersion`). `pioneer-library/tests/schema_fixture.rs` opens the committed `tests/fixtures/library-schema-v7-beta1/` (library.db plus its anlz tree) (3 tracks, analysis and cues, a folder and playlist) with data intact and asserts that a version above the fixture is refused and left unchanged. A schema bump must keep this fixture opening; regenerate a new one only with `cargo test -p pioneer-library --test schema_fixture -- --ignored` on the old code.

Next step: Merlin lands `r38-recovery-v2`; agent writes the migration test and the FAQ entry.

## 12. USB export safety - needs brother

Done on `ov-1028` (Q42, merged to master 9fc5e48; `docs/ui/usb-sync-spec.md` "Built"): playlist-scoped sync through `sync_usb_many` with `SyncSelection::Playlists`, stable stick id, ownership manifest `PIONEER/fourfour-manifest.json` (sync removes only files it wrote), `sync_usb_keeping(keep_files)` so removals never delete foreign files (Waiting 10), dry-run plan, capacity check, wipe guard that refuses the system volume (`pioneer-test-ui/src/volumes.rs:57`), removable-only listing.

What the beta needs from the brother, in priority order:
1. CDJ-3000 test of a current export with `docs/architecture/cdj-checklist-2026-10.md` (open since Waiting 0): multi-page PDB over 190 tracks, cue names, colours and loops, 3-band `.2EX` waveforms, playlist folders as PDB/OneLibrary tree (Waiting 12, schema v7), re-sync after removing a track, hot cues after re-sync. Merlin travels, no hardware on this run.
2. Manifest-aware wipe. The spec lists it as "Not built": wipe still deletes whole `PIONEER/` and `Contents/`, so a stick that also holds a Rekordbox export loses those files. Needed before strangers wipe shared sticks (spec engine change 7).
3. First sync to a stick that already has a Rekordbox export: foreign tracks drop out of the CDJ browser while their files stay (spec "Needs a real stick / CDJ check"). Decide and test what the user sees.
4. Duplicate USB path: two tracks with the same artist and file name (untagged "01 Intro.mp3") land on one path (STATE brother's lane r12; ignored repro in `pioneer-library/tests/robust_store.rs`).
5. Lofty rejects some bad tags, so playable files are skipped (relaxed parse in `pioneer-usb-writer/src/scanner.rs`); a cold 10k import reads 2.7 GB because of ID3 pictures (r6).
6. Q6 key source PDB vs OneLibrary; `.2EX` and PWV6-7 placement for CDJ-3000X; older players (CDJ-2000NXS2) check.
7. Stick id across a macOS remount and capacity numbers against Finder (needs a real stick; spec "Built").
8. FAT32/exFAT behaviour and the 4 GB file limit for large WAV/AIFF on FAT32: not covered in any doc found; ask the brother.

Windows items (r20: `is_removable` true for every drive, `statvfs` stub) are not blockers for a macOS-only beta.

## 13. Analysis quality bars - needs Merlin

| Bar | Target | Now | Evidence | Status |
|---|---|---|---|---|
| Key exact (Beatport test, n=199) | >=60% | 56.3% clean model (60.3% with unlicensed templates) | `key-licence-2026-10-31.md`, Waiting 4 | not met; accepted for beta per rec |
| Beat grid within 20 ms of Rekordbox | no number set besides this metric | 80.5% test, 87.3% on constant grids | `rust-status-2026-10-28.md` line 16, AGENTS.md "targets" | Merlin to confirm it is enough |
| BPM within 0.1 / 1.0 of Rekordbox | | 180 and 190 of 205 (87.8%, 92.7%) | `rust-status-2026-10-28.md` B15 table | ok |
| Bar 1 (downbeat) vs Rekordbox | | 79.3% test (was 61.5%), dev 88.9% | Waiting 16; `BAR_ONE_RULE` at `fourfour-analysis-rs/src/service.rs:278`; landed on ov-1028 29add96 | listen check open |
| Cues, energy | | Rust emits none (`cue_points: []`, `energy: None`) | `rust-status-2026-10-28.md:25` | say so in FAQ |
| CoreML vs rten results | identical | keys 12/12, every beat time identical to the ms | `perf-run-2026-11.md:640-645` | ready |

Next steps:
1. Merlin: listen to the 5 bar-1 regressions on the Mac (list in `rust-status-2026-10-28.md` B15; Waiting 16). 72 tracks differ: 59 now right, 13 were right before.
2. Merlin: the key call in 7.
3. Agent: a one-page "what to expect" (grid about 80% within 20 ms, key about 56%, no cues or energy) for the README and FAQ, as a release note and not marketing copy.

## 14. Known open Waiting items touching the beta

From master STATE.md (ddcb8c5):
- 4 key licence: accepted, landed. Needs About attribution (6).
- 16 bar 1: landed, listen check open (13).
- 24 bundle id: open (1).
- 15 CI: open (15 below).
- 33 error messages (`r18-errors-v2`): holds real bugs (analysis falls back to the browser mock in the real app, unsaved playlists look saved). Rec: land first. A beta blocker in practice.
- 34 keyboard and screen reader (`r17-a11y-v2`): touches the design-system fence; needs a Mac WebKit check.
- 35 cover cache cap (`r31-cover-cap-v2`), 36 WebKit startup CSS (`r29-css-v2`), 37 recovery panel (`r38-recovery-v2`): parked; `parked-all-v2` (a87f361) is all of them at once.
- 19 waveform cache cap 512 MiB (rec 2 GiB); 21 rten bump (rec drop); 29 UI headroom and QoS (rec keep); 31 schema bump for 50k (after the holiday); 27 ORT and CoreML (CoreML shipped through r26 and r30).
- 1, 2, 3 USB sync round 3 UI; 5 old stash; 6 `.claude/settings.local.json` push rule; 7 push master (30+ commits ahead). Not beta blockers: 5, 6, and 31 until libraries reach 50k.

Next step: Merlin takes the recs after the holiday in this order: 33, 37, 24, 15, then 34, 35, 36 with one Mac check, then the 16 listen.

## 15. CI - needs Merlin

- Nothing runs today: `ov-1028` has no `.github/` directory.
- `r35-ci` (24add8b, Waiting 15, supersedes `ov-ci`) has `.github/workflows/ci.yml` with: linux (tests, Chromium smoke, deb checks, `--self-check` from the extracted deb), webkit-smoke (allowed to fail), macos-14 arm64 (`build-dmg.sh` and `check-dmg.sh`, unsigned, CoreML), windows-2022 (nsis, OpenSSL for sqlcipher). actionlint clean, Linux steps pass locally, never run on GitHub. Doc: `docs/process/ci.md` on that branch.
- Known gaps listed there: Windows `libcrypto-3` DLL not bundled, ORT download on the runner, `hdiutil` on a hosted runner, durations unmeasured. Windows is out of scope for a macOS beta; the macOS job is the one that matters.
- No release workflow: no tag build, no notarization job, no checksum publication, no draft GitHub Release.

Next steps:
1. Merlin: OK to push `r35-ci` for a first run (shared repo `morizkraemer/fourfour`; STATE Q8 says the first run needs Merlin's OK). Agents never push.
2. After the first green macOS run: a manual-trigger release workflow (build, sign, notarize, staple, SHA256, upload artifact; secrets from 3), no auto-publish.

## 16. User docs - needs work

- `README.md` is a developer README ("Agent entry", `cargo build`, "Status: ... under active development"). No user description, download link, requirements or screenshots.
- Exists: `docs/process/beta-install-macos.md` (Gatekeeper note and `--self-check`, with maintainer build steps mixed into the user page) and `docs/ui/keyboard-shortcuts.md` (reusable as a help page).
- Missing: user README; install page without the maintainer section; requirements (Apple Silicon, macOS 13.4+); quick start (scan a folder, analyze, build a playlist, put it on a stick); what the export writes to the stick and what it never deletes; FAQ (supported CDJ models, "my USB is not recognised", key accuracy, analysis speed, backups, how to remove all data: `~/Library/Application Support/<id>`, `~/Library/Caches/fourfour-coreml`, `~/Library/WebKit/<id>`); known issues list (from 12 and 14); link to licences.

Next step (agent): draft `docs/release/README-users.md` and `docs/release/FAQ.md` from the sources above; Merlin edits tone and the name after item 1. No marketing copy.

## 17. Support channel and release hosting - needs Merlin

- Nothing defined. The git remote is `git@github.com:morizkraemer/fourfour.git` (brother's account); no issue templates, no contact address, no place for a stranger to download the dmg.
- Options: GitHub Issues and Releases on the repo (free; needs the repo visibility decision and the brother's agreement), a small page on a domain Merlin owns with a mailto, or a chat room. Pick one primary and put it in About and README.
- Also decide whether the repo is public at beta time. A review before that: no private paths or music data in `docs/` and `benchmark/` manifests, and STATE.md and the journal carry personal planning context (Compass baseline, holiday dates); scrub them or keep them out of the public tree.

Next step: Merlin decides channel, hosting and repo visibility; the agent then adds issue templates (bug report asking for the "Copy diagnostics" output), a contact line and a release page template.

---

## Suggested order

1. Merlin, one sitting: 1 (name and id), 3 (sign or not), 7 (key), 10 (no updater), 17 (channel and repo visibility), 15 (push `r35-ci`).
2. Agent work needing no decision: 6 (notice tooling, credits, Python fallback), 9 (logs), 16 (docs), 5 (requirements line), 13 (expectation page), 11 (migration test).
3. Land `parked-all-v2` after one Mac WebKit check (14).
4. After 1: migration (2), then signing path (3, 4) on a Mac night; first notarized dmg; install test on a clean user.
5. Brother: CDJ-3000 checklist, manifest-aware wipe, duplicate path (12).
6. Release candidate: tag, signed dmg, SHA256, release notes with known issues, support channel live.

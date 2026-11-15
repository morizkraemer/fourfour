# Error and failure messages (2026-11)

Audit of every message a user can see when something fails, a rule set for new
ones, and the fixes made on branch `r18-errors`. User-visible copy: nothing here
lands before Merlin has reviewed it.

How to read it: **before** is what the app showed (or did not show), **after** is
what the branch shows. Raw detail is never thrown away: the UI logs it to the
browser console (`console.info` for failures that are also shown), and the Rust
debug log keeps it.

## Rules

1. **Say what happened, in the user's words.** Track, file, stick, library. Never
   lock, schema, decoder, IPC, state, backend.
2. **Say what to do next.** If nothing can be done, say what is safe ("Nothing was
   written.", "Your previous library is saved at ...").
3. **No internals.** No Rust type or command names, no `os error N`, no SQL text,
   no "poisoned lock", no `Failed to <verb> <path>: ...` chains, no "see debug log".
4. **No blame.** "fourfour can't read the file", not "invalid file" or "you must".
5. **File name, not full path.** A full path only when the user has to go and find
   it (the restore safety copy, the tooltip of a missing file).
6. **Plain typography.** Hyphen "-" only, no en or em dashes, no exclamation marks,
   no "Oops", sentence case, straight apostrophes in new text.
7. **Short.** One to three short sentences. The first one leads with what failed:
   "Couldn't ...", "Can't play ...".
8. **A failure always reaches the user.** Status bar, dialog line or inline text.
   Never only the console.
9. **Same voice as the good messages already in the app:** "Not enough space on X.
   Nothing was written.", "Put tracks or a playlist on this stick first", "This
   import is no longer waiting. Import the files again.", "Relocate it from the
   library status button in the top bar."

## How the fix is built

- `app/src/lib/userErrors.ts` maps a raw backend error to a message: a table of
  patterns (disk full, permission, missing file, drive gone, library busy, newer
  library, backup problems, Rekordbox problems, sync problems, audio output,
  edit validation) with one `...FailureText` helper per area. An unknown error
  falls back to the area's plain sentence plus a generic next step.
- Call sites in the stores use it where they used to show `String(err)`. The raw
  error is logged with `console.info`.
- Rust is unchanged. The messages stay as they are in the logs; the UI layer
  words them.
- Four `tauri.svelte.ts` functions swallowed real-app failures behind the browser
  mock (rows 11, 30, 31); they now rethrow in the real app like their
  neighbours (`tauri-errors.test.ts` covers it).
- Tests: `userErrors.test.ts` (mapping, no internals, no dashes), plus the old
  assertions of the stores that checked raw strings.

## Inventory

Verdicts: **ok**, **unclear** (vague, no cause), **leaks** (internals shown),
**no next step**, **silent** (should reach the user, does not).
"Fixed" means changed on this branch; "listed" means left for a decision.

### Playback and audio

| # | Where it fires | Before | After | Verdict |
|---|---|---|---|---|
| 1 | Play, seek or cue on a missing file (`playback.rs` "Open {path}: ...") | Nothing. `player.error` is set but no component renders it; the play button just flips back. | Can't play "a.mp3". The file is missing or its drive is off. Reconnect the drive or relocate the file. (status bar) | silent + leaks, fixed |
| 2 | Decode and format errors (`Unsupported or corrupt audio file {path}`, `Decode error in ...`, `Read error`, `Decoder crashed`, `Unsupported codec`, `No default audio track`) | Nothing (as row 1). Raw text carried the full path. | Can't play "a.flac". The file is damaged or in a format fourfour can't read. | silent + leaks, fixed |
| 3 | Audio device errors (`Audio output error: ...`, `No default output audio device`, `Failed to open CoreAudio output stream`) | Nothing. | Can't play. fourfour can't reach the audio output. Check your speakers or interface, or choose another output in Settings. | silent + leaks, fixed |
| 4 | Audio engine never started (`PlaybackEngine::new` failed at launch, `eprintln` only) | Nothing; commands fail with Tauri's "state not managed for field `engine` ... call `.manage()`". | Same message as row 3. | silent + leaks, fixed |
| 5 | `Audio output changed while loading; try again`, `decode buffer missing`, `resampler missing` | Nothing. | The audio output changed. Press play again. / Playback stopped unexpectedly. Press play to try again. | silent, fixed |
| 6 | Cue save fails (`cues.svelte.ts`) | Nothing (set on `player.error`, never shown). | Couldn't save the cues. <cause or Try again.> | silent + leaks, fixed |
| 7 | Beat grid save fails (`player.svelte.ts`) | Nothing. Raw `err` appended. | Couldn't save the beat grid. <cause or Try again.> | silent + leaks, fixed |
| 8 | Settings > Audio: output device, sample rate or buffer change fails (promise dropped with `void`) | Control shows the new value, playback unchanged, no message. The next play now shows row 3. | unchanged | silent, listed |

### Analysis

| # | Where it fires | Before | After | Verdict |
|---|---|---|---|---|
| 9 | One file fails in a run (`Failed X.mp3 (3/10)` progress event) | Status bar shows "Failed X.mp3 (3/10)", replaced by the next event within a second. Cause only in the debug log. | Couldn't analyze "X.mp3". Check that the file plays, then analyze it again. | no next step, fixed |
| 10 | Run ends with failed tracks | Status bar goes to "Ready". | 3 tracks couldn't be analyzed. Check that they play, then analyze them again. | silent, fixed |
| 11 | `analyze_track_ids` or `analyze_tracks` rejects in the real app | Hidden: the browser mock took over, emitted fake "Analyzed" progress events and a fake "complete". | Rethrown; status bar shows row 12 or 13. Spinner state is reset. | silent, fixed |
| 12 | Same rejection, generic catch (`Analysis failed`) | "Analysis failed" and the analyzing flag stayed on. | Couldn't start the analysis. <cause or Try again.> Analyzing flag cleared. | unclear, fixed |
| 13 | No analysis backend (`NO_BACKEND_ERROR`: names `beat_this.onnx`, `mel_spectrogram.onnx`, `FOURFOUR_MODELS_DIR`) | Not shown (hidden by row 11), raw text if it had been. | Couldn't start the analysis. Analysis isn't available because its models are missing. Reinstall fourfour. | leaks, fixed |
| 14 | `Analysis failed for all {n} tracks - see debug log` (the Rust string uses an em dash) | Not shown (row 11); "debug log" means nothing to a user. | None of the tracks could be analyzed. Check that the files play, then try again. | leaks, fixed |
| 15 | Resume at launch fails (`resume_analysis`, `Resumed analysis failed` in the debug log only; the UI gets 0) | Nothing; unanalyzed tracks stay unanalyzed. | unchanged | silent, listed |
| 16 | `enqueue_analysis` and `prioritize_analysis` fail (return 0 / swallow) | Nothing; the track just does not get analyzed. | unchanged | silent, listed |
| 17 | Analysis display fetch fails (`getAnalysisData`, row previews, labels: `console.warn`) | The waveform and fields stay empty. | unchanged | silent, listed (low) |

### Import

| # | Where it fires | Before | After | Verdict |
|---|---|---|---|---|
| 18 | Import fails (`plan_import`, `commit_import`: SQLite and lock text via `e.to_string()`) | Top-bar chip "Import failed", menu lines show the raw error (e.g. "poisoned lock: another task failed inside", "database is locked"). | Couldn't import those files. The library is busy. Wait a moment and try again. (and the other causes of the rule table) | leaks, fixed |
| 19 | `This import is no longer waiting. Import the files again.` | same | unchanged | ok |
| 20 | Skipped files line: "N files skipped: unsupported format (.xyz)" | same | unchanged | ok |
| 21 | Skipped files line: "N audio files couldn't be read." / headline "Nothing imported" | same | unchanged (reason and count are clear; next step is implied) | ok |
| 22 | Rekordbox: master.db cannot be decrypted (`Failed to set SQLCipher key`, `master.db decryption failed: not a Rekordbox master.db or wrong key`) | Raw text with the full master.db path in the dialog. | Couldn't read the Rekordbox library. fourfour can't read this Rekordbox library. It may be from a Rekordbox version fourfour doesn't support yet. Try the XML export instead. | leaks, fixed |
| 23 | Rekordbox: not an XML export (`Not a Rekordbox XML export: no DJ_PLAYLISTS element`) | Raw text. | Couldn't read the Rekordbox library. That file isn't a Rekordbox XML export. Choose the rekordbox.xml you exported from Rekordbox. | leaks, fixed |
| 24 | Rekordbox: USB without library (`No PIONEER/rekordbox/exportLibrary.db found under {path}`, `only has the legacy export.pdb`) | Raw text with path. | That folder isn't a Rekordbox USB drive. Choose the top folder of the drive. / ... uses Rekordbox's older format, which fourfour can't read yet. | leaks, fixed |
| 25 | Rekordbox import run fails (`{e:#}` chain) | Raw chain. | Couldn't import the Rekordbox library. <cause or Check the file or folder, then try again.> | leaks, fixed |

### Library, database and edits

| # | Where it fires | Before | After | Verdict |
|---|---|---|---|---|
| 26 | Library database fails to open at launch (`main.rs` `.expect("Failed to open library database")`: corrupt file, newer schema, folder on a drive that is off) | The app panics in setup. No window, no message. | unchanged | silent, listed (needs a decision, see below) |
| 27 | Library fails to load after launch | Status bar "Failed to load library". | Couldn't load your library. <cause or the next step: Restart fourfour. If it keeps happening, check the library file in Settings.> | no next step, fixed |
| 28 | Change library folder fails (Settings > Library) | Status bar "Failed to open library". | Couldn't open a library in that folder. That library file is damaged or isn't a fourfour library. (or the other causes; default: Choose another folder.) | unclear, fixed |
| 29 | Reset library fails (Settings > Advanced) | "Library reset failed". | Couldn't reset the library. <cause or Try again.> | unclear, fixed |
| 30 | Playlist save fails (`save_state`, swallowed in `tauri.svelte.ts` and again in the store) | Nothing; playlists look saved and are not. | Couldn't save your playlists. Recent changes may be lost. <cause or Try again.> | silent, fixed |
| 31 | Remove tracks fails (swallowed in `tauri.svelte.ts` under the mock fallback) | Nothing; the rows reappear after the reload. | Couldn't remove the tracks. <cause or Try again.> | silent, fixed |
| 32 | BPM, key, rating, tags, release hand-set values fail (`Setting the BPM failed: {raw err}`) | Status bar with raw text. | Couldn't set the BPM. <cause or Try again.> Same for key, rating, "save the tags", "give the hand-set value back to the analysis". | leaks, fixed |
| 33 | Tag validation (`Title cannot be empty`, `{name} is longer than N characters`, control character) | "Saving the tags failed: Title cannot be empty". | Couldn't save the tags. The title can't be empty. / The album can be at most 500 characters. | unclear, fixed |
| 34 | Edit needs an analysis (`Track has no analysis to set the BPM of`), key not Camelot, BPM out of range, track gone | Raw text. | The track has to be analyzed first. Analyze it, then try again. / That isn't a Camelot key. Use a key like 8A. / That value is outside the allowed range. / The track is no longer in the library. | leaks, fixed |
| 35 | Settings toggles fail (auto-analyze, analyze after import, analysis engine; dropped promise, value snaps back) | Nothing but the snap-back. | unchanged | silent, listed |
| 36 | "Set test cues" (debug menu) fails | Console only. | unchanged | silent, listed (debug only) |
| 37 | BPM field and BPM popup: invalid typed value | Red border, no text. | unchanged | no next step, listed (taste call: needs a hint) |
| 38 | Settings > Advanced "Last error" shows the last backend error text | Raw error. | unchanged: it is a labelled developer diagnostic | ok (intentional) |

### Files, relocate

| # | Where it fires | Before | After | Verdict |
|---|---|---|---|---|
| 39 | Library check fails | `Library check failed: {raw}` | Couldn't check your files. <cause or Try again.> | leaks, fixed |
| 40 | Relocate in folder fails (dialog line) | Raw `String(err)` (e.g. `/x is not a folder`) | Couldn't relink the files. That isn't a folder. Choose a folder. | leaks, fixed |
| 41 | Locate one file fails (dialog line) | Raw (`/x is not a readable file`, `Track 4 is not in the library`) | Couldn't relink the files. fourfour can't read that file. Choose a different one. | leaks, fixed |
| 42 | Tooltip of a missing file: "File missing: {path} / Relocate it from the library status button in the top bar." | same | unchanged | ok |
| 43 | Tooltip of an unreadable file: "Can't read this file: {path}" | same | ... Check that it opens in another app, or relocate it from the library status button in the top bar. | no next step, fixed |
| 44 | Blocked-load tip: "“Title” is missing. It can't be loaded." with Locate | same | ... can't be read, so it can't be loaded. (same pattern for missing) | ok (wording only) |
| 45 | Dialog lines: "N files are missing (moved, or on a drive that is off)", "N files can't be read", row reasons "more than one match" / "not found" | same | unchanged | ok |
| 46 | Reveal in Finder fails (`File not found: {full path}`, `Failed to ...`) | Status bar with raw text and path. | Couldn't show the file. It may have been moved or deleted. | leaks, fixed |

### USB sticks

| # | Where it fires | Before | After | Verdict |
|---|---|---|---|---|
| 47 | Sync plan or write fails (`Cannot open /Volumes/X: ...`, `Cannot read free space on X`, `statvfs {path}`, anyhow chains) shown in the stick header | Raw chain. | The sync stopped. fourfour can't reach the stick. Check that it is still plugged in, then sync again. | leaks, fixed |
| 48 | `Not enough space on X: needs 2.0 GB, 1.0 GB free. Nothing was written.` | same | unchanged, passed through as is | ok |
| 49 | Sync fails without a detail (`Sync failed.`) | "Sync failed." | The sync stopped. Check the stick, then sync again. | no next step, fixed |
| 50 | Stick is already syncing, chosen twice, nothing selected | Raw engine sentences. | That stick is already syncing. Wait for it to finish. / ... | unclear, fixed |
| 51 | Read the stick's library fails (`refreshUsbState`) | Status bar "Failed to read USB". | Couldn't read the stick. <cause or Check that it is plugged in, then reselect it.> | no next step, fixed |
| 52 | Eject fails (`Eject failed: {stderr}`, `Failed to run diskutil`) | Nothing (console only). | Couldn't eject the stick. Something is still using it. Close other apps using the stick, then try again. | silent, fixed |
| 53 | Clear stick (wipe) fails | Nothing (console only). | Couldn't clear the stick. <cause or Try again.> | silent, fixed |
| 54 | Put tracks or playlists on a stick fails (`sticks.error`, set but never rendered) | Nothing. | unchanged | silent, listed (needs a place to show it) |
| 55 | "Capacity unavailable" | same | unchanged | ok |
| 56 | "Put tracks or a playlist on this stick first" (Sync button hint) | same | unchanged | ok |

### Backup and restore

| # | Where it fires | Before | After | Verdict |
|---|---|---|---|---|
| 57 | Save backup fails | `Backup failed: Failed to create /x/y.partial: Permission denied (os error 13)` | Couldn't save the backup. fourfour doesn't have permission to use that location. Pick another one. | leaks, fixed |
| 58 | Open a file that is not a backup, damaged, newer, incomplete | Backend sentence as is ("This file is not a library backup"), no next step. | Couldn't open that backup. That file isn't a fourfour library backup. Choose another file. (and the damaged, newer, incomplete variants) | no next step, fixed |
| 59 | Restore fails after the library was swapped (`Restore failed: {chain}. Your previous library is saved at {path}`) | Raw chain, then the path. | Couldn't restore the backup. That backup file is damaged. Choose another one. Your previous library is saved at /path/before-restore-....gz | leaks, fixed (path kept on purpose) |
| 60 | Restore fails before anything changed (`Could not save the current library first, nothing was changed: {e}`, unpack errors) | Raw chain. | Couldn't save your current library first, so nothing was changed. Try again. / Couldn't restore the backup. Your library was not changed. Try again. | leaks, fixed |

## Counts

- Messages found: 60 rows. Some rows group several backend strings; the Rust
  side has about 120 distinct error strings behind them.
- ok: 10 (rows 19, 20, 21, 38, 42, 44, 45, 48, 55, 56).
- Changed on this branch: 42 (41 fixed, plus the wording of row 44).
- Left alone and listed: 9 (rows 8, 15, 16, 17, 26, 35, 36, 37, 54).
- Silent failures: 21 rows. Surfaced: 13 (rows 1 to 7, 10, 11, 30, 31, 52, 53).
  Left listed: 8 (rows 8, 15, 16, 17, 26, 35, 36, 54).

## Left for Merlin

- **Library fails to open at launch (row 26).** The app panics before any window
  exists. A real fix is a small recovery screen: "Couldn't open your library
  at <folder>. Choose another folder or start a new library." Product call, so not
  built here.
- **Where failures show.** All surfaced errors use the status bar, which the next
  analysis progress event overwrites while a run is active. A transient toast or
  a short history in the library activity menu would fix that; not built, it
  changes the UI.
- **Silent settings failures (rows 8, 35) and stick edits (row 54).** Each needs a
  visible place (inline line under the control, or the stick header).
- **Typography drift in non-error copy.** Existing strings still use em dashes or
  en dashes: "Nothing new - ..." in `importDuplicates.ts`, "Library reset -
  empty", `Artist - Title` in the health dialog. Not errors, not touched.
- **Analysis failure causes.** The Rust side only reports the file name with
  "Failed". A one-word cause (damaged, timed out, not stored) from
  `analysis_run.rs` would let the message say more.

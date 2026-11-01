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
- **Startup `expect`:** `LocalLibrary::open(...).expect("Failed to open library database")` in `setup` panics on a damaged `library.db`. `schema::initialize` also treats an unreadable `schema_version` as a fresh database. Both are product decisions (quarantine the file? ask?).
- **Duplicates inside one drop are not flagged** (only against the library), and symlinked folders or files import as separate rows under their link paths. Aliases of the same file count twice.
- `normalise` path keys fold case, so on a case-sensitive volume `A.wav` next to `a.wav` counts as already in the library.
- A WAV cut to 20% analyses as complete with BPM 0; the decoder's `truncated` flag is not shown anywhere.

## Run

`cargo test -p pioneer-test-ui --bin pioneer-test-ui robustness -- --nocapture` (about 15 s with the decode engine, 5 min with the real analyzer and `FOURFOUR_MODELS_DIR=~/.cache/fourfour-models`); `cargo test -p fourfour-analysis-rs --test messy_audio -- --include-ignored` with weights; `cargo test -p pioneer-library --test robust_store`. Permission cases skip as root.

# Rust analysis in the app (Q33)

Branch `q33-rust-analysis`. The app now analyses tracks with the Rust analyzer (`ffa`: decode, Beat This! on rten, tempo/grid fit with the Q30 rules, cyanbox-port key, shared band waveforms) when the model weights are installed. Python stays as the fallback and is untouched in behaviour.

## Design

- **Library crate** `fourfour-analysis-rs/` (workspace member, lib `fourfour_analysis`). The prototype core moved here; `prototyping/analysis-packaging` re-exports it as `ffcore`, so `ffa`, `grid`, `cyankey` and the other prototype bins build unchanged.
  - `service::Analyzer`: loads the model once (`Arc<BeatEngine>`, immutable weights), `analyze_with(path, cancel, threads) -> pioneer_usb_writer::models::AnalysisResult`.
  - `service::analyze(path)`: one-call form with a lazily loaded default analyzer.
  - `service::analyze_batch` / `run_pool`: worker pool with per-track progress callback and a cancel flag (checked between stages: after decode and after the model).
  - `service::plan_pool`: workers = min(max threads, `available memory * 0.5 / 900 MB`, jobs); threads per track = max threads / workers. The 900 MB per worker figure is the measured single-thread peak (0.56 to 0.7 GB, `pipeline-review-2026-10.md`) with margin.
  - `bin/analyze-json`: dumps `AnalysisResult` JSON for a list of files (used for parity).
- **Mapping to `AnalysisResult`** (`service::to_app_result`): BPM from the fit (fractional), key as Camelot, beats from the fitted grid with bar positions from the model downbeats (vote over `index mod 4`, so bar 1 sits on a real downbeat; Python just counts from the first beat), `tempo` = BPM x 100 on every beat, colour waveform and the 400-byte PWAV preview from the shared `pioneer_usb_writer::waveform` module (`Meter` over the decoded mono mix, then `analyze`), including the `exact` PWV3/PWV5/PWVC columns. Energy is `None`, cue points are empty (see "What is left").
- **App wiring** (`pioneer-test-ui/src`):
  - `analysis_run.rs`: the coordinator (queue, dedupe, `prioritize_analysis`, `enqueue_analysis`, total counter, run guard) and the run loop moved out of `main.rs`, now generic over an `AnalysisSink` (log, progress, complete). `main.rs` supplies a Tauri sink that emits the same `debug-log`, `analysis-progress` and `analysis-complete` events with the same messages. The queue semantics are unchanged: front-insert for priority, dedupe by track id, merge into an active run.
  - `analysis_python.rs`: `resolve_python`, `analyze_one_track`, `parse_analyzer_stdout`, `python_result_to_analysis` moved verbatim (plus master's band pass, unchanged).
  - `analysis_backend.rs`: backend choice, setting storage, analyzer cache.
  - New command `cancel_analysis` (clears the queue and stops in-flight Rust analyses at the next stage). No UI uses it yet, so there is no visual change.
  - `bin/bench_analysis.rs`: includes the same modules by path and drives `run_track_analysis` like the Tauri commands do.
- **Concurrency**: Python keeps its 3 parallel processes. Rust runs `plan.workers` tasks (`spawn_blocking`), each with `plan.threads_per_track` threads, recomputed per launched track from the remaining queue so a lone track gets all threads. Reserve: the app uses `cores - 1` threads at most (`FOURFOUR_ANALYSIS_THREADS` overrides). One model instance is shared by all workers instead of one per worker: the weights are immutable and the per-worker memory is activations, so a copy per worker would add 83 MB x workers for nothing. Each worker reuses it across tracks.
- **Timeout**: Python keeps 120 s. Rust uses 900 s: one single-thread track takes 70 to 110 s on 8 busy cores, so 120 s killed tracks in the first bench run (10 of 50 timed out before the change).

## Settings and weights

- Setting `analysis_backend` in the app `config.json` (`"rust"` or `"python"`; absent means default). Commands `get_analysis_backend` and `set_analysis_backend` (no UI). `write_library_path` now keeps other config keys. Env override `FOURFOUR_ANALYSIS_BACKEND=rust|python`.
- Default: rust when weights are found, otherwise python. Rust requested without weights falls back to python and logs one line in the debug panel: `analysis backend: python - rust requested but model weights are missing (looked in $FOURFOUR_MODELS_DIR, <app data>/models, dev models dir; run ...)`. A weights directory that fails to load also falls back, with the load error in the log.
- Weights lookup order: `$FOURFOUR_MODELS_DIR`, `<app data dir>/models`, `prototyping/analysis-packaging/models` (dev). Needs `mel_spectrogram.onnx` and `beat_this.onnx`. Never committed (`models/` is gitignored; 83 MB).
- Fetch: `fourfour-analysis-rs/scripts/fetch-models.sh [dest]` (default dest is the dev dir; checks the sha256).

### Weights on the Mac build

The app data dir is `~/Library/Application Support/com.pioneer.test-ui/` (identifier `com.pioneer.test-ui`). On the Mac, from the repo root:

```bash
fourfour-analysis-rs/scripts/fetch-models.sh "$HOME/Library/Application Support/com.pioneer.test-ui/models"
```

Restart not needed; the next analysis run picks them up. For a dev run from the checkout, run the script without an argument.

### Packaging notes (later)

- Bundle size: `beat_this.onnx` 83.2 MB plus `mel_spectrogram.onnx` 0.27 MB. The installer grows by about 84 MB uncompressed (ONNX weights compress poorly, roughly 75 to 80 MB zipped).
- Platform: the files are the same for macOS, Windows and Linux (rten is pure Rust, CPU only). Ship them as Tauri `resources` (`bundle.resources`) and add the resolved resource dir as a lookup candidate before the app data dir; `locate_models` takes the candidate list from one place. Alternative: download on first run into the app data dir with the fetch script's URLs and checksum, which keeps the installer small but needs network once.
- Licences: Beat This! weights are MIT (original) and the beat-this-rs port is MIT; recorded in `fourfour-analysis-rs/NOTICE`.

## Parity: Rust vs Python on 10 tracks

Tracks: every 63rd file of `benchmark/datasets/rekordbox-testdata-2025` (3 wav, 4 mp3, 3 flac, 4.9 to 7.9 min each). Python is the app's call (`--backend deeprhythm_essentia`), Rust is `analyze-json` (same `AnalysisResult` the app stores). Scripts: `fourfour-analysis-rs/scripts/parity_run.py` and `parity_compare.py` (run with the analysis venv). There is no Rekordbox ground truth for these 10 files, so the table shows agreement, not accuracy. Accuracy against Rekordbox on the 205-track set is in `bpm-grid-2026-10.md` (Rust BPM 86% within +-0.1, key 58.3% exact on Beatport).

| Track | BPM py / rs | Key py / rs | Beats py / rs | First-beat phase | PWAV height corr | PWAV height MAD (of 31) |
|---|---|---|---|---|---|---|
| Burn | 133 / 133.0 | 6A / 7A (fifth) | 735 / 735 | +2.8 ms | 0.964 | 4.0 |
| Rainy Dayz | 126 / 126.0 | 10A / 10A | 941 / 941 | -4.7 ms | 0.948 | 10.4 |
| Zilog Z80 | 128 / 128.0 | 2A / 2A | 715 / 715 | +14.0 ms | 0.833 | 10.2 |
| Dub Tool 1 | 139 / 140.0 | 6A / 10A | 834 / 840 | -9.9 ms | 0.962 | 9.7 |
| Bipolar | 144 / 145.0 | 4B / 1A | 768 / 774 | -52.5 ms | 0.948 | 9.2 |
| Exos - Spliffmatch | 144 / 145.0 | 2B / 11A | 737 / 742 | -15.0 ms | 0.944 | 9.0 |
| Planetary Assault Systems | 103 / 148.1 | 8B / 5A | 339 / 488 | n/a (tempo differs) | 0.989 | 0.9 |
| Vertigo (Tale of Us) | 125 / 125.0 | 4A / 4A | 991 / 991 | +100 ms | 0.911 | 8.7 |
| Drax Copter | 123 / 124.06 | 5A / 5A | 840 / 847 | -49.6 ms | 0.968 | 10.3 |
| Bitch Kicked Me | 125 / 125.0 | 8A / 8A | 891 / 891 | -24.9 ms | 0.914 | 9.0 |

Per field:

- **BPM**: 5 of 10 identical, 4 within 1.1% (Python rounds to whole BPM, Rust is fractional and was fit against Rekordbox in Q30), 1 different tempo reading (Planetary Assault Systems, 103 vs 148, ratio 1.44; neither is an octave of the other, needs a look at that track against Rekordbox).
- **Key (Camelot)**: 5 exact, 1 fifth neighbour, 4 other. Different detectors (Essentia bgate vs cyanbox port); on the Beatport test split they score about the same (58.3% cyanbox). Expect different keys on about half the tracks.
- **Grid beats**: same count on 5 tracks, within 6 beats on 4 (follows the BPM difference). The phase differs by 3 to 100 ms: Python's grid is not Rekordbox-aligned, Rust's is fit to it (Q30). A fixed `bar_position = i % 4 + 1` in Python vs downbeat-aware phase in Rust, so bar numbers differ by design.
- **PWAV preview**: heights correlate 0.83 to 0.99 (median 0.95), mean absolute difference 1 to 10 of 31 levels. Different algorithm: Python's mean-of-peak preview vs the shared module's loudness envelope, which is the one the USB export writes and the app draws.
- **Colour waveform (PWV7 detail, PWV6 overview, exact PWV3/PWV5/PWVC)**: Rust uses the shared band module. The app already replaces the Python colour data with the same module (`band_analysis.rs`), so what is stored is identical in kind. Checked on Bipolar.wav (`rust_bands_equal_the_app_band_pass`, ignored test): same column count, under 1% of columns differ (decoder differences, symphonia 0.6 in the library vs 0.5 in the app). Python's own filter-bank colours are not used any more; against the raw Python colours the low band differs by 12 to 31 of 127 on average (correlation 0.84 to 0.94), mid and high correlate only 0.44 to 0.89. Column counts equal +1 (Rust has `ceil`).
- **Cues**: Python emits 1 to 5 memory cues per track (energy phrase segmentation); Rust emits none. **Energy**: Python 2 or 7, Rust none.

## Throughput through the coordinator (50 tracks)

`bench_analysis` (release) calls `run_track_analysis`, the function behind `analyze_tracks`, with a real library database and the Tauri-free sink: queue, backend choice, DB and ANLZ write per track. 50 tracks (every 12th file, 3 to 10 minutes, mean 6.2 min, wav, mp3 and flac). Peak RSS is the sum over the bench process and all its descendants, sampled every 100 ms.

| Backend | Parallelism | Tracks done | Wall | Tracks/min | Time to first result | Peak RSS |
|---|---|---|---|---|---|---|
| Rust | 8 workers x 1 thread | 50 of 50 | 1269 s | 2.36 | 200 s | 4.8 GB |
| Python | 3 processes x 2 torch threads | 40 of 50 (10 hit the 120 s timeout) | 1346 s | 1.78 successful | 55 s | 3.2 GB |

Read these with care:

- The `dev` box was shared and its load average was 40 on 16 cores during both runs, so every number is pessimistic. The first Rust run died on the old 120 s limit for the same reason, which is why Rust now has 900 s. The prototype measured 4.5 tracks/min for 8 single-thread workers on a quiet box (`pipeline-review-2026-10.md`); expect the app to be near that on an idle machine.
- Python's 10 timeouts are the app's existing 120 s limit meeting a loaded CPU; on an idle box Python finished each track in 31 to 46 s (parity run, 2 processes at once).
- Rust trades latency for throughput: with 8 workers every track takes about 100 to 200 s, so the first result arrives late. Python finishes its first track in under a minute. If first-result time matters more than total time, start with fewer, wider workers and widen later. Not done here.
- Memory: 4.8 GB at 8 workers (about 0.6 GB per worker). The pool cap (`available * 0.5 / 900 MB`) keeps this inside half of free RAM.

## Waveform gap: verdict

No gap against what the app writes today. The prototype's placeholder stage (per-track normalised FFT bands, 0 to 96/64/48 range) is gone from the library: the Rust analyzer feeds the decoded mono mix to `pioneer_usb_writer::waveform::Meter` and `analyze`, the same code that master uses for the Python path and for the USB export. Colour bytes, PWV6/PWV4, the `exact` PWV3/PWV5/PWVC columns and the PWAV preview all come from it. I wrote a port of the Python filter-bank waveform first (4th-order Butterworth bands, sqrt compression, fixed gains) and dropped it once master's fitted module landed. Remaining difference: the stored PWAV preview comes from the shared module in the Rust path and from the Python analyzer in the Python path (see the preview row above). Both are valid 400-byte previews; the Rust one matches the PWV3 column data exactly.

## What is left

1. **Cues and energy**: Rust leaves `cue_points` empty and `energy` `None`. The Python versions (`lexicon_cues.py`, `lexicon_energy.py`) are ports from Lexicon's analysis notes; I did not port them without a licence decision. The app shows the Rust results without auto memory cues. Needs a decision: port, replace with something permissive, or drop auto cues.
2. **First-result latency**: add an adaptive worker plan (few wide workers first, then widen).
3. **UI**: the backend setting and cancel have commands but no controls (the brief said no visual change).
4. **Bundling**: add the weights to `bundle.resources` and the resource dir to the lookup list (see above).
5. **Planetary Assault Systems tempo**: check 148 vs 103 against Rekordbox.
6. **macOS**: `available_memory_mb` uses `hw.memsize / 2` there (no `/proc`), a rough proxy; use `vm_stat` pages if that proves too tight or too loose.
7. **Decoder**: the library decodes with symphonia 0.6, the app's band pass with 0.5. Moving the app to 0.6 would make both paths bit-identical.
8. **Per-worker models**: one shared model instance serves all workers, not one per worker (immutable weights; see Design).

## Tests and validation

- Library: AnalysisResult mapping (bar-position phase, ms/tempo fields, key, waveform and `exact` columns, failed fit), pool plan limits, weights lookup, shared-band waveform tests; integration test on two generated 30 s WAV tracks plus a cancel test (`#[ignore]`, need weights: `cargo test --release -p fourfour-analysis-rs -- --include-ignored`, 3 of 3 pass); a non-ignored test that loading without weights fails with a clear error.
- App: backend choice rules, setting round trip (keeps other config keys), fallback to Python without weights and with an unloadable weights directory. Ignored: `rust_bands_equal_the_app_band_pass`.
- `cargo build --workspace && cargo test --workspace && (cd app && npm ci && npm test && npm run check && npm run build)`: all green (172 app tests, 0 check errors).

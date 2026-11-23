# Release profile (hc-L7)

Date 2026-11-20, Linux dev box (shared, load 25-35 during runs), rten CPU path (no ort/CoreML).

Chosen profile in the workspace `Cargo.toml`:

```
[profile.release]
lto = "fat"
codegen-units = 1
strip = "symbols"
```

`panic = "abort"` is deliberately not set: the app relies on `catch_unwind` to survive decoder panics.

## Method

- Each variant was built clean (`rm -rf target/release`) with the same command: `cargo build --release -p pioneer-test-ui --bin pioneer-test-ui -p fourfour-analysis-rs --example stage_cpu --example beat_windows_bench`.
- Variants were set through `CARGO_PROFILE_RELEASE_*` env vars; baseline is no profile.
- Bench set: 10 tracks, 120 s each (20 audio-min), taken every 40th file of `benchmark/datasets/beatport-edm-key/audio`.
- `stage_cpu` (3 internal reps per stage) and `beat_windows_bench` (1 worker, 1 thread, default windowed beat model) ran back to back per variant, rounds interleaved (base, thin, fat1 x3 rounds; base vs opt-level s x2 rounds). CPU time is user+sys from `/usr/bin/time`.
- Build times are cargo's own "Finished in" on a loaded box queued behind other workers' builds; compare them only as ratios.

## Results

| variant | app binary (unstripped / stripped) | stage_cpu bin | beat_windows_bench bin | clean build | beat_windows_bench CPU ms/audio-min | stage_cpu CPU s (3 reps, 20 min audio) | output identical |
|---|---|---|---|---|---|---|---|
| baseline | 48.7 MB / 35.8 MB | 6.6 MB | 13.8 MB | 7m34 | 7815 | 33.1 | reference |
| lto=thin | 49.0 MB / 36.5 MB | 5.8 MB | 13.5 MB | 8m09 | 7749 | 33.8 | yes |
| lto=fat, cgu=1 | 30.4 MB / 25.8 MB | 3.7 MB | 8.9 MB | 18m38 | 7736 | 32.0 | yes |
| lto=fat, cgu=1, opt-level s | 23.8 MB / 19.0 MB | 3.3 MB | 7.2 MB | 13m59 | 16058 (1 round) | 45.7 | not checked |

Stripped size is `strip` applied to the built binary (equals `strip = "symbols"`). Baseline cargo default already drops debuginfo.

## Findings

- Speed: no measurable change. Differences between base, thin and fat are within the 2-3% noise of the shared box (beat bench 7.7-7.8 s per audio-min for all three; stage_cpu 32-34 s). rten and the FFT crates dominate and are already optimized.
- opt-level "s" doubles beat analysis CPU (about 2x) and costs 40% on stage_cpu. Rejected.
- Size: fat LTO + codegen-units 1 cuts the app binary by 38% unstripped (48.7 to 30.4 MB), and strip symbols takes the total to 25.8 MB, 28% below the stripped baseline. thin LTO gives no size win.
- Cost: clean release build is about 2.5x slower (18m38 vs 7m34 under load).
- Identical output: stage_cpu FNV hashes (samples, key, resample outputs) identical for all 10 tracks in every run of base, thin, fat1. beat_windows_bench JSON identical after dropping the `ms` timing field (md5 equal across 3 runs each).
- `cargo test --workspace --release` with the chosen profile: exit 0, all 27 test suites ok.

## Mac to confirm

- DMG size: the Tauri bundle was not built here; expect roughly 10 MB less binary inside the .app, DMG compression shrinks that, so confirm the real delta.
- int8 speed: this box ran fp32 rten. The Mac ort/CoreML path and the int8 model were not exercised, so confirm analysis speed there (fat LTO should not matter for ort, which is a prebuilt dylib).
- Build time on the Mac release pipeline will rise about 2.5x.

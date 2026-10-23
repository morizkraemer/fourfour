# Analysis packaging decision (Q10), 2026-10

Status: decision doc plus a measured prototype. The choice is Merlin's. Nothing in `app/`, `pioneer-test-ui/`, `pioneer-usb-writer/` or `analysis/` was changed.

## The problem

The app analyzes by spawning `python -m fourfour_analysis analyze --backend deeprhythm_essentia` (`pioneer-test-ui/src/main.rs:255-266`, `:419-440`). `resolve_python()` bakes `CARGO_MANIFEST_DIR/analysis/.venv` into the binary at compile time and falls back to `python3`. A .dmg on a stranger's Mac has neither. Signals the app needs: BPM, beat grid (PQTZ), key, energy, mono preview waveform (PWAV), 3-band colour waveforms (PWV3/4/5/6/7), cues.

Two further facts shape the decision:

- Essentia and DeepRhythm are AGPL-3.0 (verified below), and the repo has no LICENSE. Shipping them decides the licence of what we ship.
- Current accuracy is modest (docs/analysis/benchmark-2026-10.md): key 54.0% exact on Beatport (598 tracks), BPM within 1 BPM of Rekordbox on 93.8% (128 unique tracks), but integer only; no beat-grid ground truth exists; about 20 s per 7-minute track.

## Verified licences (primary sources)

| Component | Licence | Source checked |
|---|---|---|
| Essentia | AGPL-3.0; "open licence, Affero GPLv3, for non-commercial applications", commercial licence on request from MTG/UPF | essentia.upf.edu/licensing_information.html; GitHub licence API; wheel metadata `AGPL-3.0-only` |
| Essentia pre-trained models | CC BY-NC-ND 4.0, commercial licence on request | same page |
| DeepRhythm 0.0.13 | AGPL-3.0 | PyPI wheel metadata classifier |
| librosa 1.0.0 | ISC | wheel metadata |
| torch 2.14.1+cpu | Apache-2.0 AND BSD-3/2 AND MIT (and others, bundled third party) | wheel metadata |
| numba / llvmlite / soundfile | BSD / BSD-2 + Apache-2.0 LLVM exception / BSD-3 | wheel metadata |
| PyInstaller | GPL-2.0-or-later with a bootloader exception (frozen apps may be any licence) | COPYING.txt in the repo |
| libKeyFinder | GPL-3.0 | GitHub licence API, mixxxdj/libkeyfinder |
| stratum-dsp 1.0.0 | MIT OR Apache-2.0 | `cargo info`, crate metadata |
| Beat This! code and weights | MIT. README adds: some training files are fully copyrighted or under limited Creative Commons licences, and "it is up to the user to assess whether this may impact their use case" | CPJKU/beat_this README, "License" section |
| beat-this (Rust port) 1.1.0 | MIT | `cargo info` |
| rten 0.24 / ort 2.0.0-rc.14 | MIT OR Apache-2.0 | cargo metadata (rten), `cargo info` (ort) |
| symphonia 0.6 (MP3, AAC, FLAC etc.) | MPL-2.0 (32 crates in the prototype tree) | cargo metadata |
| Rest of the prototype's 70-odd crates | MIT / Apache-2.0 / Zlib / BSD-3 mixes; no GPL or AGPL | cargo metadata (`--locked`) |

Not verified: the legal reading of AGPL for a Tauri shell that spawns an AGPL sidecar. The common reading is that a separate process talking via CLI and pipes is a separate program, but when both ship in one installer the safe assumption is that we must offer the sidecar's source and, to be safe, license the whole app AGPL-compatible. Ask a lawyer before relying on the separate-program reading. The Essentia commercial licence price and terms were not looked up.

## Options

All "guess" marks mean I did not measure and did not find a primary source.

| | A1 PyInstaller sidecar | A2 Relocatable Python runtime (uv / python-build-standalone) | B Rust-native, all stratum-dsp | C Hybrid: Rust DSP + ONNX Beat This! (+ stratum key) |
|---|---|---|---|---|
| What | Freeze the current CLI into a onedir sidecar (`externalBin`) | Ship a relocatable CPython plus the wheels as app resources, or download on first run | stratum-dsp (pure Rust) for BPM, key, beats; Rust port of waveform code | Beat This! via rten or ort for beats, downbeats, BPM; stratum key; Rust waveforms, energy |
| Install size | measured venv on Linux x86_64: **1.2 GB** (torch 708 MB, llvmlite 172, scipy 80+30, essentia 38). macOS arm64 smaller, guess 400-700 MB. DMG compressed, guess 40% of that | same as A1, minus the frozen-bundle overhead; or about 20 MB installer plus a 0.4-1.2 GB download | measured: binary **7.7 MB**, no models | measured: binary **7.7 MB** + models **83 MB** FP32 (0.27 MB mel + 83 MB), or 10.5 MB small model; int8 quantised FP32, guess about 21 MB |
| Cold start | torch + librosa + numba import and model load: benchmark first track 21-53 s | same | none measured; model-free | beat model load **0.14-0.26 s** measured (rten) |
| Per track | benchmark: about 20 s per 7-minute track, all features, DeepRhythm dominated | same | **10-16 s per 120 s clip on a loaded box**, stratum alone (see caveat) | stratum 10-16 s plus Beat This! 14-22 s per 120 s clip (loaded box). Published M4 figure: 4:32 in 4.6 s |
| BPM accuracy | within 1 BPM of Rekordbox 93.8%, integer only (benchmark) | same | stratum README: 87.7% within +-2 BPM on 155 Beatport/ZipDJ tracks, with about 12% octave/harmonic errors (self-reported, vendor tags). Not comparable to our benchmark | Beat This!: fractional BPM from beat intervals; paper reports state-of-the-art beat F1 (not re-verified here). No BPM ground truth on dev to score it |
| Key accuracy | Essentia bgate: 54.0% exact, 68.9% exact+relative+neighbour (benchmark, 598 Beatport) | same | stratum measured **25.0% exact, 45.8% exact+relative+neighbour** (120 Beatport clips) | stratum key is not good enough; needs a replacement key engine (not built) |
| Beat grid | none today (empty `beats`); DeepRhythm gives a global BPM only | same | stratum HMM grid, constant BPM | Beat This! beats and downbeats, can drive a variable grid |
| Licence consequence | AGPL-3.0 for the shipped Essentia + DeepRhythm; assume whole app AGPL-compatible, or buy Essentia commercial and replace DeepRhythm | same | none: MIT/Apache + MPL-2.0 (symphonia, file-level). App can be MIT/Apache | none beyond B: MIT (code and weights) plus the training-data caveat quoted above |
| Signing / notarisation | Every Mach-O in the onedir bundle must be signed with the hardened runtime. torch, numba/llvmlite JIT usually need `allow-unsigned-executable-memory` and `disable-library-validation` entitlements (general knowledge, not tested; no Mac here). Hundreds of dylibs | same, signing the whole runtime tree; downloaded-at-runtime variant avoids bundling but needs a download host and integrity checks | one small binary (or a library inside the app); standard Tauri signing. No JIT | same as B; ONNX models are data files, not executables |
| Cross-platform | per-OS freeze, 3 builds, torch wheels per arch (x86_64 and arm64 macOS) | per-OS runtime, same wheels problem; Windows DLL path issues | one codebase, `cargo build` per target | same as B; `rten` needs no system libs; `ort` needs libonnxruntime per OS |
| Maintenance | high: PyInstaller hooks break with torch/numba releases; pin everything | medium-high: pinned wheel set per OS | medium: owning DSP code; stratum-dsp is a young single-maintainer crate (guess on risk) | medium: owning DSP and the model files; Beat This! and the Rust port are both active (version 1.1.0, ported "with Claude") |
| Effort (days, guess) | 8-12 for macOS, plus 4-6 per extra OS | 6-10 for macOS, plus 3-5 per extra OS | 8-12: integrate stratum, port waveform, energy, cues; most risk in accuracy tuning | 12-18: B plus model plumbing, grid from beats, key option; prototype already covers decode, stratum, Beat This!, PWAV and 3-band waveforms |
| Accuracy vs today | equal | equal | key likely different, BPM unknown on our set, no grid truth | key like B, beats probably better than today (a grid exists at all) |

Other Rust-native candidates considered, not prototyped:

- aubio-rs: GPL-3.0 libaubio underneath (I did not re-verify), C library to link per OS, tempo and onset only, no key. Dominated by stratum-dsp for licence.
- libKeyFinder: GPL-3.0 (verified). Would force the whole app GPL. keyfinder-cli is a fallback in cratedrop (docs/research/synthesis-2026-10-15.md). Same constraint as AGPL, only slightly less contagious.
- Key CNN via ONNX Runtime (`ort`): I found no permissively licensed pre-trained key model. Essentia's key models are CC BY-NC-ND (verified), unusable commercially. Training our own on Beatport labels is a project, not a task. Not recommended before the rest ships.
- A clean-room Rust HPCP + profile key estimator (the Essentia `bgate` idea) is feasible from the literature, about 3-5 days, but whether it reaches 54% is unknown.

## Prototype numbers

Prototype: `prototyping/analysis-packaging/` (a separate Cargo workspace, not a member of the root workspace, not wired into the app). Dependencies, used only here: `stratum-dsp =1.0.0`, `beat-this =1.1.0` (features `decode`, `serde`; pulls in `rten 0.24`, `symphonia 0.6`, `rubato`), `rustfft 6.2`, `serde_json`. Models are fetched by `fetch-models.sh` (not committed, 90 MB).

Dev box, Linux x86_64, 16 cores, **load average about 40 during every run** (other workers compiling and running Python). Wall times are inflated; the figures show an upper bound. No `analysis/.venv` exists on this box and none was used. All inputs are the 120 s Beatport MP3 clips, read-only.

| Metric | Value |
|---|---|
| Release binary size (stripped, LTO) | 7.7 MB |
| Models | mel 0.27 MB, small beat model 10.6 MB, full beat model 83.2 MB (sha256 verified) |
| Peak RSS, 3 tracks, full model | about 1.4-1.6 GB (stratum plus rten); 0.6 GB with beats off |
| Beat model load (rten) | 0.14-0.26 s |
| Decode MP3 to 44.1 kHz mono (symphonia) | 0.35-0.8 s per clip |
| stratum-dsp BPM + key + grid | 9-16 s per 120 s clip (README claims about 0.2 s per 3 minutes; not reproduced, see caveat) |
| Beat This!, full model, rten | 21-24 s per 120 s clip (loaded box) |
| Beat This!, small model, rten | 13-15 s per 120 s clip (loaded box) |
| Waveforms: PWAV 400 bytes + 3-band detail (150 per second) + 1200 overview | 0.4-0.6 s per clip |
| Full prototype, one track, full model, loaded box | about 36-39 s; stratum + waveform only: 12-17 s |

Caveat on speed: the box was oversubscribed roughly 2.5 times, so wall and CPU seconds are inflated by an unknown factor. The stratum README figure is 70 times lower than what I measured, which cannot be explained by load alone. Possible causes (not investigated): the default `AnalysisConfig` runs the expensive multi-resolution tempo escalation, or the README number came from a different build or synthetic input. Re-measure on an idle box and on a Mac before trusting any speed figure.

Outputs on the three tracks (stratum | Beat This! full model). No BPM ground truth is available on dev for these tracks.

| Track | stratum BPM / key (Camelot) / beats | Beat This! BPM / beats / downbeats |
|---|---|---|
| Cecile - Talk Talk (Jr. Blender Remix) | 134.00 / 3B / 400 | 136.36 / 271 / 69 |
| Zoom - Five To Eleven | 140.68 / 3A / 116 | 130.43 / 194 / 63 |
| Taco & Da Mofos - Miracle Whip | 123.86 / 8B / 275 | 125.00 / 248 / 62 |

Observations, not conclusions:

- The two engines disagree on BPM by 2 to 10 BPM on all three tracks, so at least one is wrong on each; stratum's 116 beats on a 120 s clip at 140.7 BPM means its beat list is incomplete (expected about 280). Someone with Rekordbox references needs to score both.
- The small Beat This! model reported 62.5 BPM on Miracle Whip (half tempo) while the full model reported 125.0. Do not ship the small model without an octave check.
- stratum's grid on Zoom starts at 24.6 s (first beat), while Beat This! starts at 0.66 s. A grid that misses the first 25 s is unusable for PQTZ without correction.
- Beat This! produces fractional BPM, which removes the integer-only limitation noted in the benchmark.

### Key accuracy of stratum-dsp on the Beatport set (measured)

Every 5th clip of the 598-track clean Beatport manifest (120 tracks, 0 failures), Camelot labels from the manifest, Camelot computed from stratum's tonic and mode by the prototype (stratum's own `numerical()` uses a non-standard mapping and was not used).

| Class | Tracks (of 120) |
|---|---:|
| Exact | 30 = 25.0% |
| Relative major/minor | 2 |
| Camelot neighbour | 23 |
| Exact + relative + neighbour | 55 = 45.8% |

Largest error cluster: parallel mode swap (same tonic, other mode), 23 of 120. Essentia `bgate` on all 598 is 54.0% exact and 68.9% exact+relative+neighbour (benchmark doc). On a 120-track sample the 95% interval for 25% is about +-8 points, so stratum is clearly worse than Essentia on this set, not just noisier. The stratum README claims 72.1% exact on 68 tracks of its own set; it does not reproduce on Beatport 120 s previews (these clips are shorter than full tracks, which may hurt, unverified). Mean stratum time per clip with 6 parallel processes on the loaded box: 20.5 s.

Implication: stratum-dsp is not a key engine for us as it stands. Options C and B need a different key estimator (clean-room HPCP + profile, about 3-5 days, accuracy unknown) or a decision to accept a lower key bar.

## Recommendation

Recommend C (hybrid), reached in two steps, and make Merlin's licence decision first.

1. Choose the licence now. The cheapest way to stay permissive (MIT or Apache-2.0 for the app) is to drop Essentia and DeepRhythm from the shipped analysis path. Option C is the only listed option that does this and still produces a real beat grid. If Merlin accepts AGPL for the whole app, A2 (relocatable Python, same accuracy as today) is the fastest way to a shippable build, but it carries a 0.5-1.2 GB payload and the hardened-runtime signing work for hundreds of binaries.
2. Build C incrementally behind the existing JSON contract: a Rust `fourfour-analyze` binary emitting the same JSON as the Python CLI (so `parse_analyzer_stdout` and `python_result_to_analysis` keep working), shipped as a Tauri sidecar or linked as a library. Start with waveforms and Beat This! (already prototyped), then energy and cues. Key stays the open item: do not switch to stratum's key engine.

Reasons:

- It is the only option that removes the AGPL question and the Python runtime at the same time.
- Install size drops from hundreds of MB to under 100 MB (8 MB binary plus 83 MB model, or about 20 MB with a quantised model, guess).
- Beat This! gives a real beat grid and fractional BPM; today the grid is empty and BPM is integer.
- No JIT or dylib sprawl, so notarisation is routine.
- The Rust code needs no `resolve_python()` and works on Windows and Linux without per-OS Python builds.

Risks (the reasons this may be wrong):

- Key accuracy: stratum's key engine measured 25.0% exact on a 120-track Beatport sample against Essentia bgate's 54.0%. C is only viable if we write a better key estimator (clean-room HPCP, 3-5 days, accuracy unknown), accept the drop, or keep Essentia as an optional AGPL download that is not in the installer.
- Speed: measured speeds here are poor, though inflated by load. If idle-box numbers stay above a few seconds per minute of audio, analysis of a 5,000-track library becomes a multi-hour job and needs parallelism or a lighter configuration.
- No BPM or grid ground truth on dev; the Rekordbox ANLZ PQTZ comparison suggested in the benchmark doc is the prerequisite for any claim that C is "at least as good".
- Model weights are MIT, but the upstream README warns about training data licences. That risk is theirs to assess; it needs a line in `docs/legal.md`.
- stratum-dsp is a young crate with one maintainer (guess). It can be vendored (MIT OR Apache-2.0).

## Open questions for Merlin

1. Licence: permissive (MIT/Apache) and replace Essentia + DeepRhythm, or AGPL for the app, or buy an Essentia commercial licence? This decides A versus C.
2. Size budget: is a 100 MB download acceptable, or does it need to be under 50 MB (quantised model) or even under 20 MB (small model plus octave checks)?
3. Accuracy bar: is "at least as good as today" (54% key, 94% BPM within 1) required, or is "close to Rekordbox's 47% key" enough? Without a defined bar the key engine cannot be chosen.
4. Is it acceptable to ship analysis later and export with Rekordbox-less defaults (no analysis) first?
5. Do you want a Mac measurement of both stacks? Everything here ran on a loaded x86 box; Apple Silicon speeds and torch sizes are guesses.

## Reproduce

```bash
cd prototyping/analysis-packaging
./fetch-models.sh
cargo build --release
./run-3-tracks.sh                      # full model; MODEL_FLAG=--small, MODEL_FLAG=--no-beats also work
python3 -I key-subset.py 5 6           # every 5th of the 598 Beatport clips, 6 workers, key vs label
./measure-python-size.sh <empty-dir>   # measures the Python venv size for option A
```

Outputs land in `prototyping/analysis-packaging/results/` (git-ignored). `run-3-tracks.sh` expects the Beatport audio at `/home/merlin/dev/projects/fourfour/benchmark/datasets/beatport-edm-key/audio` (override with `AUDIO=`).

The Python venv measurement used `uv`, Python 3.12, `torch 2.14.1+cpu`, `essentia 2.1b6.dev1389`, `librosa 1.0.0`, `deeprhythm 0.0.13`.

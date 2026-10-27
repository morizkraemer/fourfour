# Tempo and key engine plan (player BPM / KEY become playback controls)

Status: plan only, no engine code. Branch `wp-tempo-plan` (from `q41-feedback`). Written 2026-10-27.

Goal: the player BPM field works like a CDJ tempo control (with Master Tempo / key lock, pitch stays), the KEY field shifts pitch in semitones independent of tempo. Today `playback.rs` has neither; the fields edit analysed metadata (`commitTrackBpm/Key` in `app/src/stores/track-edits.svelte.ts`).

## 1. Reuse from audio_tools / konsole (read-only survey)

| Source | What it is | Licence | Verdict |
|---|---|---|---|
| `konsole/` (fork of Magda, `UPSTREAM.md`) | C++23, JUCE + Tracktion Engine DAW | GPL-3.0 (`LICENSE`) | No code. Design reference only. |
| `konsole/magda/engine/clip/ClipStretcher.{hpp,cpp}` (908 lines) | Stretcher lives beside the stream, built on a non-RT thread; locate = reset + re-prime (no alloc, no file handle); stretcher owns its latency | GPL-3.0 | Read for design: it matches section 3. Do not copy. |
| `konsole/third_party/signalsmith-stretch/` | Vendored Signalsmith Stretch 1.3.2 + signalsmith-linear 0.3.1, unmodified, header-only | MIT | Konsole's default engine (`kSignalsmith`, `TimeStretchModes.hpp`). Same lib we would use. |
| `konsole/third_party/soundtouch/` | SoundTouch, second engine | LGPL-2.1 | Flag: LGPL (section 2). |
| `konsole/third_party/{farbot,moodycamel,rigtorp}` | C++ lock-free queues | MIT / BSD | Not needed; `ringbuf` + atomics already do this in `playback.rs`. |
| `audio_tools/refs/soundcraft/` (Rust, cpal 0.16, symphonia; notes in `audio_tools/docs/code-notes-soundcraft.md`) | Third-party DAW clone kept as reference. Stretch = linear varispeed in `render_clip` plus offline WSOLA (`crates/dsp/src/offline.rs`) | Apache-2.0 (its `LICENSE`) | Nothing worth porting. Its own notes list rayon in the callback and whole-session `mpsc` snapshots as RT hazards; fourfour's engine is already stricter. |
| `audio_tools/audioloop`, `audiocoder` | Node + Elementary Audio (JS) patch tools | n/a | Different runtime. Skip. |
| `audio_tools/refs/magda`, `konsole-wt/*` | More copies of the GPL tree | GPL-3.0 | Skip. |

Net: nothing ports as code. Reusable: the library choice, the placement idea (stretcher beside the stream, reset on locate), and one lesson: Konsole compiles the stretch TU with `-O2` even in Debug because Signalsmith misses deadlines at `-O0` (`magda/daw/CMakeLists.txt` ~L702).

## 2. Stretch engine options (permissive only)

| Option | Licence | Fit | Notes |
|---|---|---|---|
| **Signalsmith Stretch** via `signalsmith-stretch` crate 0.1.3 ([crates.io](https://crates.io/crates/signalsmith-stretch), [repo](https://github.com/colinmarc/signalsmith-stretch-rs); 142k downloads, updated 2025-09; C++ built by `build.rs`) | MIT ([upstream](https://github.com/Signalsmith-Audio/signalsmith-stretch)) | Recommended | No tempo parameter: feed N input samples, ask M output samples, so the ratio can change every call (what we need). `setTransposeSemitones/Factor` gives key shift. Quality "best between 0.75x and 1.5x" (upstream README). Reports `inputLatency()` + `outputLatency()`. Debug builds up to 10x slower. Apple Clang 16.0.0 miscompiles with `-ffast-math`; check the wrapper's flags on the Mac. Optional faster FFT (Accelerate, IPP, PFFFT). |
| Bungee via `bungee-rs` 0.1.1 ([digger](https://rust-digger.code-maven.com/crates/bungee-rs), [site](https://bungee.parabolaresearch.com/)) | MPL-2.0 (from a vcpkg manifest; confirm in repo `LICENSE`) | Fallback | C++ adaptive phase vocoder, real time, any ratio. File-level copyleft: fine if unmodified and notice shipped. Binding is young (submodule build). Needs an A/B. |
| SoundTouch ([licence](https://queue.cppget.org/soundtouch?f=full)) | LGPL-2.1 | Avoid | Static link in a Tauri binary conflicts with the relink duty; time-domain (WSOLA), cheap, rough on key shift. Only as a replaceable dynamic lib. |
| Rubber Band | GPL | Excluded | |
| Pure Rust | n/a | None production-ready found | `kithara-stretch` 0.0.1-alpha5 is a contracts/adapters crate, not an engine (unverified). |
| In-tree varispeed (cubic resample) | ours | Always ship | Key lock off = plain varispeed like a CDJ without Master Tempo: exact, near-free, no artefacts. |

Not measured yet (no Mac, no audio on this box): transient quality, CPU at 44.1/48 kHz, latency. Slice 0 measures them. Expectation: vocoders smear hi-hats and kicks beyond about +-16 %; fine at +-6/10 %. No source states real-time allocation safety of the wrapper; the alloc test proves it.

Recommendation: Signalsmith for key lock and key shift; in-tree varispeed when lock is off and shift is 0; Bungee as the swap-in if the A/B prefers it. A small `Stretcher` trait (`reset`, `latency_frames`, `process(in, out)`) keeps the swap cheap.

## 3. Engine changes (`pioneer-test-ui/src/playback.rs`)

Placement: **in the cpal callback, after the ring, not in the decode thread.** The ring is about 2 s (`RING_SAMPLES`); a stretcher on the producer side would delay every tempo/key change by up to 2 s. After the ring, a change is audible within one block plus stretcher latency. The ring stays in source time, so decode, seek, cue and loops stay untouched.

- `RenderState` gains the stretcher and fixed scratch buffers, built in `RenderState::new` (non-RT thread) and moved into the callback with the stream.
- Per block: smooth the rate, take `n_in = round(n_out * rate + carry)` source frames from the ring (carry keeps the fraction), run stretcher or varispeed, write `n_out` frames. Scratch is sized for the max rate (1.5x) times the largest device buffer; bigger blocks are chunked.
- Parameters: `tempo_ratio`, `key_semitones` (f32 bits) and `key_lock` (bool) as atomics in `SamplePipe`, set by new commands `playback_set_tempo`, `playback_set_key_shift`, `playback_set_key_lock`. No locks.
- Smoothing: one-pole slew on the rate per block (about 30 ms) so fader drags do not zipper; transpose slewed about 50 ms.
- Modes: ratio 1 and shift 0 = bypass, bit-exact with today. Lock on, or shift != 0: stretcher with time ratio = rate, transpose = 2^(shift/12). Lock off and shift 0: varispeed. Lock off with shift: stretcher with transpose = rate * 2^(shift/12). Mode switches reuse the 5 ms declick crossfade (`seam`, `DECLICK_FRAMES`).
- Position in source time: `frames_played` becomes source frames consumed (fraction kept), minus the stretcher's `inputLatency`, so the playhead matches what is heard. `clock_position_ms` and the tick thread keep working; the tick also reports `tempo_ratio`. Everything the frontend stores (beat grid, waveform playhead, hot cues, seek targets) stays in source ms. Only displays showing BPM or time-left scale by the ratio.
- Seek / cue / loop: seek already flushes the ring and bumps the generation; the callback also resets the stretcher (reset clears buffers, no alloc). Pre-roll: decode starts `latency` source frames early and the callback drops that much output so a CUE start is tight; if fiddly, accept warm-up under the existing declick. Loops (frontend seek at loop end) are unaffected: they live in source time.
- Hot cues and CUE button: stored in source ms, they do not move with tempo. CUE-hold preview uses the same path, so it plays at the current tempo/key; stretcher latency adds to its response (measure in slice 0 against the 128-frame buffer).
- Beat grid: source ms. The waveform keeps drawing in source time. Effective BPM = analysed BPM * ratio. Grid edits (nudge, set downbeat) still edit source-time beats.
- Tests in `playback_tests.rs` on the fake device:
  - extend `render_callback_never_allocates` with tempo 1.16, key +5, lock on/off, mode switches, seek (0 allocs, 0 frees; also proves the crate's `process` is alloc-free);
  - bypass bit-exact at ratio 1;
  - 10 s of source at ratio r yields 10/r s (+-2 ms); reported position within 20 ms of truth;
  - 1 kHz sine at key +12 comes out at 2 kHz (+-1 %); at ratio 0.9 with lock on it stays 1 kHz;
  - tempo change audible within 100 ms; seek during a tempo change never plays pre-seek audio (reuse the ramp test).

## 4. UI / UX

- Range presets on the player: +-6 / 10 / 16 / Wide (+-50 %), CDJ style; default +-10. The range limits drag/steps; typed values may use Wide.
- BPM field: `ScrubValue` with a new `tempoScrub` model next to `bpmScrub` in `prototyping/viewport/design-system/utils/scrub.js`. Shows effective BPM live. Drag/wheel = 0.1 % step, shift = fine 0.01 %, alt = coarse 1 %. Typing a BPM sets the ratio that hits it. Chip beside it: `+4.2 %` and dim `orig 124.0`. Double-click (or a Cmd+0 `model.actions` entry) resets tempo.
- KEY field: shows the effective key (reuse `keyColor`; +1 semitone = +7 steps on the Camelot wheel), chip `+2 st`, range -12..+12, integer steps, double-click resets. With lock off the effective key follows tempo (dim chip, e.g. `+0.7 st`).
- `MT` (Master Tempo) toggle next to the tempo field, default on.
- A small reset glyph shows only when tempo or shift is off identity (one click clears both).
- Metadata vs playback: the **inspector** keeps editing analysed BPM/key, labelled "Analysed BPM / Key" with the existing lock icon for user-set values. The **player** stops writing metadata (drop `commitTrackBpm/Key` from `Player.svelte` and `ExpandedPlayer.svelte`). Player fields get the accent tint only while off identity, so "not the track's own value" is visible at a glance. One explicit right-click item on the player BPM, "Set analysed BPM to current", is the only bridge back to metadata.
- State: `player.tempo` ({ ratio, keyShift, keyLock, range }) in `player.svelte.ts`, pushed to the engine on change and on track load.

## 5. Slices

| # | Slice | Effort | Tests |
|---|---|---|---|
| 0 | Spike: `bench_stretch` bin (like `bench_analysis`), Signalsmith vs Bungee on 5 tracks, +-8/16/50 %, key +-3; WAVs for Merlin to hear | 0.5 d | CPU % of one core at 44.1/48 kHz stereo, buffers 64/128/256, release build; `[profile.dev.package.*] opt-level=3` for the stretch crate (root Cargo has no profile today) |
| 1 | Engine: atomics, slew, varispeed + stretch path, source-time position, `Stretcher` trait, 3 commands | 2-3 d | extended alloc test, length/pitch/position tests, bypass bit-exact |
| 2 | Seek/cue pre-roll, mode-switch declick, tick reports ratio | 1 d | seek-during-tempo test, CUE latency on the fake device |
| 3 | Frontend: store, tauri wrappers, `tempoScrub`/`keyShiftScrub`, presets, MT, reset, effective BPM/key | 2 d | vitest for scrub models and effective BPM/key math (`player-grid.test.ts` style), svelte-check |
| 4 | Inspector relabel, remove metadata edit from player, bridge menu item | 0.5 d | adjust track-edits tests |
| 5 | Mac pass: real device, 32/64/128 frames, long mix, listening | Merlin 1 h | checklist like `audio-engine-2026-10.md` "Needs the Mac" |

Total about 6-7 days; slice 0 gates the engine choice.

## 6. Risks

- Stretch CPU in Debug and on weak laptops (dropouts): bench first, optimise the dep in Debug, fall back to varispeed if load exceeds about 25 % of a core.
- Wrapper alloc in `process` undocumented: the alloc test decides; a failure sends us to Bungee or a pre-allocated wrapper.
- C++ toolchain in the build (Xcode CLT, MSVC, g++): new CI/packaging friction; Apple Clang 16.0.0 `-ffast-math` bug.
- Vocoder quality on transients at Wide range; set expectations per range.
- Latency accounting drifts if stretcher latency depends on rate; measure in slice 2.
- Grid lines stay in source time while playhead speed changes: correct but verify visually with a worker.

## 7. Open questions for Merlin

1. Does tempo reset on each track load (CDJ), or persist per session / per track?
2. Key lock on by default (rec), and is plain varispeed (lock off) wanted at all?
3. Wide = +-50 % (rec), or +-100 %?
4. Keep the "Set analysed BPM to current" bridge in the player menu, or inspector only?
5. Key shown as Camelot only, or musical (Am), following the library setting?
6. OK to add a C++ build dependency (MIT, via `build.rs`) for Signalsmith?
7. Should row/hover previews follow the player tempo, or always play at 1.0?

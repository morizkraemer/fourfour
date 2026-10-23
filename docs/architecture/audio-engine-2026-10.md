# Audio engine review and hardening (Q22, 2026-10)

Scope: `pioneer-test-ui/src/playback.rs` (decode thread, ring buffer, cpal callback, tick thread) and the `playback_*` commands. Branch `q22-audio`. Reviewed on the `dev` box (no audio device); everything that touches a real device is listed under "Needs the Mac".

Design ideas compared (ideas only, no code taken): rbxport (generation-tagged blocks, callback only reads the ring, allocation-count test) and cyanbox (no alloc or locks on the RT thread, feed thread around the playhead). The engine already had the "callback only pops from the ring" shape. It lacked the generation handshake, the allocation test and a device-free test path. Now it has all three. A persistent feed thread was not built (see "Left").

## Findings, ranked

Only F1 was checked by mutation (removing the fix makes the new test fail). The others come from code review and are covered by regression tests on the new code. They were not reproduced on the old code.

| # | Severity | Finding | Status |
|---|---|---|---|
| F1 | High | Seek/stop race. The decode thread pushed sample by sample and only checked its generation before each push. The callback cleared the ring when it saw a new stream generation. A push could land after the clear (stale sample), or new-stream samples could be cleared mid-frame. One stale or lost sample swaps left/right until the next flush. | fixed: generation bump under the producer lock, and the callback echoes `ack_generation`; the decode thread waits for the echo before its first push. Tests: `seek_never_outputs_audio_from_before_the_seek`, `audio_decoded_before_the_callback_runs_is_not_flushed_after_a_seek` (fails deterministically without the ack), `rapid_seek_spam_lands_on_the_last_target` |
| F2 | High | Underrun swapped channels. The callback popped per sample, so a ring that ran dry between L and R shifted every later frame by one sample. | fixed: producer pushes whole frames, callback pops only whole frames. Test: `channels_stay_paired_through_underruns` |
| F3 | High | Symphonia panics on a malformed header (zero sample rate: `TimeBase cannot have 0 numerator`). `play()` without a duration hint probed the file on the command thread, so one bad file could take down the command (main) thread. | fixed: probe and decode wrapped in `catch_unwind`, reported as errors. Tests: `corrupt_files_report_errors_without_panicking` (also without hint), `mutated_headers_never_panic_the_decoder` (400 mutated files) |
| F4 | High | Decode failures were only `eprintln!`; the UI saw nothing. A file shorter than its tagged duration (truncated, wrong header) stayed "playing" forever, because the end was detected only by position >= duration. | fixed: `playback-error` event, and drain detection (decode ended and ring empty gives `playing=false`). Tests: `truncated_file_ends_playback_instead_of_hanging`, corrupt-file test |
| F5 | High | A failed device switch left the engine silent. `rebuild_output_stream` replaced the pipe before the new stream opened. On failure the old stream was muted by the epoch bump, the new pipe had no consumer, and `output` still looked valid. Later plays decoded into a ring nobody read. | fixed: old stream dropped first, `output` stays `None` on failure, next play retries. `PlaybackEngine::new()` no longer fails without a device (an unmanaged Tauri state would panic every command). Test: `failed_output_open_returns_an_error_and_recovers` |
| F6 | Medium | Device disappearing: the cpal error callback only printed. Position froze, UI said playing. | fixed: error event, `playing=false` on the next tick, next play reopens the output. Test: `device_error_is_reported_and_next_play_reopens_the_output` |
| F7 | Medium | `seek()` read path and duration, released the lock, then called `play()`. A `stop()` in between was undone: seek restarted the stopped track. | fixed: one mutex; `seek`/`resume` start under the same lock as their read. Covered by the multi-thread spam test |
| F8 | Medium | Stale position. The tick thread read `frames_played` before the callback reset it, so right after a seek the playhead jumped to old count + new start. | fixed: position uses the frame count only after the callback acked this stream. Test: `ticks_do_not_report_the_previous_streams_position_after_a_seek` |
| F9 | Medium | Resampler restarted its phase per packet and rounded each packet up with `ceil`. About 0.04% too long (about 100 ms drift over a 4 min track at 44.1 to 48 kHz) plus a small seam at every packet. Beat grids drift against the audio. | fixed: streaming linear resampler keeps phase and last frame. Test: `resampler_is_packet_size_independent_and_drift_free` (480000 +/- 2 frames for 10 s) |
| F10 | Medium | Ring underrun cut to hard zero, and recovery started at full level (click). | fixed: 2 ms fade when dry, 5 ms crossfade on resume. Test: `running_dry_fades_instead_of_cutting_off` |
| F11 | Medium | Pause killed the tick thread, so the level meters froze at their last value. | fixed: tick thread keeps running until the paused meters decay to zero, then exits (this also stops the endless 20 Hz ticks after end of track) |
| F12 | Low | `play()` on a missing file returned Ok and failed later in the thread. | fixed: synchronous `Open <path>: error` from the command |
| F13 | Low | Per-sample mutex lock plus 250 us sleep in the push loop; per-packet `Vec` allocations in decode (not RT). | fixed: slice push, reused buffers |
| F14 | Low | Channel count and sample rate came from codec params with a stereo default; a file without `channels` in its params was mapped wrongly. | fixed: use the decoded buffer's spec |
| F15 | Low | Mid-file I/O errors were silently treated as end of file. | fixed: only `UnexpectedEof` ends quietly; other read errors become an error event (the stream still drains and ends) |
| F16 | Low | No bound on repeated `ResetRequired`; the loop could spin. | fixed: aborts after 8 in a row |
| F17 | Low | No thread accounting or shutdown: threads were detached. | fixed: `live_thread_count()`, `shutdown(timeout)`, `Drop` stops and waits. Test: `stop_and_drop_join_all_threads` |

Checked and fine: no `unwrap`/`expect` on hot or external paths (mutex poisoning is now ignored instead of returned as an error); the callback takes no lock and makes no syscall; no priority inversion (only non-RT threads take the producer lock, the callback never does); very long files use `u64`/`u128` position math, so no overflow.

## What changed

- `OutputBackend` and `PlaybackEvents` traits. `CpalBackend` is the only real backend. `AppHandle` implements `PlaybackEvents` (`playback-tick`, new `playback-error`). Tests use a fake device and a recording event sink. Command names and arguments are unchanged.
- `RenderState::render` is the whole callback. One `Mutex<Inner>` replaces four locks.
- Frontend (no visual change): `player.error` state (nothing displays it yet), `subscribePlaybackError`, play/seek rejections are caught and reset `playing`, so the player cannot get stuck "playing" after an error.

## Tests (26 new, in `pioneer-test-ui/src/playback_tests.rs`)

Decode: native rate, resample + upmix, seek offset, superseded decode, sink error mid-stream. Resampler: packet independence and drift. Engine on the fake device: play to end, seek without stale audio (up and down ramps, 7 seeks), decode ahead of the callback, tick position after a seek, channel pairing, underrun fade, truncated file, six corrupt files (garbage, empty, header only, zero channels, zero rate, huge data length), 400 mutated headers, device error then recovery, failed open then recovery, missing file, rate changes (44.1/96/48 kHz, and a device that ignores the requested rate), pause/resume, thread join, 300-seek spam, a 6-thread load/seek/stop/pause/resume spam with concurrent `configure` and a running device (2.5 s run, 20 s deadline), allocation count, latency.

The allocation test uses a counting global allocator armed only on the thread that calls the callback. It covers steady playback, seek flush, pause and stop: 0 allocations and 0 frees.

Run: `cargo test -p pioneer-test-ui --bin pioneer-test-ui playback -- --nocapture`.

## Latency (fake sink, debug build, 4 min 44.1 kHz stereo WAV)

| Path | min | median | max |
|---|---|---|---|
| load (with duration) to first audio | 1.6 ms | 1.6 ms | 1.6 ms |
| load (no duration hint) to first audio | 1.6 ms | 1.7 ms | 1.7 ms |
| seek to first audio of the new position (8 seeks) | 1.3 ms | 2.5 ms | 3.1 ms |

These exclude the device buffer (128 frames is about 2.7 ms), the 5 ms declick crossfade and the IPC hop. WAV needs no entropy decoding and the file sits in the page cache, so expect MP3/FLAC/AAC and cold disk to be slower. Measure on the Mac. Each play/seek still re-opens and probes the file (see "Left").

## Needs the Mac (real device)

1. `cd pioneer-test-ui && ./dev.sh`, load a 44.1 kHz MP3 and a 48 kHz FLAC, press play. Expect audio within about 100 ms and a playhead that moves without jumping.
2. Scrub hard on the waveform for 10 s while playing. Expect no pops beyond the soft seek crossfade, no old audio after you stop, and left/right never swapped (use a track with a hard-panned instrument).
3. Pause, resume, CUE hold and release. The meters must fall to zero after pause.
4. Unplug headphones or the USB DAC while playing. Expect `playing` false within about 50 ms, a `playback error: ...` line in the console, no crash. Replug and press play: audio resumes without restarting the app.
5. In settings change the output device, sample rate (44.1/48/96 kHz) and buffer size with a track loaded. Expect no crash. A failed switch gives an error, and the next play works. In Audio MIDI Setup the device rate should follow the setting.
6. Play a truncated MP3 (`head -c 1000000 song.mp3 > cut.mp3`) and a renamed text file (`cp README.md fake.mp3`). Expect an error in the console, no hang, `playing` false at the end.
7. Open a 2 hour mix, seek to the end and back to the start. Note how long the first audio takes (expected well under 0.5 s; report if not).
8. Listen for dropouts at 32 and 64 frame buffers. The callback has no alloc or locks, so dropouts would point at the device or system load, not at this code.

## Left

- A persistent decode worker. Every play/seek spawns a decode thread that re-opens and re-probes the file, so scrubbing is a burst of short threads. Superseded threads exit at their next check, but the probe and demuxer seek are not interruptible. A worker that keeps the demuxer open and coalesces requests would cut seek latency on large MP3/FLAC files (the cyanbox feed-thread idea). Not done: needs real files and timings on the Mac first.
- Sync Tauri commands run on the main thread. That keeps seek order (good) but `configure` and `set_output_device` block the UI while the device opens. Making them async would reorder rapid seeks, so left as is.
- Linear interpolation resampling is fine for preview, not for mastering-grade playback.
- Nothing displays `player.error`; user-visible UI is Merlin's call. The event and state exist.
- The error callback and `Drop` shutdown are untested on a real CoreAudio device.

# Writer bugs, October 2026

For the `pioneer-usb-writer` owner. Found by test-only work on branch `q27-writer-repro`; no writer source was changed. Repro tests are `#[ignore]`d so `cargo test --workspace` stays green.

## 1. PDB page split breaks past ~192 tracks

- Where: `pioneer-usb-writer/src/writer/pdb.rs`, `split_into_pages` (line 124); heap offsets are `u16` (`offsets.push(heap.len() as u16)` at lines 448-565); rebase at line 173.
- Root cause: each table's heap and `offsets: &[u16]` span the whole table. Once the heap passes 64 KiB, `heap.len() as u16` wraps, so offsets stop being monotonic. In debug, `heap_end_offset - heap_start_offset` (line 150) panics with "attempt to subtract with overflow". In release, the slice at line 170 panics with "slice index starts at 65360 but ends at 168" (matches the report).
- Repro: `cargo test -p pioneer-usb-writer --test writer_scale_repro -- --ignored` (tests 200, 500, 2000 and long strings). Release also fails: add `--release`.
- Measured threshold (track table, one track row about 340 bytes):
  - Short or 30-char strings: 191 tracks pass, 192 fail.
  - 100-char title/artist: 174 fails.
  - 200-char title/artist: 137 fails.
  - So long strings do lower it; the 100-track case passes only up to the 100-char test.
  - Measure again with `cargo test -p pioneer-usb-writer --test writer_scale_repro report -- --ignored --nocapture`.
- Suggested fix (not applied): build rows per page instead of per table. Keep offsets as `usize` (or per-page `u16` relative to the page heap) during build, and cut a new page when the next row would not fit. Convert to `u16` only after rebasing. Applies to every table using `split_into_pages` (lines 903-913), not only tracks.

## 2. ANLZ colour-detail index overflows u32

- Where: `pioneer-usb-writer/src/writer/anlz.rs`, `(i * src_len / entry_count) as usize` at lines 258 (PWV3), 314 (PWV5), 364 (PWV7), 404 (PWV6 path). Lines 283, 332, 469 are `i * 400 / n`, safe unless entry counts exceed ~10 million.
- Root cause: `i` and `src_len` are `u32`, so `i * src_len` reaches about `entry_count^2`. That exceeds 2^32 once `entry_count > 65536`, i.e. `duration_secs * 150 > 65536`. Debug build: "attempt to multiply with overflow". Release: wraps, so source indices are garbage and PWV5 amplitude values come from the wrong detail entries (first bad entry at 600 s is 47722).
- Repro: `cargo test -p pioneer-usb-writer --test anlz_long_track_repro -- --ignored` (600 s and 1200 s tracks, plus `--release`). The 300 s control test passes.
- Measured threshold: 437 s (7 min 17 s) fails; 436 s passes (65550 entries at 437 s; theoretical 436.9 s).
- Suggested fix (not applied): compute `(i as u64 * src_len as u64 / entry_count as u64) as usize` in all four spots, or factor one `resample_index(i, src_len, entry_count)` helper. Section lengths (`entry_count * 3`) stay far below u32 for realistic tracks.

## Other writer findings reported this run

Sources are other workers' reports. Those docs and findings were not on this branch's base, so they were not re-verified here.

- `parse_pco2` assumes 88-byte entries and drops names and colours: `docs/architecture/rekordbox-import.md`.
- SQLCipher keys are private/unavailable for the library DB: reported by the rekordbox-import worker.
- There is no PDB reader (only a writer; tests parse the PDB by hand): reported by the same worker.
- Writer proposals for multi-USB sync: `docs/architecture/multi-usb-sync.md`.

# perf-scan: cold import reads fewer bytes

Branch `perf-scan` (on top of `hc-fuzz`). Box `dev`, load 6-17, everything `nice -n 10`.

## Finding

The r6 note ("a cold 10k import reads 2.7 GB of ID3 pictures, seek-past reader in scanner.rs") is out of date: R92 (`tag_scan.rs`, 2026-11-03) already replaces picture bodies with zeros for mp3, aiff, wav and m4a, so the first tag pass never reads them. The measured 2.7 GB (2.86 GB in these runs) was two things:

- the tag pass (527 MB): about 53 KB per file, nearly all kernel readahead (128 KB window) around small tags;
- the background cover pass (2.33 GB): reads 8,132 covers (about 1.5 GB of pictures) plus readahead.

Covers are read after the rows are committed (`store_pending_covers`, background fill, also at launch and before a sync). That timing is unchanged; only the bytes around the tags and covers were cut.

## How it was measured

- Fixture: `~/.cache/ff-fixture/10k/music` (10,000 files, 7.3 GB). Cold = `posix_fadvise(DONTNEED)` on every file before each run (same as `scripts/perf.sh evict`), no sudo.
- Bytes = `read_bytes` from `/proc/self/io` (real disk reads, page cache hits not counted), printed by `bench_startup import` as `disk_read_bytes:` and in the PERF json (`scan_disk_bytes`, `covers_disk_bytes`).
- Per-format split: new `bench_tags` (one thread, per-file delta) on a fixed 1,500-file sample.
- A/B: base = binary built before the change, interleaved, 3 rounds.

## Steps

| # | Step | Result | Kept |
|---|---|---|---|
| 0 | baseline | scan 526.6 MB / 845 ms median, covers 2,330 MB / 18.3 s, total 2,857 MB | |
| 1 | `posix_fadvise(RANDOM)` on the tag-pass file (Linux): readahead off, so only the pages lofty asks for are read | 1,500-file sample 79.7 MB to 33.4 MB; 10k scan 526.6 MB to 241.5 MB (-54%), scan 845 ms to 752 ms median | yes |
| 2 | one open per file: guard (hc-fuzz) and tag read share the fd; `scanner::track_from_reader` (non-generic, so lofty stays compiled at opt-level 3 in dev builds) replaces the second copy of the field extraction in `tag_scan.rs`; the flac/mp4 flood guard now also covers the import tag pass (before it only ran in the fallback) | sample 36.1 MB (guard included), same fields | yes |
| 3 | `fadvise(RANDOM)` in the cover pass too (`read_track_with_cover`) | covers 2,330 MB to 1,428 MB (-39%); covers 18.3 s to 19.6 s (+7%, 8 KB reads without readahead) | yes (bytes win, background job) |
| 4 | 256 KB buffer in the cover pass | covers 3,828 MB, 64 s in the first run | no |
| 5 | 32 KB buffer in the cover pass | covers 1,663 MB, no time gain | no |

Kept result for the cold 10k import: **2,857 MB to 1,669 MB read (-42%)**; tag pass 527 MB to 241 MB (-54%) and 845 to 752 ms median on a quiet disk (-11%). Rows are in the library after the tag pass, so that is the number that gates the UI. The cover pass is CPU/db bound on this disk and 1 s slower; on a contended disk the bytes decide (the first baseline run on a busy disk took 65 s for the tag pass and 59 s for the covers).

Per format, tag pass, 1,500 sample (KB per file, before / after step 2): mp3 48 / 25, flac 58 / 22, m4a 63 / 28, aiff 57 / 20, wav 49 / 12.

## Real-tracks subset (`benchmark/datasets`, read-only)

Only the first cold read per file counts (the fadvise did not drop pages of these files on later rounds, so 2 of 3 rounds read 0 bytes and are discarded). First cold run, 400 real mp3 (beatport-edm-key), base: tag pass 21.3 MB (53 KB per file), same as the fixture. A cold run of the new reader on a disjoint set was not comparable (different file mix, partly cached), so no real-files after-number is claimed beyond the 10k fixture.

## Same output

`read_track_metadata` / `read_track_tags` / `read_track_with_cover` return the same fields. Tests in `tag_scan.rs`: `large_covers_in_every_format_leave_the_tags_and_the_stored_cover_unchanged` builds 400 KB covers in mp3 (APIC), flac (PICTURE), aiff and wav (ID3 chunk) and checks the full read keeps the cover bytes, the tag read equals the full read minus artwork, and the cover read equals the full read; the existing m4a (covr) and broken-tag fallback tests still pass. `tag_floods_are_refused_by_the_import_reader_too` covers the guard on the new paths.

## Not done / needs the Mac

- macOS: `fcntl(F_RDAHEAD, 0)` was tried and dropped 2026-11-22: on the Mac the quick tag read of a 64 MB sparse file took 3.42 s vs 0.34 s for the plain reader (`sparse_zero_files_read_as_fast_as_the_plain_reader`). macOS keeps default readahead; the Mac byte saving is open.
- Reading the cover only (no other tag) would need a lofty change; floor for the cover pass is about 1.5 GB of pictures.

Gates at the last commit: `cargo test --workspace` 655 passed, 0 failed; `cargo check -p pioneer-test-ui --features dev-bins --bins` clean. No `app/` change, so vitest, svelte-check and Playwright were not run. Rejected 2 of 5 steps (rows 4, 5), no streak.

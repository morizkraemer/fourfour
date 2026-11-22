# hc-L5 fuzz robustness

Harness: `fourfour-analysis-rs/examples/fuzz_corpus.rs` (opt-in example, not part of `cargo test`).

```
FOURFOUR_MODELS_DIR=~/.cache/fourfour-models cargo run --release -p fourfour-analysis-rs --example fuzz_corpus -- \
  --seeds 200 --muts 50 --analyze-every 1 --shards 3 --work ~/.cache/hc-L5-work
```

Bases: 6 s excerpts of one real mp3 and one real wav (GiantSteps+, FSL10k) encoded with ffmpeg to mp3, flac, wav, m4a, aiff, ogg (12 bases). Case `id = seed * 50 + mutation`; seed picks the base, mutation picks one of 10 kinds (truncate, bitflips, zero, garbage, header-len, swap-ext, cut-flip, dup-chunk, header-only, delete), RNG fixed by (seed, mutation). Each case runs decode, tag scan (lofty), library import (`add_track`), `file_status` and optionally the analyzer, in a child process (`ulimit -v 16 GB`, nice 10). A case is a failure on panic (caught), no END line within 30 s (hang), child crash, a single allocation of 2 GB or more (refused by a counting allocator), or a peak above 2 GB over the case baseline. Failing inputs are saved to `<work>/fails`.

| step | change | cases | analysed | panics | hangs | crashes | allocs >2 GB | slowest | peak over base |
|---|---|---|---|---|---|---|---|---|---|
| 0 | baseline, no fix | 10000 | every 25th (400) | 0 | 0 | 0 | 0 | 6.9 s | 40 MB |
| 1 | same corpus, analysis on every decodable case | 10000 | all decodable | 0 | 0 | 0 | 0 | 9.9 s | 40 MB |

Failures by format: none for mp3, flac, wav, m4a, aiff, ogg. Outcomes in step 1: 2619 ok, 7381 clean errors.


## Extension (profiles)

`--profile base|hostile|odd|long|real`, same seeds and mutation kinds plus an `intact` kind (the crafted file unmutated). Per case the TSV records time and allocation peak for decode, tag scan and import, and the process peak RSS (VmHWM).

- hostile: 74 crafted files from one 6 s track. mp3: ID3 tag sizes claiming 50 MB to 4 GB, real 20 MB APIC, 100k and 200k repeated frames, invalid UTF-16, huge extended header, 4 MB unsync, ID3v2.2 PIC, footer, 5000 stacked tags. flac: Vorbis comment count/vendor/length at 2^32-1, 1M real comments, PICTURE lengths at 2^32-1, two real 16 MB pictures, 200k tiny blocks, 16 MB padding, ID3 prefixes. wav and aiff: ID3 chunks (claim 500 MB, real 20 MB, stacked), chunk sizes at max, 100k chunks, container size 0 and max, bext, cue, COMT, MARK. m4a: covr claiming 500 MB, real 20 MB, largesize 2^63, size 0 and below 8, 10k nested meta, 20k flat deep, 1M empty atoms, 200k ilst items, moov size max and 1, stsz/stts/stco/stsc counts at max, timescale 0. ogg: comment count, vendor length and comment length patched with CRC fixed.
- odd: 30 s, 8 kHz, 11 kHz, 44.1 kHz mono, 48 kHz 6 ch, 96 kHz, 192 kHz where the encoder allows (36 bases).
- long: 15 min mp3, flac, m4a, ogg and 15 min 16 kHz mono wav and aiff (105 MB flac).
- real: 6 GiantSteps previews (2 min) and 2 FSL10k wavs, each in all 6 formats (48 bases).

| step | profile | cases | analysed | panics | hangs | crashes | allocs >2 GB | slowest tag scan | slowest case | process peak RSS |
|---|---|---|---|---|---|---|---|---|---|---|
| 2 | hostile, before fix | 15000 | every 20th | 0 | 0 | 0 | 0 | 26.3 s (15 flac cases), 10.5 s (m4a, 100 cases) | 26.3 s | 1094 MB |
| 3 | hostile, after fix | 15000 | every 20th | 0 | 0 | 0 | 0 | 1.1 s | 3.7 s | 1133 MB |
| 4 | odd | 6000 | every 10th | 0 | 0 | 0 | 0 | 0.56 s | 2.2 s | 1318 MB |
| 5 | long | 1250 | every 100th | 0 | 0 | 0 | 0 | 1.5 s | 8.5 s | 1828 MB |
| 6 | real | 7200 | every 40th | 0 | 0 | 0 | 0 | 0.86 s | 6.0 s | 1831 MB |

Fix (step 3): lofty (external) reads one flac metadata block or mp4 atom at a time with no cap, so a flac with about 2 million tiny blocks (an 8 MB zeroed range over the two 16 MB pictures) took 26 s and a flat 1M-atom udta took 6 to 10 s. `scanner::read_track_metadata` now pre-scans structure with seeks only and refuses a flac with more than 4096 metadata blocks or an mp4 with more than 100000 atoms (nesting capped at 12). Real files have under 30 blocks and a few thousand atoms. Regression tests: `scanner::tests` (flood of blocks, flood of atoms, bogus atom sizes, ID3 claiming more than the file).

Not a failure but noted: no tag-scan allocation above 66 MB even for pictures claimed at 4 GB; a real 20 MB picture is read into memory once (about 48 MB peak). One shard of the first `odd` run exited with code 101 outside the guarded stages at case 2452 and did not repeat in a full rerun or in isolation; the panic hook did not print then, it does now.

Analysis output: decode.rs and the analysis crate are untouched, and the analysis crate does not call the scanner, so the 50-track dev output is unchanged by construction; the diff was not run.

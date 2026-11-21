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

Result: corpus clean at baseline, so no decoder fixes were made. Not covered: Mo's lane (PDB/ANLZ writer) is not exercised by this corpus.

# Analysis accuracy benchmark, 2026-10

Status: PARTIAL. Key accuracy on Beatport is complete (598 of 598). Rekordbox agreement covers only 178 of 652 manifest entries, because the Rekordbox audio was still copying onto the dev box when the timebox ended.

Backend measured: `deeprhythm_essentia` (the backend `pioneer-test-ui` calls: DeepRhythm BPM, Essentia `bgate` key). Measured, not tuned. Box: dev VM, Linux x86_64, 16 CPU cores, CPU-only torch 2.14.1+cpu.

## Read this first

- Beatport labels are external ground truth, so Beatport key numbers are accuracy.
- Rekordbox `master.db` values are Rekordbox's own analysis (or Merlin's edits). Agreement with them is compatibility, not correctness (docs/process/lessons.md).
- DeepRhythm emits integer BPM only (0 of 175 detected values were fractional). Rekordbox BPM is often fractional (e.g. 124.91). The strict "within +-0.1" row is therefore close to "Rekordbox's value is itself an integer", and is not a quality measure on its own.
- There is no beat-grid ground truth in this benchmark. See "Beat grid: missing" below.

## Data actually used

| Set | Manifest entries | Present and fingerprint-verified | Notes |
|---|---:|---:|---|
| beatport-edm-key keyonly-clean-full | 598 | 598 | complete; fingerprint mismatch 0 |
| beatport-edm-key keyonly-200 | 200 | 200 | subset of the above; not scored separately |
| rekordbox-testdata-2025 | 652 | 178 | copy in progress; 474 not yet present; fingerprint mismatch 0 of the 178 |

Each Rekordbox file was checked against the manifest `content_fingerprint` (SHA256 of first 64 KB + last 64 KB + size) by `benchmark/scripts/make_dev_manifest.py`. The coordinator reported 22 of 652 are missing on the Mac side and 50 have duplicate filenames; files were matched by filename and parent folder. No file was excluded for a fingerprint mismatch, so the match is byte-identical to what the manifest recorded.

The quick60 / key160 / batch100 subsets could not be scored meaningfully: only 8, 37 and 29 of their tracks were present. Their numbers are in line with the pooled set (key exact 62.5%, 62.9%, 63.0%; BPM integer-rounded 87.5%, 57.1%, 48.1%) but are far too small to rank. Re-run them when the copy is complete.

## Key accuracy vs Beatport labels (accuracy)

Denominator: 598 tracks, all with a single `X major` / `X minor` Beatport label (the clean subset; unknown, modal and multi-key labels are excluded upstream). Every track produced a key (0 failures). Beatport labels are converted to Camelot. Detected and reference keys are both Camelot codes.

| Class | Tracks |
|---|---:|
| Exact | 323 / 598 = 54.0% |
| Relative major/minor (same Camelot number, other letter) | 33 / 598 = 5.5% |
| Camelot neighbour (+-1 number, same letter) | 56 / 598 = 9.4% |
| Exact + relative + neighbour | 412 / 598 = 68.9% |
| Exact + neighbour (the metric docs/analysis/key-detection-findings.md calls "adjacent") | 379 / 598 = 63.4% |
| Other | 186 / 598 = 31.1% |

Exact 54.0% and exact+relative+neighbour 68.9% reproduce the April result for `essentia_key_bgate` (54.0% / 68.9%) exactly. Rekordbox on the 698-track broad Beatport set was 47% exact (different denominator).

Musical relationship of all errors (denominator 598):

| Relationship | Tracks |
|---|---:|
| Parallel (same tonic, other mode) | 100 / 598 = 16.7% |
| Other mode swap | 66 / 598 = 11.0% |
| Fifth neighbour, same mode | 56 / 598 = 9.4% |
| Relative major/minor | 33 / 598 = 5.5% |
| Other, same mode | 16 / 598 = 2.7% |
| Whole tone, same mode | 3 / 598 = 0.5% |
| Tritone, same mode | 1 / 598 = 0.2% |

About 33% of all tracks (parallel + other mode swap + relative) have the wrong mode. Mode confusion is the dominant failure, the same pattern Rekordbox shows.

## Key agreement vs Rekordbox (compatibility)

Denominator: 129 unique tracks (176 entries with a detected key and a Rekordbox key; duplicates collapsed by artist + title + reference BPM + reference key). Reference: `rekordbox-masterdb-standard-camelot`. Entries with no Rekordbox key are excluded.

| Class | Unique tracks (129) | All entries (176) |
|---|---:|---:|
| Exact | 70 = 54.3% | 97 = 55.1% |
| Relative major/minor | 3 = 2.3% | 3 = 1.7% |
| Camelot neighbour | 16 = 12.4% | 25 = 14.2% |
| Exact + relative + neighbour | 89 = 69.0% | 125 = 71.0% |
| Other | 40 = 31.0% | 51 = 29.0% |

Relationship breakdown (176 entries): exact 55.1%, fifth neighbour 14.2%, other mode swap 11.4%, other same mode 10.8%, parallel 4.5%, relative 1.7%, whole tone 1.7%, tritone 0.6%. Here the parallel-key confusion that dominates Beatport is rare; this library skews to minor-key house and techno where Rekordbox and Essentia agree more on mode.

## BPM agreement vs Rekordbox (compatibility)

Denominator: 128 unique tracks (175 entries with a Rekordbox BPM and a detected BPM; duplicates collapsed as above). Reference: `rekordbox-masterdb` BPM. Tolerance rows use the reference BPM as given, not rounded.

| Metric | Unique tracks (128) | All entries (175) |
|---|---:|---:|
| Exact within +-0.1 BPM | 67 = 52.3% | 89 = 50.9% |
| Octave-tolerant (x1, x2, x0.5), within +-0.1 | 67 = 52.3% | 89 = 50.9% |
| Integer-rounded equal (round(ours) = round(ref)) | 75 = 58.6% | 102 = 58.3% |
| Integer-rounded, octave-tolerant | 75 = 58.6% | 102 = 58.3% |
| Within 1.0 BPM absolute (extra) | 120 = 93.8% | |
| Within 1% (extra) | 125 = 97.7% | 170 = 97.1% |

- Octave errors: none. No detection was 2x or 0.5x the reference, so the octave-tolerant rows equal the plain rows.
- Mean signed error (detected minus reference): -0.24 BPM. The tool tends to land slightly low, consistent with integer truncation.
- Strict agreement is limited by resolution, not by gross errors: 94% of tracks are within 1 BPM, but only about half agree to +-0.1 or to the integer.

## Worst failures and likely cause

Causes below are inferred from the numbers, not confirmed by listening or by looking at the audio.

BPM (unique tracks, worst by ratio):

| Track | Detected | Rekordbox | Likely cause |
|---|---:|---:|---|
| Koreless - Primes | 117 | 95.24 | Not an octave error (ratio 1.228, about 5:4). Likely a polyrhythmic or off-grid groove where DeepRhythm locks onto a different subdivision. Only real gross BPM miss in the set. |
| LP - When The Beat Drops | 118 | 120.00 | Off by 2 BPM; likely DeepRhythm resolution or a tempo-drifting track. |
| Kai Campos, Mount Kimbie - Zone 3 (City Limits) | 126 | 127.40 | Fractional reference; integer output cannot match. |
| The Moody Boys - Lion Dance (Funky Zulu Mix) | 118 | 119.07 | Same: integer output vs fractional reference. |
| Frank & Tony - Back Home | 112 | 113.00 | Off by one with an integer reference; true 1 BPM error. |
| Russ Gabriel - Drax Copter, Four Tet - Reversing, Frits Wentink - Jellyfish, Linkwood - Love Lost | 123 / 114 / 120 / 120 | 124.06 / 114.98 / 121.00 / 121.00 | Consistent -1 BPM bias: DeepRhythm appears to floor, not round. |

The bulk of non-matching tracks sit in this -1 BPM band, not in octave errors.

Key vs Beatport: all 186 "other" misses are listed in the run output, not here. Typical patterns from the sampled misses (artist/title, Beatport key vs detected): Zoom - Five To Eleven (7A vs 11B), Havoc - Balling Out (3A vs 6B), Lil Ric - In My Throwback (11A vs 8A), PNC - Who Betta Than This? (1A vs 12B). Likely causes: short 120 s Beatport preview clips, bass-heavy or modally ambiguous EDM (little harmonic content), tonic ambiguity between a major key and its parallel minor, and genuine disagreement between Beatport's label and the audible key. Beatport labels themselves are noisy, so some "failures" are label errors. Not verified per track.

Key vs Rekordbox examples: Dauwd - La Nota (8A vs 1A), Silat Beksi - Curandero (5A vs 8A), Move D - Silk Dub (5A vs 6B), Moodymann - Black Paint, Larry Heard's After Dark Mix (6A vs 11A). Both are algorithmic outputs with different key profiles, so a disagreement does not say which is right.

Backend errors (2 of 178 Rekordbox entries, both failed hard, no result):

- One track, "Mike Dehnert - Lautschrift" (405 s, mp3): DeepRhythm raised `AttributeError: 'NoneType' object has no attribute 'to'`. Cause not investigated.
- One track (no reference BPM/key, not in any denominator): `ffmpeg failed ... Failed to find two consecutive MPEG audio frames`, i.e. a corrupt or unusual mp3 header.

## Runtime per track (this box)

| Run | Features | Track length | Mean s / track |
|---|---|---:|---:|
| Beatport 598, 1 process, otherwise idle | key only | 120 s clips | 1.17 (median 1.15, p95 1.29) |
| Beatport smoke test, 5 tracks, 1 process | bpm + key | 120 s | about 5.9 (first track 21 s incl. model load) |
| Beatport smoke test, 5 tracks, 1 process | all (what the app requests) | 120 s | about 7.7 (first track 23 s) |
| Rekordbox 12 tracks, 1 process | all (what the app requests) | mean 414 s | 22.96 (median 20.1; first track 53 s incl. model load) |
| Rekordbox 176, 8 parallel processes x 2 threads, rsync running | bpm + key | full-length tracks | 37.6 (median 35.7); contended, not representative |

Best single-number answer for the app's real workload on this box: about 20 s per 7-minute track with all features, which is roughly 0.05x realtime on CPU. Model load adds roughly 15 to 30 s once per process. DeepRhythm dominates cost: key-only is about 1 s on a 2-minute clip.

## Beat grid: missing

This benchmark has no beat-grid ground truth. `fourfour-analyze` returns an empty `beats` array today (DeepRhythm gives a global BPM only; the Lexicon-seeded beat generator inside the backend is not exposed, and nothing here scores grid placement). So no first-beat, grid alignment or drift number exists for any backend. BPM agreement above does not validate the grid.

How to get one:

1. Rekordbox ANLZ PQTZ, same 652 tracks. Rekordbox stores a beat grid per track in its ANLZ `.DAT`/`.EXT` analysis files (PQTZ tag: beat number, tempo, time per beat). Read them with an existing parser (e.g. the repo's own Pioneer ANLZ code or a library such as rekordcrate). This gives a grid for the exact files already benchmarked, at no annotation cost. Caveat: it is Rekordbox's grid, so it measures compatibility, and any grids Merlin hand-corrected are the closest thing to truth. Score with the existing `compare_beats` (F-measure at 50 ms, median offset), plus first-beat offset in ms and drift over the track.
2. Hand-verify a small subset (30 to 50 tracks, mixed genres) in Rekordbox or by ear, to turn the PQTZ comparison into a measure of correctness for at least some tracks.
3. Public beat-annotated sets for correctness on non-library material. Candidates to evaluate: Ballroom, GTZAN with Marchand/Peeters beat annotations, SMC, Harmonix Set (annotations only, audio must be sourced), and Giant Steps Tempo for BPM-only EDM clips. Licences differ and several are research or non-commercial; the annotation licence and the audio licence must be checked per dataset before use. I did not verify any licence for this report. For a private benchmark that never ships audio, research-use terms are typically workable, but that is a decision for Merlin.

Recommendation: do option 1 first (cheap, same tracks), then option 2 on a small set.

## Other findings

- `fourfour-benchmark run` is broken for every backend on this repo state. The result cache cannot serialise `AnalysisResult.waveform_preview` (a `bytes` field with default `b""`), so the first track raises `TypeError: Object of type bytes is not JSON serializable`. The same cause fails 5 of 127 tests (below). The backend files were not touched; a driver, `benchmark/scripts/run_backend_direct.py`, runs the registered backend in-process without the cache and writes the same `raw/<variant>.json` layout. Fix in `cache.py` or `types.py` is a small follow-up.
- Rekordbox corpus contains duplicate entries (same track in several folders): 178 entries collapse to 128 unique BPM rows. Always quote the unique denominator.
- `analysis/README.md` links to `../docs/key-detection-benchmark-findings.md`, which no longer exists at that path (now `docs/analysis/key-detection-findings.md`).

## Validation

`cd analysis && .venv/bin/python -m pytest -q`: 122 passed, 5 failed (`test_cache.py::test_save_and_load`, `test_cache.py::test_cache_roundtrip_with_beats`, `test_cli.py::test_cli_single_file_json_output_payload`, `test_cli.py::test_fourfour_analyze_json_output_payload`, `test_cli.py::test_cli_multiple_files`). The two cache failures are the `bytes` serialisation bug above. The three CLI failures are an ImportError and a KeyError in the tests; not investigated.

## Reproduce

Setup (CPU torch from the PyTorch CPU index):

```bash
cd analysis
uv venv .venv
uv pip install --python .venv/bin/python --extra-index-url https://download.pytorch.org/whl/cpu --index-strategy unsafe-best-match -e ".[dev]"
```

Dev-path manifests (originals untouched; drops missing files and fingerprint mismatches, prints the counts):

```bash
python3 benchmark/scripts/make_dev_manifest.py \
  benchmark/manifests/beatport-edm-key-keyonly-clean-full.corpus.json \
  benchmark/manifests/beatport-edm-key-keyonly-clean-full.dev.corpus.json \
  --strip-prefix /Users/merlinkraemer/dev/projects/fourfour/benchmark/datasets/beatport-edm-key/audio/ \
  --dataset-root <repo>/benchmark/datasets/beatport-edm-key/audio

python3 benchmark/scripts/make_dev_manifest.py \
  benchmark/manifests/rekordbox-testdata-2025.corpus.json \
  benchmark/manifests/rekordbox-testdata-2025.dev.corpus.json \
  --strip-prefix /Users/merlinkraemer/local/2-Areas/1-Mukke/ \
  --dataset-root <repo>/benchmark/datasets/rekordbox-testdata-2025 \
  --rename-map <tsv of "<current path under dataset-root>\t<path relative to the strip prefix>">
```

Key accuracy, Beatport (about 12 minutes):

```bash
cd analysis
.venv/bin/python ../benchmark/scripts/run_backend_direct.py \
  --corpus ../benchmark/manifests/beatport-edm-key-keyonly-clean-full.dev.corpus.json \
  --features key --run-id q9-beatport-clean-full-key
.venv/bin/python ../benchmark/scripts/score_run.py ../benchmark/results/q9-beatport-clean-full-key \
  --corpus ../benchmark/manifests/beatport-edm-key-keyonly-clean-full.dev.corpus.json
python3 ../benchmark/scripts/key_error_breakdown.py \
  ../benchmark/manifests/beatport-edm-key-keyonly-clean-full.dev.corpus.json ../benchmark/results/q9-beatport-clean-full-key
```

Rekordbox agreement (8 shards, 2 threads each):

```bash
cd analysis
for i in 0 1 2 3 4 5 6 7; do
  OMP_NUM_THREADS=2 MKL_NUM_THREADS=2 .venv/bin/python ../benchmark/scripts/run_backend_direct.py \
    --corpus ../benchmark/manifests/rekordbox-testdata-2025.dev.corpus.json \
    --features bpm,key --run-id q9-rb-bpmkey --shard $i/8 &
done; wait
.venv/bin/python ../benchmark/scripts/score_run.py ../benchmark/results/q9-rb-bpmkey \
  --corpus ../benchmark/manifests/rekordbox-testdata-2025.dev.corpus.json --dedupe
```

Add `--only-ids-from ../benchmark/manifests/rekordbox-testdata-2025-quick60.corpus.json` (or key160, batch100) to score a subset. Runtime sample with the app's feature set: `--features bpm,key,energy,waveform,cues --limit 12`, one process.

The dev manifests hold local absolute paths and are not committed; regenerate them with the commands above.

# Analysis accuracy benchmark, 2026-10

Status: complete for what is on disk. Beatport key accuracy: 598 of 598. Rekordbox agreement: 579 fingerprint-verified tracks of the 652 manifest entries (89%); the other 73 are not on this box (see "Data actually used"). Re-run on 2026-10-15 (Q9b); it replaces the earlier 178-track partial run.

Backend measured: `deeprhythm_essentia` (the backend `pioneer-test-ui` calls: DeepRhythm BPM, Essentia `bgate` key). Measured, not tuned. Box: dev VM, Linux x86_64, 16 CPU cores, CPU-only torch 2.14.1+cpu.

## Read this first

- Beatport labels are external ground truth, so Beatport key numbers are accuracy.
- Rekordbox `master.db` values are Rekordbox's own analysis (or Merlin's edits). Agreement with them is compatibility, not correctness (docs/process/lessons.md).
- DeepRhythm emits integer BPM only (0 of 573 detected values were fractional). Rekordbox BPM is often fractional (e.g. 124.91). The strict "within +-0.1" row is therefore close to "Rekordbox's value is itself an integer", and is not a quality measure on its own.
- There is no beat-grid ground truth in this benchmark. See "Beat grid: missing" below.

## Data actually used

| Set | Manifest entries | Present and fingerprint-verified | Notes |
|---|---:|---:|---|
| beatport-edm-key keyonly-clean-full | 598 | 598 | complete; fingerprint mismatch 0 |
| beatport-edm-key keyonly-200 | 200 | 200 | subset of the above; not scored separately |
| rekordbox-testdata-2025 | 652 | 579 | 589 present when the run started, 10 fingerprint mismatches excluded, 63 not present |

Rekordbox denominator: 579 verified entries. Each file was checked against the manifest `content_fingerprint` (SHA256 of first 64 KB + last 64 KB + size) by `benchmark/scripts/make_dev_manifest.py`; the 10 mismatches are different bytes behind a matching filename (filename-only matches: duplicate filenames or another version of the track) and are excluded. Of those 579, 574 analysed without a backend error, and 573 / 574 have a Rekordbox BPM / key. Because the same track appears in several folders, the tables also give a unique-track count (artist + title + reference BPM + reference key counted once): 472 unique for BPM, 473 for key.

Why tracks are missing: 63 of the 652 manifest entries had no file on this box at run start. The coordinator reported 22 tracks missing on the Mac side (moved or renamed there, so not found by filename); the rest were still copying. A later copy pass brought the present count to 631, which would leave about 21 not found by filename. That later state is not scored: the run had already started on the 589 present files and was not restarted. A re-run over the 631 would add about 40 entries (about 7%); its effect on the percentages was not measured.

The quick60 / key160 / batch100 subsets are scored below on their present, verified tracks (56 of 60, 145 of 160, 89 of 100 entries).

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

Reference: `rekordbox-masterdb-standard-camelot`. Entries with no Rekordbox key are excluded. Denominator: 473 unique tracks (574 entries with a detected key and a Rekordbox key). Every analysed track produced a key.

| Class | Unique tracks (473) | All entries (574) |
|---|---:|---:|
| Exact | 199 = 42.1% | 251 = 43.7% |
| Relative major/minor | 29 = 6.1% | 29 = 5.1% |
| Camelot neighbour | 60 = 12.7% | 80 = 13.9% |
| Exact + relative + neighbour | 288 = 60.9% | 360 = 62.7% |
| Exact + neighbour | 259 = 54.8% | 331 = 57.7% |
| Other | 185 = 39.1% | 214 = 37.3% |

Relationship breakdown (574 entries): exact 43.7%, other mode swap 16.0%, fifth neighbour 13.9%, other same mode 12.0%, parallel 5.7%, relative 5.1%, whole tone 2.8%, tritone 0.7%. Compared with the 178-track run (exact 55%) agreement is about 11 points lower on the full set: the 178 tracks were a partial slice and not representative. Wrong mode (mode swap + parallel + relative) is about 27% of entries.

## BPM agreement vs Rekordbox (compatibility)

Reference: `rekordbox-masterdb` BPM. Denominator: 472 unique tracks (573 entries with a Rekordbox BPM and a detected BPM). Tolerance rows use the reference BPM as given, not rounded.

| Metric | Unique tracks (472) | All entries (573) |
|---|---:|---:|
| Exact within +-0.1 BPM | 168 = 35.6% | 222 = 38.7% |
| Octave-tolerant (x1, x2, x0.5), within +-0.1 | 169 = 35.8% | 223 = 38.9% |
| Integer-rounded equal | 185 = 39.2% | 247 = 43.1% |
| Integer-rounded, octave-tolerant | 188 = 39.8% | 250 = 43.6% |
| Within 1.0 BPM absolute (extra) | 425 = 90.0% | 520 = 90.8% |
| Within 1% (extra) | 434 = 91.9% | 533 = 93.0% |

- Octave errors by the script's definition (integer-rounded, detected = 2x or 0.5x the reference): 0 at 2x, 3 at 0.5x (3 of 472 = 0.6%). The check is strict: tracks with a fractional reference near 2x or 0.5x do not count (Four Tet - Two Thousand and Seventeen 150 vs 75.45; Guilherme Coutinho/Grupo Stalo - Rio Corrente 80 vs 162.17). Counting those, true octave confusion is a handful of tracks, not a trend.
- Mean signed error (detected minus reference): -0.41 BPM (unique), -0.37 (all). The tool lands slightly low, consistent with integer truncation.
- Strict agreement is limited by resolution: 90% of tracks are within 1 BPM, but only 36% (unique) agree to +-0.1 and 39% to the integer.
- Compared with the 178-track run: strict agreement fell (52% to 36% within +-0.1, 59% to 39% integer-equal) and within 1 BPM fell from 94% to 90%. The remaining tracks include more slow or half-time material (see worst failures).

Subsets (unique tracks, present and verified):

| Subset | Tracks present (BPM / key denominators) | BPM within +-0.1 | BPM integer-equal | BPM within 1 BPM | Key exact | Key exact + relative + neighbour |
|---|---:|---:|---:|---:|---:|---:|
| quick60 | 56 of 60 (55 / 55) | 29 = 52.7% | 30 = 54.5% | 49 = 89.1% | 26 = 47.3% | 41 = 74.5% |
| key160 | 145 of 160 (139 / 139) | 61 = 43.9% | 65 = 46.8% | 126 = 90.6% | 63 = 45.3% | 93 = 66.9% |
| batch100 | 89 of 100 (84 / 84) | 32 = 38.1% | 35 = 41.7% | 77 = 91.7% | 37 = 44.0% | 52 = 61.9% |
| All verified | 579 (472 / 473) | 168 = 35.6% | 185 = 39.2% | 425 = 90.0% | 199 = 42.1% | 288 = 60.9% |

The subsets are small and overlap the full set; differences between them are within sampling noise (quick60 in particular, n = 55).

## Worst failures and likely cause

Causes below are inferred from the numbers, not confirmed by listening or by looking at the audio.

BPM (unique tracks, worst by ratio against the reference):

| Track | Detected | Rekordbox | Likely cause |
|---|---:|---:|---|
| Four Tet - Two Thousand and Seventeen | 150 | 75.45 | Octave error (1.99x) with a fractional reference, so it is not counted as octave by the strict check. |
| Guilherme Coutinho/Grupo Stalo - Rio Corrente | 80 | 162.17 | Octave error the other way (0.49x). |
| Melanie Poret - Humanity, Floating Points - Falaise, Khotin - Shopping List, Boards of Canada - Olson | 151 / 143 / 136 / 120 | 83.33 / 82.19 / 78.53 / 78.95 | Reference about 75-85 BPM, detected 1.5-1.8x higher: half-time feel or broken-beat tracks where DeepRhythm picks a faster pulse. |
| Koreless - Primes | 117 | 95.24 | Ratio 1.23 (about 5:4); still the oddest single miss, probably an off-grid groove. |
| Russ Gabriel - Drax Copter and similar | 123 | 124.06 | Consistent -1 BPM bias: DeepRhythm appears to floor, not round. |

The bulk of non-matching tracks sit in this -1 BPM band (90% are within 1 BPM). The roughly 10% of tracks beyond 1 BPM are the ones that matter for beat-grid use; they are dominated by slow (75-100 BPM) references. Causes are inferred from the numbers, not confirmed by listening.

Key vs Beatport: all 186 "other" misses are listed in the run output, not here. Typical patterns from the sampled misses (artist/title, Beatport key vs detected): Zoom - Five To Eleven (7A vs 11B), Havoc - Balling Out (3A vs 6B), Lil Ric - In My Throwback (11A vs 8A), PNC - Who Betta Than This? (1A vs 12B). Likely causes: short 120 s Beatport preview clips, bass-heavy or modally ambiguous EDM (little harmonic content), tonic ambiguity between a major key and its parallel minor, and genuine disagreement between Beatport's label and the audible key. Beatport labels themselves are noisy, so some "failures" are label errors. Not verified per track.

Key vs Rekordbox examples: Dauwd - La Nota (8A vs 1A), Dave Aju - Get It (10A vs 6A), Oleg Poliakov - Caravan (9B vs 5A), Russ Gabriel - Drax Copter (5A vs 8A). Both are algorithmic outputs with different key profiles, so a disagreement does not say which is right.

Backend errors (5 of 579 Rekordbox entries, no result; excluded from the denominators):

- 4 tracks: DeepRhythm raised `AttributeError: 'NoneType' object has no attribute 'to'` (the earlier run saw this on "Mike Dehnert - Lautschrift"; cause not investigated, so it recurs and is worth a follow-up).
- 1 track: `ffmpeg failed ... Failed to find two consecutive MPEG audio frames`, a corrupt or unusual mp3 header.

## Runtime per track (this box)

| Run | Features | Track length | Mean s / track |
|---|---|---:|---:|
| Beatport 598, 1 process, otherwise idle | key only | 120 s clips | 1.17 (median 1.15, p95 1.29) |
| Beatport smoke test, 5 tracks, 1 process | bpm + key | 120 s | about 5.9 (first track 21 s incl. model load) |
| Beatport smoke test, 5 tracks, 1 process | all (what the app requests) | 120 s | about 7.7 (first track 23 s) |
| Rekordbox 12 tracks, 1 process (earlier run) | all (what the app requests) | mean 414 s | 22.96 (median 20.1; first track 53 s incl. model load) |
| Rekordbox 12 tracks, 1 process, box load average 4 to 6 from other work (Q9b) | bpm + key | full-length tracks | 12.99 (median 11.85, p95 24.7; first track includes model load) |
| Rekordbox 574 ok, 4 processes x 4 threads (Q9b), box load average about 28 | bpm + key | full-length tracks | 18.32 (median 16.47, p95 34.5, max 59.1); wall 44 min for 579 tracks |

The 4-process run is within the 4-process limit but not uncontended: other agents shared the box (load average rose to about 28 against 16 cores), and 4 x 4 threads alone fill the CPU. Read it as an upper bound. The single-process sample ran at a load average of 4 to 6 and is the better per-track figure; it is bpm + key only, so the app's full feature set costs more (earlier measurement: about 20 s per 7-minute track with all features, roughly 0.05x realtime). Model load adds roughly 15 to 30 s once per process. DeepRhythm dominates cost.

## Beat grid: missing

This benchmark has no beat-grid ground truth. `fourfour-analyze` returns an empty `beats` array today (DeepRhythm gives a global BPM only; the Lexicon-seeded beat generator inside the backend is not exposed, and nothing here scores grid placement). So no first-beat, grid alignment or drift number exists for any backend. BPM agreement above does not validate the grid.

How to get one:

1. Rekordbox ANLZ PQTZ, same 652 tracks. Rekordbox stores a beat grid per track in its ANLZ `.DAT`/`.EXT` analysis files (PQTZ tag: beat number, tempo, time per beat). Read them with an existing parser (e.g. the repo's own Pioneer ANLZ code or a library such as rekordcrate). This gives a grid for the exact files already benchmarked, at no annotation cost. Caveat: it is Rekordbox's grid, so it measures compatibility, and any grids Merlin hand-corrected are the closest thing to truth. Score with the existing `compare_beats` (F-measure at 50 ms, median offset), plus first-beat offset in ms and drift over the track.
2. Hand-verify a small subset (30 to 50 tracks, mixed genres) in Rekordbox or by ear, to turn the PQTZ comparison into a measure of correctness for at least some tracks.
3. Public beat-annotated sets for correctness on non-library material. Candidates to evaluate: Ballroom, GTZAN with Marchand/Peeters beat annotations, SMC, Harmonix Set (annotations only, audio must be sourced), and Giant Steps Tempo for BPM-only EDM clips. Licences differ and several are research or non-commercial; the annotation licence and the audio licence must be checked per dataset before use. I did not verify any licence for this report. For a private benchmark that never ships audio, research-use terms are typically workable, but that is a decision for Merlin.

Recommendation: do option 1 first (cheap, same tracks), then option 2 on a small set.

## Other findings

- `fourfour-benchmark run` is broken for every backend on this repo state. The result cache cannot serialise `AnalysisResult.waveform_preview` (a `bytes` field with default `b""`), so the first track raises `TypeError: Object of type bytes is not JSON serializable`. The same cause fails 5 of 127 tests (below). The backend files were not touched; a driver, `benchmark/scripts/run_backend_direct.py`, runs the registered backend in-process without the cache and writes the same `raw/<variant>.json` layout. Fix in `cache.py` or `types.py` is a small follow-up.
- Rekordbox corpus contains duplicate entries (same track in several folders): 573 entries with a BPM collapse to 472 unique rows. Always quote the unique denominator.
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
  --dataset-root <repo>/benchmark/datasets/rekordbox-testdata-2025
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

Rekordbox agreement (4 shards, 4 threads each; Q9b run, about 45 min on a busy box):

```bash
cd analysis
for i in 0 1 2 3; do
  OMP_NUM_THREADS=4 MKL_NUM_THREADS=4 .venv/bin/python ../benchmark/scripts/run_backend_direct.py \
    --corpus ../benchmark/manifests/rekordbox-testdata-2025.dev.corpus.json \
    --features bpm,key --run-id q9b-rb-bpmkey --shard $i/4 &
done; wait
.venv/bin/python ../benchmark/scripts/score_run.py ../benchmark/results/q9b-rb-bpmkey \
  --corpus ../benchmark/manifests/rekordbox-testdata-2025.dev.corpus.json --dedupe
```

Add `--only-ids-from ../benchmark/manifests/rekordbox-testdata-2025-quick60.corpus.json` (or key160, batch100) to score a subset. Runtime sample: `--features bpm,key --limit 12`, one process (add `energy,waveform,cues` for the app's feature set). The 2026-10-15 run needs no `--rename-map` because the files now sit at their manifest paths under the dataset root.

The dev manifests hold local absolute paths and are not committed; regenerate them with the commands above.

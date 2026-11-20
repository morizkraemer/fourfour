# hc-L9: key gap diagnostic (fourfour vs Rekordbox on 509 library tracks)

Report-only. Scratch code and raw outputs: `~/.cache/hc-L9/` (`keyrun/`, `out.*.jsonl`, `an.py`). Not committed.

## Verdict

1. No mapping bug. Camelot tables in `keymap.rs` and `compare.py` are both standard (2A = Ebm, 7A = Dm, 1A = Abm) and agree. The pair "Rekordbox Abm vs fourfour 2A" is Abm vs Ebm, a real fifth apart, not a notation slip.
2. No preview-vs-full domain bug. fourfour already analyses the whole gapless track; analysing a 120 s middle section agrees with Rekordbox worse, not better.
3. The gap is mostly reference noise plus a harder domain. Rekordbox is itself about 47% exact against Beatport labels. Against a third source (file key tags, n=84) fourfour agrees more than Rekordbox does (56% vs 46%). Remaining fixable lever: low-confidence tracks (see below).

## Numbers

Mapping check (step 1)
- Exact agreement 250 of 509 (49.1%). Best constant pitch-class shift is 0 (237 of 432 minor-minor pairs); no shift, mode swap or Camelot permutation raises it. A wrong table would give about 8% exact (chance), so 0 disagreements vanish from a mapping fix.
- `keymap.rs` tonic index is C=0; MAJOR/MINOR tables checked entry by entry against standard Camelot. The stored key string is `camelot(class)` (`service.rs:244,413`, `analyze.rs:309`). Rekordbox rows hold note names ("Abm"), converted with the standard wheel.
- Duplicates: the 509 are 373 unique tracks (same BPM, key, length across wav/flac/mp3 copies). Unique-track exact is 48.3%, E+R+F 68.4%. The "13 tracks" Abm/2A pair is about 7 unique tracks; Gm/7A about 5.
- Fifth direction (unique, 63 fifth cases): fourfour is the dominant (+7) of Rekordbox in 36, subdominant (+5) in 27. Mild skew, not a systematic offset.
- Mode: Rekordbox minor 84%, fourfour minor 94%. Relative+parallel only 6% of tracks.

Domain gap (step 2), 120 library tracks (25 systematic-pair tracks plus 100 random, so exact is lower than the full-set 49%; read the rows relative to each other). Median length 401 s, median |tuning| 9 cents.

| Window analysed | Exact vs Rekordbox | Fifth | Same key as full-track |
|---|---:|---:|---:|
| full track (shipped) | 39.2% | 30.0% | - |
| middle 120 s | 34.2% | 30.0% | 103/120 |
| first 120 s | 30.8% | 21.7% | 76/120 |
| last 120 s | 36.7% | 25.8% | 90/120 |
| middle 60 s | 34.2% | 30.8% | 89/120 |
| quarter points, 60 s | 40.8% / 39.2% | 25-30% | 88-93/120 |

- Re-running the detector on the audio reproduces the stored library key for 120 of 120 tracks (no drift between app and this harness).
- Previews are not the cause: section keys do not match Rekordbox better than the whole track. Sections disagree with the full-track key on 14-37% of tracks, so long-form EDM has real key movement and a 2-min clip is a noisy sample.
- Confidence tracks agreement: full-track confidence under 0.2 gives 1 of 26 exact; 0.2-0.4 gives 16 of 44; 0.4-0.6 gives 17 of 32; 0.6-0.8 gives 11 of 14; over 0.8 gives 2 of 4.
- The systematic pairs (25 rows): confidence 0.15-0.2 for 14 of 25. In 6 other views per track, fourfour's own key wins 112 times, Rekordbox's key 10 times, something else 28. So fourfour is consistent about Ebm/Dm there; these are low-confidence, bass/drone-heavy tracks where the model picks the dominant minor. Tuning is not the driver (median |tuning| 6 cents on these; a few sit at about +-40 cents).

Third opinion (84 library files carry a key tag, source unknown, likely Beatport or Mixed In Key):
- fourfour exact 47 of 84 (56%), Rekordbox exact 39 of 84 (46%). Tags are not truth, but Rekordbox is not a better reference than they are.

Context from existing docs (`key-detector-2026-10.md`, `benchmark-2026-10.md`): Rekordbox vs Beatport labels 47% exact; Essentia bgate vs Rekordbox on this library about 45%; fourfour out-of-fold on the 598-track Beatport set 57-61%, not 79%. The 78.9% GiantSteps+ figure is on easy, preview-length, expert-labelled material; the matching figure for this genre mix is about 60%.

## Classification

| Part | Share of the 509 | Nature |
|---|---|---|
| Notation / mapping | 0% | none found |
| Preview vs full-track | 0% | not a factor; full track is already used and is best |
| Fifth confusions (about 17-18%) | | model plus Rekordbox disagreement; low-confidence dominant calls |
| Other (about 28%) | | mostly Rekordbox error or modal/ambiguous tracks; not separable without a human ear |
| Duplicate counting | | inflates pair counts about 1.4x, not the rate |

## Fix

None to apply. No bug found, so no file/line to change.

Cheap product lever (not a bug): the detector already outputs a confidence. Showing the key as unsure below about 0.3 would flag the tracks where agreement is under 30%. Needs a decision on UX, about 0.5 day.

## Follow-up cost

- Truth for this library: Merlin labels about 60 disagreement tracks by ear (about 1 hour listening, phone clips) to split "fourfour wrong" vs "Rekordbox wrong". That is the only way to know the true gap; compute cost is nil.
- Training on in-domain labels needs those ear labels at scale (hundreds); the existing docs already conclude no permissive full-track EDM key set exists.

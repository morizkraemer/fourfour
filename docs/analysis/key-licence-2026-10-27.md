# Key templates from permissive sources (Q46), 2026-10-27

Branch `q46-key` (from `q42-overnight`). Report only. Nothing here ships.

**Goal.** Replace the shipped key templates with templates whose training labels are permissively licensed. The shipped templates are the cyanbox port, fit by its author on GiantSteps MTG, which has no licence. The keydet alternative was fit on Beatport (CC BY-SA 4.0). The bar is at least 60% exact on the Beatport test split: 199 tracks, those with manifest `id % 3 == 0`.

**Result: bar not reached.** The best licence-clean templates score **55.8% exact on the test split, 72.9% exact + relative + fifth, MIREX 65.6**. On the 399 dev tracks they score 59.9%. The shipped cyanbox port scores 58.3% / 72.9% / 67.0 on the same 199 tracks, and keydet scores 57.3% / 70.9%. The 95% interval at n = 199 is about ±7 points, so the gap to cyanbox is 5 tracks and within noise. It is still the wrong direction.

## Label sources and licences

Checked on 2026-10-27 with the GitHub API `license` field, Zenodo records and project pages. Permissive means CC0, CC BY 4.0, MIT, BSD, Apache-2.0 or public domain. CC BY-SA, NC, GPL-for-data and "no licence" do not count.

| Source | Licence | Link | Key labels | Usable |
|---|---|---|---|---|
| GiantSteps Key (604, Beatport previews) | none (`license: null`, no LICENSE file) | https://github.com/GiantSteps/giantsteps-key-dataset | global | **n** |
| GiantSteps MTG Key (cyanbox's training set) | none (`license: null`) | https://github.com/GiantSteps/giantsteps-mtg-key-dataset | global | **n** |
| Beatport EDM Key (our evaluation set) | CC BY-SA 4.0 | https://zenodo.org/records/1101082 | global | **n** (evaluation only) |
| McGill Billboard | CC0 per mirdata and mirrors; the primary ddmal page was not reachable, so unverified | https://mirdata.readthedocs.io/en/latest/_modules/mirdata/datasets/billboard.html | tonic field, chords; no audio (commercial songs) | y for labels, but no audio |
| Isophonics (Beatles, Queen, Carole King, Zweieck) | no licence stated | http://isophonics.net/content/reference-annotations | key regions; no audio | **n** |
| ChoCo (includes Isophonics, Billboard, etc.) | repo CC BY 4.0 except the NC-SA partitions; the Zenodo record says CC BY-NC-SA 4.0 for everything | https://github.com/smashub/choco, https://zenodo.org/records/7193888 | tonality, mostly symbolic | **n** (conflicting) |
| FMA | metadata CC BY 4.0, audio under per-artist licences | https://github.com/mdeff/fma | none | y for audio (CC BY / CC0 subset only), no labels |
| MTG-Jamendo | metadata CC BY-NC-SA 4.0, non-commercial | https://github.com/MTG/mtg-jamendo-dataset | none | **n** |
| Lakh MIDI | CC BY 4.0 | https://colinraffel.com/projects/lmd/ | key-signature events, often a default C, so unreliable | y (MIDI only; derive keys from the notes) |
| POP909 | MIT | https://github.com/music-x-lab/POP909-Dataset | key annotations; piano MIDI | y (licence text not re-read) |
| GuitarSet | CC BY 4.0 | https://zenodo.org/records/3371780 | key per file; 360 guitar-only excerpts | y, small and off-genre |
| Schubert Winterreise | CC BY 3.0 | https://zenodo.org/records/5139893 | global and local keys | y with attribution; tiny, classical |
| When in Rome | CC BY-SA (new content) | https://github.com/MarkGotham/When-in-Rome | keys, symbolic | **n** |
| MusicNet, Slakh2100 | CC BY 4.0 | https://zenodo.org/records/5120004, https://zenodo.org/records/4599666 | none documented | y, no key labels |
| Krumhansl-Kessler 1982 and Temperley 1999 profiles | published numbers from papers; no dataset licence | Krumhansl 1990; Temperley 1999 | n/a | y (numbers typed from the papers) |
| Sha'ath profiles (libKeyFinder) | code GPL-3.0; the numbers are thesis data quoted by cyanbox | https://github.com/mixxxdj/libkeyfinder | n/a | used only as cyanbox's tie-breaker; flagged below |
| Soundfonts: FluidR3_GM / GeneralUser GS / TimGM6mb | MIT / custom permissive / GPL-2 | Debian copyright files, GeneralUser LICENSE.txt | n/a | y / likely / n. None used here. |
| **Procedural synthetic audio (this work)** | own code, MIT OR Apache-2.0; no external data | `prototyping/analysis-packaging/src/bin/keyfit/synth.rs` | exact by construction | **y** |

The finding: almost no permissively licensed audio has key labels in an EDM-like genre. The clean routes are synthetic audio and theory profiles. This round used both.

## Method

Front end unchanged: the cyanbox port in `fourfour-analysis-rs/src/cyankey.rs` (decimate to 11 kHz, 8192-point STFT, tuning, whitened semitone bands, three 12-bin chroma bands: bass MIDI 36–47, mid 48–71, treble 72–95). New public hooks: `Templates`, `CYANBOX_TEMPLATES`, `spectrum`, `chroma_sums`, `bass_root_counts`, `normalise_bands`, `scores`, `detect_with`. `detect` is `detect_with(&CYANBOX_TEMPLATES, …)`, so the shipped behaviour is identical: re-scored at 58.3% test, 62.7% dev, matching Q21.

The harness is `prototyping/analysis-packaging/src/bin/keyfit/`.

- **Synthetic EDM renderer** (`synth.rs`). Renders 30 s at 11,025 Hz. Tonic is uniform; the minor share is 70% (EDM prior). Each track has:
  - Chord progressions: 12 major and 14 minor pop/EDM patterns.
  - Optional harmonic-minor V, sevenths, inversions.
  - Bass in 4 rhythm patterns, a pad, and an arpeggio/lead with chord tones, scale tones, tonic emphasis and chromatic passing tones.
  - A tuned kick sweep, hats, noise, and random timbres and levels.
  - Global detune of ±35 cents.

  `variety=1` adds:
  - mixolydian major with bVII progressions, dorian and phrygian minor;
  - power and sus chords (no third);
  - a tonic pedal bass;
  - riff-only (techno) tracks;
  - random off-key FX tones.

  `borrow` adds parallel-minor chords in major, and `noise` raises the noise floor. One track costs about 0.1 s single-threaded. Labels are exact by construction, so there is no licence question.
- **Fit**. A transposition-equivariant multinomial logistic regression on the 36 normalised chroma values, the same form as cyanbox's 72 weights plus a major bias. It is trained with Adam, full batch, 300 epochs, L2 0.001.
- **Options tried**:
  - a bass-root histogram channel (12 more inputs; this is the parallel/relative disambiguation feature);
  - a relative-pair rule that switches to the relative key when scores are close and the bass root favours it;
  - start/end weighting (first and last 3 blocks of about 3 s each count double);
  - per-band gains;
  - mixing in a theory profile score;
  - calibrating the major bias on unlabelled audio to a 70% minor prior.
- **Fold**. The final band gains and theory mix are linear, so they fold into one plain `Templates` (`fold=`). The folded file reproduces the score bit for bit through `cyankey::detect_with`, end to end from decoded audio.

Selection protocol: no parameter is fit on Beatport. Configurations were compared on the dev split (n = 399), and the test split (n = 199) is reported for the configuration picked on dev. The choices made this way were drop or keep the bass band, the mix weight and the dataset combination: about 30 discrete runs in total, which is selection, not fitting. Treat dev numbers as optimistic.

## Results (Beatport EDM Key, Camelot exact; denominators are each row's n)

MIREX weighted score = exact + 0.5 fifth + 0.3 relative + 0.2 parallel.

### Test split, n = 199

| System | Labels it was fit on | Exact | Relative | Parallel | Fifth | E+R+F | MIREX |
|---|---|---:|---:|---:|---:|---:|---:|
| cyanbox port (shipped) | GiantSteps MTG (no licence) | **58.3%** | 4.0% | 11.1% | 10.6% | 72.9% | 67.0 |
| keydet Q16 | Beatport dev (CC BY-SA) | 57.3% | 3.5% | 12.6% | 10.1% | 70.9% | n/a |
| Krumhansl profile, 3 bands | none | 36.2% | 4.5% | 27.1% | 7.5% | 48.2% | 46.7 |
| Temperley profile | none | 31.7% | 9.0% | 20.1% | 11.6% | 52.3% | 44.2 |
| Sha'ath profile | none (libKeyFinder numbers) | 40.7% | 6.0% | 25.1% | 8.5% | 55.3% | 51.8 |
| synthetic, plain renderer (3000 tracks) | synthetic | 51.3% | 6.5% | 13.6% | 12.6% | 70.4% | 62.2 |
| synthetic, variety renderer (b+e, 6000) | synthetic | 52.8% | 5.0% | 12.6% | 12.1% | 69.8% | 62.8 |
| **+ bass band dropped + Temperley mix 1.0 (chosen on dev)** | synthetic + published profile | **55.8%** | 5.0% | 11.6% | 12.1% | **72.9%** | **65.6** |

Chosen configuration on the other splits: dev 59.9% (cyanbox 62.7%), all 598 58.5% (cyanbox 61.2%).

Confusion on the test split for the chosen configuration:
- Major truths: 26 predicted major, 34 predicted minor. Minor truths: 18 predicted major, 121 predicted minor.
- Top errors: major → parallel minor 17; minor → minor a fifth up 15; major → relative minor 7; minor → parallel major 6; minor → minor a fourth up 6.
- cyanbox also has major → parallel minor as its top error (16), so mode confusion on major tracks is shared. Our model adds fifth-up confusions.

### What each idea did (exact; plain = renderer without variety, synth-a 3000; b, e = variety renderer, 3000 each)

| Change | Train set | Dev exact (n=399) | Test exact (n=199) |
|---|---|---:|---:|
| Plain renderer | a | 53.4% | 51.3% |
| Variety renderer | b | 58.1% | 53.8% |
| Variety renderer, two seeds | b+e | 57.6% | 52.8% |
| Bass-root histogram channel (relative/parallel feature) | a / b | 52.1% / 57.1% | 53.8% / 49.2% |
| Relative-pair rule (margin 0.5, bass-root vote) | a | 52.1% | 53.3% |
| Start/end tonic weighting (edge x2 over 3 blocks) | b+e | 58.6% | 52.8% |
| Minor share 0.5 instead of 0.7 (EDM prior) | plain, 1500 | 50.4% | 48.7% |
| Minor share 0.6 | variety, 3000 | 57.4% | 49.7% |
| Borrowed chords in major (0.4) | variety, 3000 | 55.6% | 51.3% |
| Higher noise floor (0.4) | variety, 3000 | 57.1% | 52.3% |
| L2 0.01 / 0.0003 (default 0.001) | a / b+e | 53.1% / 57.1% | 53.3% / 52.8% |
| Weights tied across bands (collapsed chroma) | b+e | 57.4% | 50.8% |
| Bass band gain 0 (dropped) / 0.5 | b+e | 59.1% / 58.6% | 55.3% / 55.3% |
| Treble band dropped | b+e | 51.6% | 43.2% |
| Temperley mix 1.0 (bass kept) / Krumhansl mix 1.0 | b+e | 59.1% / 57.9% | 52.8% / 51.3% |
| **Bass dropped + Temperley mix 1.0** | b+e | **59.9%** | **55.8%** |
| Bass dropped, edge weighting / b+e+f | b+e / b+e+f | 58.9% / 57.9% | 55.3% / 55.8% |
| Major bias calibrated to a 70% minor share on 622 unlabelled library tracks | b+e | 56.4% | 51.8% |

What helped:
- Renderer variety: no thirds, modal colour, pedal bass, riffs. +4.7 dev.
- Dropping the bass band: +1.5. The synthetic bass band does not look like real EDM sub/kick content, so learned bass weights hurt on real audio.
- A small theory prior.

What did not help:
- The bass-root histogram, the relative-pair rule, edge weighting, and prior calibration. None separates parallel keys on real audio.

Reading: the synthetic model reaches about 85–90% on its own data, so the limit is the domain gap between rendered and produced EDM, not model capacity.

Runtime: unchanged (same front end). `detect_with` measured 234 ms median per 120 s clip, decode excluded, single thread, box load about 14.

## Licence status of the best configuration

- **Templates** (`benchmark/results/q46/q46-templates.json`, gitignored, not committed): 72 weights and one bias. They are fit only on procedurally rendered audio from our own code, plus the Temperley 1999 profile numbers. No dataset labels went in, so that part is shippable.
- Caveats:
  1. The choice of 2 or 3 discrete settings (bass band off, mix weight 1, which synthetic sets) was made on Beatport dev results. Beatport is CC BY-SA. This is model selection, not fitting, but say so if asked.
  2. `cyankey.rs` still carries cyanbox's confidence calibration (`TEMPERATURE`, `CAL_A/B/C`). Those were fit on GiantSteps MTG too, and they are tuned to cyanbox's score scale. Shipping new templates means refitting them, on synthetic data or by hand, or dropping the calibrated confidence.
  3. The Sha'ath tie-breaker table (libKeyFinder numbers) only fires on exact score ties. It can be removed with no measurable effect.
- Not shippable on accuracy: 55.8% is below both the bar (60%) and the current stage (58.3%).

## Reproduce

```bash
cd prototyping/analysis-packaging
CARGO_TARGET_DIR=$HOME/.cache/fourfour-q46/proto-target cargo build --release --bin keyfit --bin cyankey
K=$HOME/.cache/fourfour-q46/proto-target/release/keyfit
B=/home/merlin/dev/projects/fourfour/benchmark; R=$B/results/q46
$K extract $B/manifests/beatport-edm-key-keyonly-clean-full.corpus.json $B/datasets/beatport-edm-key/audio $R/beatport-feat.json 3
$K synth 3000 4 $R/synth-b.json 4      # see the note below on re-rendering
$K synth 3000 7 $R/synth-e.json 4
$K score $R/beatport-feat.json templ=cyanbox
$K score $R/beatport-feat.json train=$R/synth-b.json,$R/synth-e.json gbass=0 mix=temperley:1 save=$R/q46-synth-model.json fold=$R/q46-templates.json
$HOME/.cache/fourfour-q46/proto-target/release/cyankey $B/manifests/beatport-edm-key-keyonly-clean-full.corpus.json $B/datasets/beatport-edm-key/audio 4 - $R/q46-templates.json
$K extract-dir <unlabelled audio dir> $R/calib.json 3   # for calib=…
```

Note: `synth-b.json` was rendered before the `borrow` option existed. That option draws one extra random number per track, so re-rendering seed 4 now gives a different but statistically equivalent set. `synth-e.json` (seed 7) was rendered after it. The feature files are in `benchmark/results/q46/` (gitignored).

## Next steps

1. A closer domain: render permissively licensed MIDI (Lakh CC BY 4.0, POP909 MIT) through FluidR3_GM (MIT) with production-style processing (sidechain, saturation, reverb, EDM drum loops). Take labels from the notes, not the key-signature events. This is the most likely way to close the 2–5 point gap.
2. Self-labelling: label CC BY / CC0 FMA audio with the synthetic model, keep only confident labels, and refit. This is label-clean and audio-clean when filtered per track.
3. Refit or drop the confidence calibration before any swap. Re-check on a second labelled set (GiantSteps is evaluation-only, licence unclear) before calling any difference of 2–3 points real.

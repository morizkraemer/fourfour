# Waveform display dynamics

How loudness is mapped from analysis data to pixels, what caused near-silent gaps, what we changed (quick wins), and what to do next.

## Pipeline today

```
Analysis (filterbank)          IPC (main.rs)              Display (waveform-display.js)
─────────────────────          ─────────────              ───────────────────────────
Band RMS → √ compress    →     amp = max(L,M,H)/127       displayAmp() → envelope → draw
min_val / max_val peaks        × PWAV calibration         3-band filled paths
(stored, not sent to UI)       r,g,b per-column weights
```

- **Analysis** (`lexicon_waveform.generate_waveform_filterbank`): Butterworth 3-band RMS, `power=0.5` compression, fixed gains. Each column also has `min_val` / `max_val` (sample peaks) used for PWAV preview — not for color height in the UI.
- **IPC** (`waveform_color_json`): `amp` is linear 0–1 (track-peak calibrated via 400-byte PWAV). `r,g,b` are relative band weights per column.
- **Display** (`WaveformDisplay`): height = `amp × weights`, envelope-smoothed, mirrored 3-band paths.

## Why near-silent passages looked like gaps

1. **Linear scale** — Material −40 dB below track peak is ~1% height; −60 dB is invisible.
2. **Envelope decay** — `applyEnvelope` tails decay to zero between transients; quiet input + decay = holes.
3. **Hard skip (overview)** — `ampE[px] < 0.005` skipped drawing entirely.
4. **Semantic mismatch** — Color height uses band energy; PWAV uses sample peaks. Quiet audio with weak bands → `amp ≈ 0` even when the track isn’t silent.

The track-row strip primitive (`waveform.js`) already avoided this with a **display floor** (`floor + p × (1 − floor)`). `WaveformDisplay` did not.

## Quick wins (implemented)

**File:** `prototyping/viewport/design-system/waveform-display.js`

| Change | Detail |
|--------|--------|
| `displayAmp(raw)` | `floor + (1 − floor) × raw^γ` with `floor = 0.045`, `γ = 0.5` (√ lift) |
| Applied before envelope | Overview cache, zoom `_renderLexiconWaveform`, mono overview/fallback |
| `clampEnvelopeFloor()` | Post-envelope clamp so tails stay ≥ floor when signal was present |
| Removed overview skip | No more `continue` on `ampE < 0.005` — quiet pixels draw at floor height |

**Scope:** Compact strip, expanded overview, expanded zoom (same engine).

**Not changed:** Analysis, IPC, ANLZ/PWV7 bytes, re-analyze not required.

### Tuning knobs (display-only)

```javascript
const DISPLAY_AMP_FLOOR = 0.045;  // min silhouette (~4.5% of container)
const DISPLAY_AMP_GAMMA = 0.5;    // 0.5 = sqrt; lower = more lift in quiet material
```

## Suggested next steps

### Tier B — Better `amp` at IPC (medium effort, re-analyze)

Derive display height from sample peaks (already computed in analysis):

- `amp = compress(max(|min_val|, |max_val|))` per column, or
- Blend: `amp = α × peak + (1 − α) × bandMax` so color and loudness both contribute.

Plumb `min_val`/`max_val` through library schema → `get_analysis_data` → `waveform_color_json`.

**Pros:** Correct semantics; aligns color view with PWAV preview.  
**Cons:** Requires re-analyze; USB export semantics unchanged if only IPC mapping changes.

### Tier C — Analysis tuning (hardware-aligned)

Tune `FilterbankParams` in `analysis/.../lexicon_waveform.py`:

- `power` (default 0.5) — lower lifts quiet columns at source
- `peak_hold` — bleed transients into neighbours (already used)
- `gain_*` — per-band fixed gains vs Rekordbox reference
- `scale_mode` — research used `per_track_95`; production uses absolute scale

Validate against Rekordbox PWV7 (`ui/waveform/dev/scripts/fit_transform.py` already compares power-law fits).

**Pros:** Pioneer export matches hardware.  
**Cons:** Touches ANLZ; needs CDJ visual check.

### Tier D — Two-layer rendering (Rekordbox model)

Separate layers in the renderer:

1. **Envelope** — peak/RMS height, compressed, always ≥ floor
2. **Tint** — band ratios from PWV7, independent of height

Pioneer already does this: PWAV = peak silhouette, PWV7 = spectral color.

### Open product questions

1. **Floor vs proportional** — Constant thin silhouette (current quick win) vs noise-shaped quiet texture?
2. **Dynamic range target** — Match Rekordbox exactly, or prioritise readable intros/outros?
3. **Viewport gain** — Optional extra lift when zoomed into a quiet region only?
4. **Dev harness** — Sync `ui/waveform/WaveformDisplay.js` if that path is still used for bisection.

## References

- `prototyping/viewport/design-system/waveform-display.js` — renderer
- `prototyping/viewport/design-system/components/waveform.js` — strip floor pattern
- `pioneer-test-ui/src/main.rs` — `waveform_color_json`, `pwav_band_calibration`
- `analysis/src/fourfour_analysis/backends/lexicon_waveform.py` — filterbank + PWAV
- `ui/waveform/dev/scripts/fit_transform.py` — Rekordbox power-law fit
- `docs/lexicon-reverse-engineering.md` §6 — Lexicon waveform pipeline

## Review checklist (after quick wins)

- [ ] Quiet intro/outro shows continuous silhouette (no full-width gaps)
- [ ] Peaks still hit ~full height on loud material
- [ ] Expanded overview + zoom match compact strip feel
- [ ] Beat grid / cues still readable over lifted floor
- [ ] Compare A/B with Rekordbox on same track (visual, not bit-exact)

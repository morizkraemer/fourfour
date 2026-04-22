"""Lexicon waveform generation — 128-pt FFT, 3-band color.

Port of Lexicon's Worker 160 waveform renderer.
Reference: docs/lexicon-wiki.md §9

Per 256-sample segment at 12kHz:
  1. Min/max → waveform shape
  2. 128-pt FFT → 3-band RMS → RGB color
  3. Smooth with previous segment (50% blend)
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np


# ── Constants (from Lexicon) ──────────────────────────────

TARGET_SR = 12_000
FFT_SIZE = 128
# 80 samples at 12 kHz = 6.67 ms/column = 150 col/sec, matching Rekordbox PWV5/PWV3.
# The FFT window is zero-padded from SEGMENT_WIDTH to FFT_SIZE to preserve band resolution.
SEGMENT_WIDTH = 80

# Frequency bands (Hz) — Nyquist at 12kHz = 6kHz.
#
# At 12kHz / 128-pt FFT, freq_per_bin = 93.75 Hz.  The original (0, 150) low band
# covered only bin 0 (DC, always ~0 in music), making the low channel dead and the
# high band win every column → all-white display.
#
# New bands capture musically meaningful regions:
#   LOW:  60–250 Hz  → kick drum fundamental + sub-bass (bins 1–2, ≈ 2 bins)
#   MID:  250–2500 Hz → bass guitar, synths, snare body  (bins 2–26, ≈ 24 bins)
#   HIGH: 2500–6000 Hz → cymbals, hi-hats, presence     (bins 26–64, ≈ 38 bins)
#
# LOW_WEIGHT is aggressive to match Rekordbox's bass-first visual hierarchy.
LOW_BAND = (60, 250)
MID_BAND = (250, 2500)
HIGH_BAND = (2500, 6000)

# Band weights — low heavily boosted to make kick drum visually dominant,
# matching Rekordbox's perceptual tuning (bass fills the waveform).
LOW_WEIGHT = 5.0
MID_WEIGHT = 1.0
HIGH_WEIGHT = 0.6

# Smoothing: blend with previous segment (0 = no smoothing, 1 = freeze).
# Reduced from 0.5 → sharper transients to match Rekordbox appearance.
MIX_FACTOR = 0.1


@dataclass(frozen=True)
class WaveformColumn:
    """One column of waveform data: shape + color + raw FFT sub-bands."""
    min_val: float
    max_val: float
    r: int
    g: int
    b: int
    # Raw FFT magnitudes, bins 1-64 (93.75 Hz–6 kHz at 12 kHz/128-pt FFT).
    # Per-column normalised: dominant bin = 255.  Used for interactive crossover tuning.
    fft_bands: tuple[int, ...]


def generate_waveform(
    audio: np.ndarray,
    sr: int,
) -> list[WaveformColumn]:
    """Generate waveform display data from 12kHz mono audio.

    Audio should already be resampled to 12kHz
    (use audio_io.preprocess_waveform before calling this).

    Args:
        audio: Mono f32, resampled to 12kHz.
        sr: Sample rate (should be 12000).

    Returns:
        List of WaveformColumn, one per 256-sample segment.
    """
    if len(audio) < SEGMENT_WIDTH:
        return []

    num_segments = len(audio) // SEGMENT_WIDTH
    columns: list[WaveformColumn] = []

    prev_r, prev_g, prev_b = 0, 0, 0

    # Precompute FFT frequency bins
    freq_per_bin = sr / FFT_SIZE
    low_bin_start = int(LOW_BAND[0] / freq_per_bin)
    low_bin_end = max(low_bin_start + 1, int(LOW_BAND[1] / freq_per_bin))
    mid_bin_start = int(MID_BAND[0] / freq_per_bin)
    mid_bin_end = max(mid_bin_start + 1, int(MID_BAND[1] / freq_per_bin))
    high_bin_start = int(HIGH_BAND[0] / freq_per_bin)
    high_bin_end = max(high_bin_start + 1, int(HIGH_BAND[1] / freq_per_bin))

    for i in range(num_segments):
        start = i * SEGMENT_WIDTH
        segment = audio[start:start + SEGMENT_WIDTH]

        # 1. Min/max for waveform shape
        min_val = float(np.min(segment))
        max_val = float(np.max(segment))

        # 2. FFT for color — zero-pad segment to FFT_SIZE to preserve frequency resolution
        fft_input = np.zeros(FFT_SIZE, dtype=np.float32)
        fft_input[:len(segment)] = segment
        spectrum = np.fft.rfft(fft_input)
        magnitudes = np.abs(spectrum)

        # Raw per-bin magnitudes (bins 1-64, skip DC at bin 0), per-column normalised 0-255.
        fft_raw = magnitudes[1:65]  # 64 values covering 93.75 Hz–6 kHz
        fft_max = float(np.max(fft_raw)) if fft_raw.size > 0 else 0.0
        if fft_max > 1e-10:
            fft_bands: tuple[int, ...] = tuple(int(round(float(v) / fft_max * 255)) for v in fft_raw)
        else:
            fft_bands = (0,) * 64

        # RMS per band
        low_rms = _band_rms(magnitudes, low_bin_start, low_bin_end) * LOW_WEIGHT
        mid_rms = _band_rms(magnitudes, mid_bin_start, mid_bin_end) * MID_WEIGHT
        high_rms = _band_rms(magnitudes, high_bin_start, high_bin_end) * HIGH_WEIGHT

        # Normalize to strongest band
        max_band = max(low_rms, mid_rms, high_rms, 1e-10)

        r_raw = int(round(low_rms / max_band * 255))
        g_raw = int(round(mid_rms / max_band * 255))
        b_raw = int(round(high_rms / max_band * 255))

        # 3. Smooth with previous segment
        r = int(round(prev_r * MIX_FACTOR + r_raw * (1 - MIX_FACTOR)))
        g = int(round(prev_g * MIX_FACTOR + g_raw * (1 - MIX_FACTOR)))
        b = int(round(prev_b * MIX_FACTOR + b_raw * (1 - MIX_FACTOR)))

        prev_r, prev_g, prev_b = r, g, b

        columns.append(WaveformColumn(
            min_val=min_val,
            max_val=max_val,
            r=max(0, min(255, r)),
            g=max(0, min(255, g)),
            b=max(0, min(255, b)),
            fft_bands=fft_bands,
        ))

    return columns


def _band_rms(magnitudes: np.ndarray, bin_start: int, bin_end: int) -> float:
    """Compute RMS energy of FFT magnitude bins in a frequency band."""
    if bin_end > len(magnitudes):
        bin_end = len(magnitudes)
    if bin_start >= bin_end:
        return 0.0

    band = magnitudes[bin_start:bin_end]
    if len(band) == 0:
        return 0.0

    return float(np.sqrt(np.mean(band ** 2)))

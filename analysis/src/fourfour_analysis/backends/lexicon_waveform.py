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
SEGMENT_WIDTH = 256  # samples per output column

# Frequency bands (Hz) — Nyquist at 12kHz = 6kHz
LOW_BAND = (0, 150)
MID_BAND = (150, 1500)
HIGH_BAND = (1500, 6000)

# Band weights
LOW_WEIGHT = 1.2
MID_WEIGHT = 1.0
HIGH_WEIGHT = 1.0

# Smoothing
MIX_FACTOR = 0.5  # blend with previous segment


@dataclass(frozen=True)
class WaveformColumn:
    """One column of waveform data: shape + color."""
    min_val: float
    max_val: float
    r: int
    g: int
    b: int


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

        # 2. FFT for color
        spectrum = np.fft.rfft(segment[:FFT_SIZE])
        magnitudes = np.abs(spectrum)

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

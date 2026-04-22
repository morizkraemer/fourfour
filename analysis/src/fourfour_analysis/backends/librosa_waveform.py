"""Librosa waveform generation — Hann window, 256-pt FFT, same 3-band coloring.

Key difference from lexicon_waveform: applies a Hann window before FFT instead of
rectangular zero-padding. Reduces spectral leakage → purer band separation.
"""

from __future__ import annotations

import numpy as np

from fourfour_analysis.backends.lexicon_waveform import (
    WaveformColumn,
    SEGMENT_WIDTH,
    LOW_BAND,
    MID_BAND,
    HIGH_BAND,
    LOW_WEIGHT,
    MID_WEIGHT,
    HIGH_WEIGHT,
    MIX_FACTOR,
    _band_rms,
)

FFT_SIZE = 256  # Larger than lexicon's 128 → better low-freq resolution


def generate_waveform_librosa(audio: np.ndarray, sr: int) -> list[WaveformColumn]:
    """Generate waveform data using Hann-windowed 256-pt FFT.

    Same segment rate and band boundaries as Lexicon, but Hann window instead of
    rectangular zero-padding reduces spectral leakage between bands.

    Args:
        audio: Mono f32, resampled to 12kHz.
        sr: Sample rate (should be 12000).
    """
    if len(audio) < SEGMENT_WIDTH:
        return []

    import librosa  # noqa: F401 — optional dependency, kept for label clarity

    num_segments = len(audio) // SEGMENT_WIDTH
    freq_per_bin = sr / FFT_SIZE

    low_bin_start = int(LOW_BAND[0] / freq_per_bin)
    low_bin_end = max(low_bin_start + 1, int(LOW_BAND[1] / freq_per_bin))
    mid_bin_start = int(MID_BAND[0] / freq_per_bin)
    mid_bin_end = max(mid_bin_start + 1, int(MID_BAND[1] / freq_per_bin))
    high_bin_start = int(HIGH_BAND[0] / freq_per_bin)
    high_bin_end = max(high_bin_start + 1, int(HIGH_BAND[1] / freq_per_bin))

    # Hann window for SEGMENT_WIDTH samples
    hann = np.hanning(SEGMENT_WIDTH).astype(np.float32)

    columns: list[WaveformColumn] = []
    prev_r, prev_g, prev_b = 0, 0, 0

    for i in range(num_segments):
        start = i * SEGMENT_WIDTH
        segment = audio[start : start + SEGMENT_WIDTH]

        min_val = float(np.min(segment))
        max_val = float(np.max(segment))

        # Apply Hann window, zero-pad to FFT_SIZE
        fft_input = np.zeros(FFT_SIZE, dtype=np.float32)
        fft_input[: len(segment)] = segment * hann

        spectrum = np.fft.rfft(fft_input)
        magnitudes = np.abs(spectrum)

        # Raw per-bin magnitudes (bins 1–64), per-column normalised 0–255
        fft_raw = magnitudes[1:65]
        fft_max = float(np.max(fft_raw)) if fft_raw.size > 0 else 0.0
        if fft_max > 1e-10:
            fft_bands: tuple[int, ...] = tuple(
                int(round(float(v) / fft_max * 255)) for v in fft_raw
            )
        else:
            fft_bands = (0,) * 64

        low_rms = _band_rms(magnitudes, low_bin_start, low_bin_end) * LOW_WEIGHT
        mid_rms = _band_rms(magnitudes, mid_bin_start, mid_bin_end) * MID_WEIGHT
        high_rms = _band_rms(magnitudes, high_bin_start, high_bin_end) * HIGH_WEIGHT

        max_band = max(low_rms, mid_rms, high_rms, 1e-10)
        r_raw = int(round(low_rms / max_band * 255))
        g_raw = int(round(mid_rms / max_band * 255))
        b_raw = int(round(high_rms / max_band * 255))

        r = int(round(prev_r * MIX_FACTOR + r_raw * (1 - MIX_FACTOR)))
        g = int(round(prev_g * MIX_FACTOR + g_raw * (1 - MIX_FACTOR)))
        b = int(round(prev_b * MIX_FACTOR + b_raw * (1 - MIX_FACTOR)))
        prev_r, prev_g, prev_b = r, g, b

        columns.append(
            WaveformColumn(
                min_val=min_val,
                max_val=max_val,
                r=max(0, min(255, r)),
                g=max(0, min(255, g)),
                b=max(0, min(255, b)),
                fft_bands=fft_bands,
            )
        )

    return columns

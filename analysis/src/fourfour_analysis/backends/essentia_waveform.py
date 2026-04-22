"""Essentia waveform generation — FrequencyBands with perceptual band splits.

Key difference from lexicon_waveform: uses essentia's FrequencyBands extractor
with wider low band (0–200 Hz) and shifted mid/high split (2 kHz vs 1.5 kHz),
closer to a perceptual/mel-scale weighting. Essentia also normalises band energy
by bandwidth, which affects relative channel balance.
"""

from __future__ import annotations

import numpy as np

from fourfour_analysis.backends.lexicon_waveform import (
    WaveformColumn,
    SEGMENT_WIDTH,
    LOW_WEIGHT,
    MID_WEIGHT,
    HIGH_WEIGHT,
    MIX_FACTOR,
)

# Perceptual band boundaries (Hz) — shifted vs Lexicon's 0/150/1500/6000
LOW_HZ = (0.0, 200.0)
MID_HZ = (200.0, 2000.0)
HIGH_HZ = (2000.0, 6000.0)

FFT_SIZE = 256  # zero-pad 80-sample segment to 256 for frequency resolution


def generate_waveform_essentia(audio: np.ndarray, sr: int) -> list[WaveformColumn]:
    """Generate waveform data using essentia FrequencyBands.

    Args:
        audio: Mono f32, resampled to 12kHz.
        sr: Sample rate (should be 12000).
    """
    if len(audio) < SEGMENT_WIDTH:
        return []

    import essentia.standard as es  # optional dependency

    # Windowing expects frames of exactly `size` samples; zero-padding is done
    # manually after windowing since essentia's Windowing has no zeroPaddingSize param.
    windowing = es.Windowing(type="hann", size=SEGMENT_WIDTH, normalized=False)
    spectrum_alg = es.Spectrum(size=FFT_SIZE)
    band_energy = es.FrequencyBands(
        frequencyBands=[LOW_HZ[0], LOW_HZ[1], MID_HZ[1], HIGH_HZ[1]],
        sampleRate=float(sr),
    )

    num_segments = len(audio) // SEGMENT_WIDTH
    columns: list[WaveformColumn] = []
    prev_r, prev_g, prev_b = 0, 0, 0

    # Precompute FFT bin range for raw export (bins 1–64)
    freq_per_bin = sr / FFT_SIZE

    for i in range(num_segments):
        start = i * SEGMENT_WIDTH
        segment = audio[start : start + SEGMENT_WIDTH]

        min_val = float(np.min(segment))
        max_val = float(np.max(segment))

        windowed = windowing(segment)   # SEGMENT_WIDTH windowed samples
        # Zero-pad to FFT_SIZE manually
        padded = np.zeros(FFT_SIZE, dtype=np.float32)
        padded[:SEGMENT_WIDTH] = windowed
        spec = spectrum_alg(padded)     # FFT_SIZE/2 + 1 = 129 bins

        # Raw per-bin magnitudes for interactive tuning (bins 1–64)
        fft_raw = spec[1:65]
        fft_max = float(np.max(fft_raw)) if len(fft_raw) > 0 else 0.0
        if fft_max > 1e-10:
            fft_bands: tuple[int, ...] = tuple(
                int(round(float(v) / fft_max * 255)) for v in fft_raw
            )
        else:
            fft_bands = (0,) * 64

        # Band energies from essentia FrequencyBands
        energies = band_energy(spec)
        low_e = float(energies[0]) * LOW_WEIGHT
        mid_e = float(energies[1]) * MID_WEIGHT
        high_e = float(energies[2]) * HIGH_WEIGHT

        max_e = max(low_e, mid_e, high_e, 1e-10)
        r_raw = int(round(low_e / max_e * 255))
        g_raw = int(round(mid_e / max_e * 255))
        b_raw = int(round(high_e / max_e * 255))

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

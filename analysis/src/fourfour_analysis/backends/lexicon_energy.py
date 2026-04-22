"""Lexicon energy rating — RMS + tempo + transient density.

Port of Lexicon's Worker 182 energy analysis.
Reference: docs/lexicon-wiki.md §8

Three features combined:
  1. RMS energy in drop regions (50% weight)
  2. Tempo factor: (bpm - 120) / 120 (30% weight)
  3. Transient density: strong beats per second (50% weight)

Output: integer 1-10.
"""

from __future__ import annotations

from typing import Optional

import numpy as np


# ── Constants (from Lexicon) ──────────────────────────────

SEGMENT_DURATION = 0.014       # seconds per segment for transient detection
STRONG_BEAT_RMS_THRESHOLD = 0.3
STRONG_BEAT_RISE_THRESHOLD = 0.2
MIN_STRONG_BEATS = 200

# Normalization constants for transient density
DENSITY_OFFSET = 550_000
DENSITY_SCALE = 150_000

# Drop region defaults (fraction of track)
DROP_START = 0.30
DROP_END = 0.70


def compute_energy(
    audio: np.ndarray,
    sr: int,
    bpm: float,
    drop_regions: Optional[list[tuple[float, float]]] = None,
) -> int:
    """Compute energy rating (1-10) from lowpass-filtered audio.

    Args:
        audio: Mono f32, already lowpass-filtered for tempo analysis.
        sr: Sample rate.
        bpm: Detected BPM.
        drop_regions: List of (start_sec, end_sec) for drop regions.
                      Falls back to 30-70% of track if None.

    Returns:
        Integer energy rating 1-10.
    """
    if len(audio) == 0:
        return 1

    duration = len(audio) / sr

    # Feature 1: RMS energy in drop regions
    rms_score = _rms_energy_score(audio, sr, duration, drop_regions)

    # Feature 2: Tempo factor
    tempo_score = _tempo_factor(bpm)

    # Feature 3: Transient density
    density_score, strong_beat_count = _transient_density_score(audio, sr)

    # Combined score
    score = 0.5 * rms_score + 0.3 * tempo_score + 0.5 * density_score

    # Penalty for "flat" tracks (no strong beats)
    if strong_beat_count <= MIN_STRONG_BEATS:
        score *= 0.2

    # Map to 1-10
    energy = int(round(9 * score)) + 1
    energy = max(1, min(10, energy))

    return energy


def _rms_energy_score(
    audio: np.ndarray,
    sr: int,
    duration: float,
    drop_regions: Optional[list[tuple[float, float]]],
) -> float:
    """Compute RMS energy score (0-1) in drop regions."""
    if drop_regions is None:
        drop_regions = [(duration * DROP_START, duration * DROP_END)]

    # Extract audio from drop regions
    segments = []
    for start, end in drop_regions:
        s = int(start * sr)
        e = int(end * sr)
        s = max(0, min(s, len(audio)))
        e = max(0, min(e, len(audio)))
        if e > s:
            segments.append(audio[s:e])

    if not segments:
        return 0.0

    combined = np.concatenate(segments)
    rms = float(np.sqrt(np.mean(combined ** 2)))

    # Normalize: map RMS to 0-1 range
    # Loudest tracks (post-lowpass) ~0.3, typical ~0.1, quiet ~0.02
    # Use a sigmoid-like mapping
    score = min(rms / 0.15, 1.0)
    return score


def _tempo_factor(bpm: float) -> float:
    """Compute tempo factor (0-1): higher BPM → higher score."""
    if bpm <= 0:
        return 0.0
    factor = (bpm - 120) / 120
    return max(0.0, min(1.0, factor + 0.5))  # shift so 120 BPM = 0.5


def _transient_density_score(
    audio: np.ndarray, sr: int
) -> tuple[float, int]:
    """Compute transient density score (0-1) and strong beat count.

    Returns (score, strong_beat_count).
    """
    segment_len = max(1, int(SEGMENT_DURATION * sr))
    num_segments = len(audio) // segment_len

    if num_segments < 2:
        return 0.0, 0

    # Compute RMS per segment
    rms_values = np.zeros(num_segments)
    for i in range(num_segments):
        start = i * segment_len
        segment = audio[start:start + segment_len]
        rms_values[i] = float(np.sqrt(np.mean(segment ** 2)))

    # Count strong beats: segments where RMS rise > threshold AND RMS > threshold
    strong_beats = 0
    for i in range(1, len(rms_values)):
        rise = rms_values[i] - rms_values[i - 1]
        if rise > STRONG_BEAT_RISE_THRESHOLD and rms_values[i] > STRONG_BEAT_RMS_THRESHOLD:
            strong_beats += 1

    # Density: strong beats per second × 60
    duration = len(audio) / sr
    density = (strong_beats / duration) * 60 if duration > 0 else 0

    # Normalize
    score = (density - DENSITY_OFFSET) / DENSITY_SCALE
    score = max(0.0, min(1.0, score))

    return score, strong_beats

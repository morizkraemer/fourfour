"""Lexicon BPM detection — onset envelope + autocorrelation.

Port of Lexicon's Worker 182 Module 745 tempo analysis.
Reference: docs/lexicon-wiki.md §4

Pipeline:
  1. Energy envelope (short-window RMS)
  2. Onset detection function (half-wave rectified diff)
  3. Autocorrelation of onset envelope → BPM candidates
  4. Octave resolution (test multipliers + genre heuristics)
  5. Fine-tune (integer snap or ±0.05 search)
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Optional

import numpy as np


# Default config (matches Lexicon)
DEFAULT_TEMPO_MIN = 80
DEFAULT_TEMPO_MAX = 180


@dataclass(frozen=True)
class BpmResult:
    """Output of tempo analysis."""
    bpm: float
    confidence: float  # 0-1, ratio of top score to total
    beats: list[float]  # beat positions in seconds


def analyze_tempo(
    audio: np.ndarray,
    sr: int,
    tempo_min: int = DEFAULT_TEMPO_MIN,
    tempo_max: int = DEFAULT_TEMPO_MAX,
) -> Optional[BpmResult]:
    """Detect BPM and beat positions from lowpass-filtered audio.

    Args:
        audio: Mono f32, already lowpass-filtered (<200Hz recommended).
        sr: Sample rate.
        tempo_min: Minimum BPM to consider.
        tempo_max: Maximum BPM to consider.

    Returns:
        BpmResult or None if detection fails.
    """
    if len(audio) < sr:  # less than 1 second
        return None

    duration = len(audio) / sr

    # Step 1: Compute energy envelope
    envelope = _energy_envelope(audio, sr)

    # Step 2: Onset detection function (half-wave rectified diff)
    onset = _onset_detection(envelope)

    # Step 3: Autocorrelation of onset → BPM candidates
    candidates = _autocorrelation_bpm(onset, sr, tempo_min, tempo_max)
    if len(candidates) == 0:
        return None

    # Step 4: Score candidates with full-signal autocorrelation
    def corr_func(bpm: float) -> float:
        return _autocorrelation_score(audio, sr, bpm)

    scored = []
    for bpm, onset_score in candidates:
        full_corr = corr_func(bpm)
        combined = 0.4 * onset_score + 0.6 * full_corr
        scored.append((bpm, combined))
    scored.sort(key=lambda x: x[1], reverse=True)

    best_bpm, best_score = scored[0]

    # Step 5: Octave error resolution
    best_bpm = _resolve_octave(best_bpm, corr_func, tempo_min, tempo_max)

    # Step 6: Fine-tune
    best_bpm = _fine_tune(best_bpm, corr_func)

    # Compute confidence
    total_score = sum(s for _, s in scored)
    confidence = best_score / total_score if total_score > 0 else 0.5
    confidence = max(0.0, min(1.0, confidence))

    # Generate beat positions
    beats = _generate_beats(audio, sr, best_bpm, duration)

    return BpmResult(bpm=best_bpm, confidence=confidence, beats=beats)


def _energy_envelope(audio: np.ndarray, sr: int) -> np.ndarray:
    """Compute energy envelope using short-window RMS.

    Window size: ~10ms (short enough to capture transients at 180 BPM).
    Hop: window_size // 2 for overlap.
    """
    window_ms = 10
    window_size = max(2, int(sr * window_ms / 1000))
    hop = max(1, window_size // 2)

    num_frames = max(0, (len(audio) - window_size) // hop + 1)
    if num_frames == 0:
        return np.array([])

    envelope = np.zeros(num_frames)
    for i in range(num_frames):
        start = i * hop
        chunk = audio[start:start + window_size]
        envelope[i] = np.mean(chunk ** 2)

    return envelope


def _onset_detection(envelope: np.ndarray) -> np.ndarray:
    """Half-wave rectified first-order difference (onset strength)."""
    if len(envelope) < 2:
        return np.array([])

    diff = np.diff(envelope)
    onset = np.maximum(0, diff)
    return onset


def _autocorrelation_bpm(
    onset: np.ndarray,
    sr: int,
    tempo_min: int,
    tempo_max: int,
) -> list[tuple[float, float]]:
    """Compute autocorrelation of onset envelope → BPM candidates.

    The onset envelope sample rate is determined by the energy envelope hop.
    We approximate it as sr / (window_hop).

    Returns list of (bpm, score), sorted by score descending.
    """
    if len(onset) < 10:
        return []

    # Onset envelope effective sample rate
    window_ms = 10
    window_size = max(2, int(sr * window_ms / 1000))
    hop = max(1, window_size // 2)
    onset_sr = sr / hop

    # Autocorrelation via FFT (fast)
    n = len(onset)
    fft_size = 1
    while fft_size < 2 * n:
        fft_size *= 2

    fft_onset = np.fft.rfft(onset, fft_size)
    acf = np.fft.irfft(fft_onset * np.conj(fft_onset))[:n]

    # Normalize by zero-lag
    if acf[0] > 0:
        acf /= acf[0]

    # Convert BPM range to lag range
    min_lag = max(1, int(onset_sr * 60.0 / tempo_max))
    max_lag = min(n - 1, int(onset_sr * 60.0 / tempo_min))

    if min_lag >= max_lag:
        return []

    # Find peaks in autocorrelation within lag range
    candidates = []
    for lag in range(min_lag, max_lag + 1):
        # Check if local maximum
        if lag > 0 and lag < len(acf) - 1:
            if acf[lag] > acf[lag - 1] and acf[lag] > acf[lag + 1]:
                bpm = 60.0 * onset_sr / lag
                score = float(acf[lag])
                candidates.append((bpm, score))

    # Sort by score
    candidates.sort(key=lambda x: x[1], reverse=True)

    # Merge candidates within 2 BPM of each other
    merged = []
    for bpm, score in candidates:
        found = False
        for i, (mb, ms) in enumerate(merged):
            if abs(bpm - mb) < 2.0:
                if score > ms:
                    merged[i] = (bpm, score)
                found = True
                break
        if not found:
            merged.append((bpm, score))

    return merged[:10]


def _autocorrelation_score(
    audio: np.ndarray, sr: int, bpm: float
) -> float:
    """Compute autocorrelation at a given BPM using 4, 8, 16 beat multiples.

    Returns correlation strength.
    """
    beat_samples = int(60.0 / bpm * sr)
    correlations = []

    for mult in [4, 8, 16]:
        offset = beat_samples * mult
        if offset >= len(audio):
            continue

        seg1 = audio[:len(audio) - offset]
        seg2 = audio[offset:]

        if len(seg1) == 0 or len(seg2) == 0:
            continue

        std1 = np.std(seg1)
        std2 = np.std(seg2)
        if std1 == 0 or std2 == 0:
            correlations.append(0.0)
            continue

        corr = np.corrcoef(seg1, seg2)[0, 1]
        if np.isnan(corr):
            correlations.append(0.0)
        else:
            correlations.append(float(corr))

    if len(correlations) == 0:
        return 0.0

    return float(np.mean(correlations))


def _resolve_octave(
    bpm: float,
    corr_func,
    tempo_min: int,
    tempo_max: int,
) -> float:
    """Resolve octave errors (half/double BPM).

    Tests common multipliers and genre heuristics.
    """
    candidates = [(bpm, corr_func(bpm))]

    # Standard multipliers
    for mult in [1.5, 2.0, 0.5, 0.67]:
        candidate = bpm * mult
        if tempo_min <= candidate <= tempo_max:
            score = corr_func(candidate)
            candidates.append((candidate, score))
            # Also try rounded
            candidate_r = round(candidate)
            if tempo_min <= candidate_r <= tempo_max:
                candidates.append((float(candidate_r), corr_func(float(candidate_r))))

    # Genre heuristics
    if 130 <= bpm <= 131:
        candidates.append((174.0, corr_func(174.0)))
    if 85 <= bpm <= 90:
        candidates.append((bpm * 2, corr_func(bpm * 2)))
    if 90 <= bpm <= 115:
        doubled = bpm * 2
        if tempo_min <= doubled <= tempo_max:
            candidates.append((doubled, corr_func(doubled)))

    # Pick best
    best_bpm, best_corr = max(candidates, key=lambda x: x[1])
    return best_bpm


def _fine_tune(
    bpm: float,
    corr_func,
) -> float:
    """Fine-tune BPM: snap to integer or search ±0.05 in 0.001 steps."""
    if bpm <= 0:
        return bpm

    # Check if integer BPM is nearly as good
    bpm_int = round(bpm)
    if bpm_int > 0:
        corr_int = corr_func(float(bpm_int))
        corr_float = corr_func(bpm)
        if corr_int >= 0.95 * corr_float:
            return float(bpm_int)

    # Search ±0.05 in 0.001 steps
    best_bpm = bpm
    best_corr = corr_func(bpm)

    for delta_1000 in range(-50, 51):
        delta = delta_1000 / 1000.0
        candidate = bpm + delta
        if candidate <= 0:
            continue
        corr = corr_func(candidate)
        if corr > best_corr:
            best_corr = corr
            best_bpm = candidate

    return round(best_bpm, 3)


def _generate_beats(
    audio: np.ndarray,
    sr: int,
    bpm: float,
    duration: float,
) -> list[float]:
    """Generate beat grid aligned to detected onsets."""
    beat_interval = 60.0 / bpm

    # Find the first significant onset
    envelope = _energy_envelope(audio, sr)
    window_ms = 10
    window_size = max(2, int(sr * window_ms / 1000))
    hop = max(1, window_size // 2)

    # Find first frame above threshold
    threshold = np.max(envelope) * 0.3 if len(envelope) > 0 else 0
    first_onset_frame = 0
    for i in range(len(envelope)):
        if envelope[i] > threshold:
            first_onset_frame = i
            break

    first_beat_sec = first_onset_frame * hop / sr

    # Snap to nearest beat position
    if first_beat_sec > beat_interval:
        # Walk back to find where beat 0 would be
        n = int(first_beat_sec / beat_interval)
        candidate = first_beat_sec - n * beat_interval
        if candidate < 0:
            candidate += beat_interval
        first_beat_sec = candidate

    # Generate grid
    num_beats = int((duration - first_beat_sec) / beat_interval) + 1
    beats = [first_beat_sec + i * beat_interval for i in range(num_beats)]
    beats = [b for b in beats if 0 <= b <= duration]

    return beats

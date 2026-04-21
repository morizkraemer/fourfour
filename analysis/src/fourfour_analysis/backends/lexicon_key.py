"""Lexicon key detection — custom chroma + Krumhansl-Schmuckler.

Port of Lexicon's Worker 182 key analysis.
Reference: docs/lexicon-wiki.md §5

Pipeline:
  1. Slice audio (25% offset, 50% duration)
  2. FIR lowpass + decimate (done in audio_io.preprocess_key)
  3. Frame with Blackman window (16384 samples, 4096 hop)
  4. FFT per frame → magnitude spectrum
  5. Triangular kernel binning → 72 pitch classes
  6. Average chroma (RMS-weighted across frames)
  7. Krumhansl-Schmuckler: 24 rotations, pick best
  8. Convert to Camelot notation
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Optional

import numpy as np

# ── Constants (from Lexicon reverse engineering) ──────────

# Krumhansl-Kessler key profiles
MAJOR_PROFILE = np.array([
    7.24, 3.50, 3.58, 2.85, 5.82, 4.56, 2.45, 6.99, 3.39, 4.56, 4.07, 4.46
])
MINOR_PROFILE = np.array([
    7.00, 3.14, 4.36, 5.40, 3.67, 4.09, 3.91, 6.20, 3.63, 2.87, 5.35, 3.83
])

# Per-octave weights (6 octaves)
OCTAVE_WEIGHTS = np.array([0.400, 0.556, 0.525, 0.608, 0.599, 0.491])

# Triangular kernel bandwidth in semitones
DSK_BANDWIDTH = 0.9

# FFT/frame config
FFT_SIZE = 2048
FRAME_SIZE = 16384
HOP_SIZE = 4096

# Note names for conversion
NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]

# Camelot wheel mapping
# Key index (0=C, 1=C#, ... 11=B) → Camelot code
MAJOR_CAMELOT = {0: "8B", 1: "3B", 2: "10B", 3: "5B", 4: "12B", 5: "7B",
                 6: "2B", 7: "9B", 8: "4B", 9: "11B", 10: "6B", 11: "1B"}
MINOR_CAMELOT = {0: "5A", 1: "12A", 2: "7A", 3: "2A", 4: "9A", 5: "4A",
                 6: "11A", 7: "6A", 8: "1A", 9: "8A", 10: "3A", 11: "10A"}


@dataclass(frozen=True)
class KeyResult:
    """Output of key detection."""
    key: str          # e.g. "C major", "A minor"
    camelot: str      # e.g. "8A", "8B"
    confidence: float # 0-1
    correlation: float


def detect_key(
    audio: np.ndarray,
    sr: int,
) -> Optional[KeyResult]:
    """Detect musical key from preprocessed audio.

    Audio should already be FIR-lowpassed + decimated to ~4400 Hz
    (use audio_io.preprocess_key before calling this).

    Args:
        audio: Mono f32, preprocessed.
        sr: Effective sample rate after decimation (~4400).

    Returns:
        KeyResult or None if detection fails.
    """
    if len(audio) < FRAME_SIZE:
        return None

    # Step 1: Frame with Blackman window
    chroma = _compute_chroma(audio, sr)
    if chroma is None or np.sum(chroma) == 0:
        return None

    # Step 2: Krumhansl-Schmuckler — test all 24 keys
    best_key_idx = 0
    best_is_major = True
    best_corr = -999.0
    second_best_corr = -999.0

    for key_idx in range(12):
        # Major
        corr = _ks_correlation(chroma, key_idx, is_major=True)
        if corr > best_corr:
            second_best_corr = best_corr
            best_corr = corr
            best_key_idx = key_idx
            best_is_major = True
        elif corr > second_best_corr:
            second_best_corr = corr

        # Minor
        corr = _ks_correlation(chroma, key_idx, is_major=False)
        if corr > best_corr:
            second_best_corr = best_corr
            best_corr = corr
            best_key_idx = key_idx
            best_is_major = False
        elif corr > second_best_corr:
            second_best_corr = corr

    # Confidence: margin between best and second-best
    margin = best_corr - second_best_corr
    strength = min(best_corr / 0.8, 1.0)
    confidence = float(np.tanh(margin * 15 * strength))

    # Convert to names
    note_name = NOTE_NAMES[best_key_idx]
    mode = "major" if best_is_major else "minor"
    key_str = f"{note_name} {mode}"

    camelot_map = MAJOR_CAMELOT if best_is_major else MINOR_CAMELOT
    camelot = camelot_map[best_key_idx]

    return KeyResult(
        key=key_str,
        camelot=camelot,
        confidence=max(0.0, confidence),
        correlation=best_corr,
    )


def _compute_chroma(audio: np.ndarray, sr: int) -> Optional[np.ndarray]:
    """Compute 72-bin chroma vector from framed FFT.

    Returns 72-element array (6 octaves × 12 semitones), RMS-weighted.
    """
    num_octaves = 6
    num_notes = 12
    total_bins = num_octaves * num_notes  # 72

    # Frequency resolution
    freq_per_bin = sr / FFT_SIZE

    # Number of frames
    num_frames = max(1, (len(audio) - FRAME_SIZE) // HOP_SIZE + 1)
    if num_frames == 0:
        return None

    frame_chromas = []
    frame_energies = []

    # Blackman window
    window = np.blackman(FRAME_SIZE)

    for frame_idx in range(num_frames):
        start = frame_idx * HOP_SIZE
        end = start + FRAME_SIZE
        if end > len(audio):
            break

        frame = audio[start:end] * window

        # FFT — only use first FFT_SIZE samples of frame (matches Lexicon)
        spectrum = np.fft.rfft(frame[:FFT_SIZE])
        magnitudes = np.abs(spectrum)

        # Triangular kernel binning → 72 pitch classes
        chroma = np.zeros(total_bins)
        for octave in range(num_octaves):
            for note in range(num_notes):
                midi_note = octave * 12 + note + 12  # start from C1 (MIDI 12)
                center_freq = 440.0 * (2.0 ** ((midi_note - 69) / 12.0))

                if center_freq / freq_per_bin >= len(magnitudes):
                    continue

                # Triangular kernel
                center_bin = center_freq / freq_per_bin
                bandwidth_bins = DSK_BANDWIDTH * (center_freq / 12.0) / freq_per_bin
                half_width = max(1, bandwidth_bins)

                low_bin = max(0, int(center_bin - half_width))
                high_bin = min(len(magnitudes) - 1, int(center_bin + half_width))

                energy = 0.0
                for b in range(low_bin, high_bin + 1):
                    weight = 1.0 - abs(b - center_bin) / half_width
                    weight = max(0.0, weight)
                    energy += magnitudes[b] * weight

                chroma[octave * num_notes + note] = energy

        # Normalize by total energy in frame
        total_energy = np.sum(chroma)
        if total_energy > 0:
            chroma /= total_energy

        frame_chromas.append(chroma)
        frame_energies.append(np.sqrt(np.mean(frame ** 2)))

    if len(frame_chromas) == 0:
        return None

    # RMS-weighted average across frames
    frame_chromas = np.array(frame_chromas)
    frame_energies = np.array(frame_energies)

    total_rms = np.sum(frame_energies)
    if total_rms == 0:
        return None

    weights = frame_energies / total_rms
    avg_chroma = np.sum(frame_chromas * weights[:, np.newaxis], axis=0)

    return avg_chroma


def _ks_correlation(chroma: np.ndarray, key_idx: int, is_major: bool) -> float:
    """Krumhansl-Schmuckler correlation for a given key.

    Tests Pearson correlation between the track's chroma and a rotated key profile,
    weighted by octave weights.
    """
    profile = MAJOR_PROFILE if is_major else MINOR_PROFILE

    # Build 72-element profile vector: profile replicated across octaves with weights
    expected = np.zeros(72)
    for octave in range(6):
        rotated = np.roll(profile, key_idx)
        expected[octave * 12:(octave + 1) * 12] = rotated * OCTAVE_WEIGHTS[octave]

    # Pearson correlation
    if np.std(chroma) == 0 or np.std(expected) == 0:
        return 0.0

    corr = np.corrcoef(chroma, expected)[0, 1]
    if np.isnan(corr):
        return 0.0

    return float(corr)


def note_to_camelot(note_name: str, is_major: bool) -> str:
    """Convert note name + mode to Camelot notation.

    Args:
        note_name: "C", "C#", "D", etc.
        is_major: True for major, False for minor.

    Returns:
        Camelot code like "8A" or "3B".
    """
    idx = NOTE_NAMES.index(note_name)
    camelot_map = MAJOR_CAMELOT if is_major else MINOR_CAMELOT
    return camelot_map[idx]

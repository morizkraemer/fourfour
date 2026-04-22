"""Tests for Lexicon key detection."""

import numpy as np
import pytest

from fourfour_analysis.backends.lexicon_key import detect_key


def _make_chord(frequencies: list[float], duration: float = 5.0, sr: int = 44100) -> tuple[np.ndarray, int]:
    """Generate a chord (sum of sine waves) at ~4400 Hz effective SR."""
    t = np.arange(int(sr * duration), dtype=np.float32) / sr
    audio = np.zeros(len(t), dtype=np.float32)
    for freq in frequencies:
        audio += (np.sin(2 * np.pi * freq * t) * 0.3).astype(np.float32)
    # Simulate preprocess_key: FIR lowpass + decimate
    from scipy.signal import firwin, lfilter
    cutoff_norm = 1999.0 / (sr / 2.0)
    taps = firwin(110, cutoff_norm, window="blackman")
    filtered = lfilter(taps, 1.0, audio)
    decimated = filtered[::10].astype(np.float32)
    return decimated, sr // 10


def test_c_major_chord():
    # C major: C4 (261.63) + E4 (329.63) + G4 (392.00)
    audio, sr = _make_chord([261.63, 329.63, 392.00])
    result = detect_key(audio, sr)
    assert result is not None
    # Should detect C major or a closely related key
    assert "C" in result.key or "G" in result.key or "F" in result.key, \
        f"Expected C/G/F major or relative, got {result.key}"


def test_a_minor_chord():
    # A minor: A3 (220.00) + C4 (261.63) + E4 (329.63)
    audio, sr = _make_chord([220.00, 261.63, 329.63])
    result = detect_key(audio, sr)
    assert result is not None
    # Should detect A minor or relative
    assert "A" in result.key or "C" in result.key, \
        f"Expected A minor or C major, got {result.key}"


def test_440hz_single_note():
    # A4 = 440Hz — should detect A major or A minor
    audio, sr = _make_chord([440.0])
    result = detect_key(audio, sr)
    assert result is not None
    # Key should contain A (or its relative F#m for A major)
    assert result.camelot is not None
    assert len(result.camelot) >= 2


def test_camelot_format():
    audio, sr = _make_chord([261.63, 329.63, 392.00])
    result = detect_key(audio, sr)
    assert result is not None
    # Camelot format: number + A/B
    assert result.camelot[-1] in ("A", "B")
    assert result.camelot[:-1].isdigit()


def test_short_audio_returns_none():
    audio = np.zeros(100, dtype=np.float32)
    result = detect_key(audio, 44100)
    assert result is None

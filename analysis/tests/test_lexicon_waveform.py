"""Tests for Lexicon waveform generation."""

import numpy as np
import pytest

from fourfour_analysis.backends.lexicon_waveform import generate_waveform, TARGET_SR, SEGMENT_WIDTH


def _sine(freq: float, duration: float = 2.0, sr: int = 12000) -> np.ndarray:
    """Generate a sine wave at target sample rate."""
    t = np.arange(int(sr * duration), dtype=np.float32) / sr
    return (np.sin(2 * np.pi * freq * t) * 0.8).astype(np.float32)


def test_basic_generation():
    audio = _sine(440.0, 2.0)
    columns = generate_waveform(audio, TARGET_SR)
    assert len(columns) > 0
    # Expected: 2s × 12000 / 256 ≈ 93 segments
    expected = int(len(audio) / SEGMENT_WIDTH)
    assert abs(len(columns) - expected) <= 1


def test_all_zeros():
    audio = np.zeros(12000 * 2, dtype=np.float32)
    columns = generate_waveform(audio, TARGET_SR)
    assert len(columns) > 0
    # All peaks should be near zero
    for col in columns:
        assert abs(col.min_val) < 0.01
        assert abs(col.max_val) < 0.01


def test_sine_peaks_nonzero():
    audio = _sine(440.0, 2.0)
    columns = generate_waveform(audio, TARGET_SR)
    # At least some peaks should be non-zero
    nonzero = [c for c in columns if abs(c.max_val) > 0.01 or abs(c.min_val) > 0.01]
    assert len(nonzero) > len(columns) * 0.5, "Majority of columns should have non-zero peaks"


def test_color_values_in_range():
    audio = _sine(440.0, 2.0) + _sine(3000.0, 2.0)  # multi-frequency
    columns = generate_waveform(audio, TARGET_SR)
    for col in columns:
        assert 0 <= col.r <= 255
        assert 0 <= col.g <= 255
        assert 0 <= col.b <= 255


def test_bass_heavy_dominates_red():
    """Low frequency content should produce more red channel."""
    bass = _sine(80.0, 2.0)  # well within low band (0-150 Hz)
    columns = generate_waveform(bass, TARGET_SR)
    # Average R should be higher than G or B
    avg_r = np.mean([c.r for c in columns])
    avg_g = np.mean([c.g for c in columns])
    avg_b = np.mean([c.b for c in columns])
    assert avg_r >= avg_g or avg_r >= avg_b, \
        f"Bass-heavy should favor red: R={avg_r:.1f} G={avg_g:.1f} B={avg_b:.1f}"


def test_short_audio_returns_empty():
    audio = np.zeros(10, dtype=np.float32)
    columns = generate_waveform(audio, TARGET_SR)
    assert columns == []


def test_segment_count():
    """Verify segment count matches expected."""
    duration = 5.0
    audio = _sine(1000.0, duration)
    columns = generate_waveform(audio, TARGET_SR)
    expected = int(duration * TARGET_SR) // SEGMENT_WIDTH
    assert len(columns) == expected

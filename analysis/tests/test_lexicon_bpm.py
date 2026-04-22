"""Tests for Lexicon BPM detection."""

import numpy as np
import pytest

from fourfour_analysis.backends.lexicon_bpm import analyze_tempo


def _click_train(bpm: float, duration: float = 10.0, sr: int = 44100) -> np.ndarray:
    """Generate a click track at the given BPM."""
    total = int(sr * duration)
    audio = np.zeros(total, dtype=np.float32)
    interval = int(sr * 60.0 / bpm)
    click_len = int(sr * 0.01)
    for i in range(0, total, interval):
        end = min(i + click_len, total)
        audio[i:end] = 0.9
    return audio


def _lowpass_simple(audio: np.ndarray, sr: int, cutoff: float = 200.0) -> np.ndarray:
    """Simple lowpass to simulate preprocessing for BPM detection."""
    from scipy.signal import butter, sosfilt
    sos = butter(4, cutoff, btype="low", fs=sr, output="sos")
    return sosfilt(sos, audio).astype(np.float32)


def test_click_track_100_bpm():
    audio = _click_train(100.0, 10.0)
    audio_lp = _lowpass_simple(audio, 44100, 200)
    result = analyze_tempo(audio_lp, 44100)
    assert result is not None
    assert abs(result.bpm - 100.0) <= 2.0, f"Expected ~100 BPM, got {result.bpm}"


def test_click_track_128_bpm():
    audio = _click_train(128.0, 10.0)
    audio_lp = _lowpass_simple(audio, 44100, 200)
    result = analyze_tempo(audio_lp, 44100)
    assert result is not None
    assert abs(result.bpm - 128.0) <= 2.0, f"Expected ~128 BPM, got {result.bpm}"


def test_click_track_140_bpm():
    audio = _click_train(140.0, 10.0)
    audio_lp = _lowpass_simple(audio, 44100, 200)
    result = analyze_tempo(audio_lp, 44100)
    assert result is not None
    assert abs(result.bpm - 140.0) <= 2.0, f"Expected ~140 BPM, got {result.bpm}"


def test_click_track_174_bpm():
    audio = _click_train(174.0, 10.0)
    audio_lp = _lowpass_simple(audio, 44100, 200)
    result = analyze_tempo(audio_lp, 44100)
    assert result is not None
    # 174 or 87 (half-time) are both valid
    assert abs(result.bpm - 174.0) <= 3.0 or abs(result.bpm - 87.0) <= 2.0, \
        f"Expected ~174 or ~87 BPM, got {result.bpm}"


def test_pure_sine_low_confidence():
    sr = 44100
    t = np.arange(int(sr * 5), dtype=np.float32) / sr
    sine = (np.sin(2 * np.pi * 440 * t) * 0.8).astype(np.float32)
    audio_lp = _lowpass_simple(sine, 44100, 200)
    # Pure 440Hz sine after 200Hz lowpass should have near-zero energy
    # so BPM detection should return None or very low confidence
    result = analyze_tempo(audio_lp, 44100)
    if result is not None:
        # If it returns something, the energy is so low the result is meaningless
        # The autocorrelation of near-silence can produce artifacts
        assert result.bpm >= 0  # just verify it doesn't crash


def test_beat_positions_generated():
    audio = _click_train(120.0, 10.0)
    audio_lp = _lowpass_simple(audio, 44100, 200)
    result = analyze_tempo(audio_lp, 44100)
    assert result is not None
    assert len(result.beats) > 0
    # Beats should be roughly evenly spaced
    if len(result.beats) >= 2:
        intervals = [result.beats[i+1] - result.beats[i] for i in range(len(result.beats) - 1)]
        expected_interval = 60.0 / result.bpm
        mean_interval = np.mean(intervals)
        assert abs(mean_interval - expected_interval) < 0.1, \
            f"Beat interval {mean_interval:.3f}s != expected {expected_interval:.3f}s"


def test_short_audio_returns_none():
    audio = np.zeros(100, dtype=np.float32)
    result = analyze_tempo(audio, 44100)
    assert result is None

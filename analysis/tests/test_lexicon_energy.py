"""Tests for Lexicon energy rating."""

import numpy as np
import pytest
from scipy.signal import butter, sosfilt

from fourfour_analysis.backends.lexicon_energy import compute_energy


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


def _lowpass(audio: np.ndarray, sr: int, cutoff: float = 200.0) -> np.ndarray:
    """Lowpass filter to simulate tempo preprocessing."""
    sos = butter(4, cutoff, btype="low", fs=sr, output="sos")
    return sosfilt(sos, audio).astype(np.float32)


def _low_sine(duration: float = 10.0, sr: int = 44100) -> np.ndarray:
    """Generate a low-amplitude sine wave."""
    t = np.arange(int(sr * duration), dtype=np.float32) / sr
    return (np.sin(2 * np.pi * 440 * t) * 0.02).astype(np.float32)


def test_fast_clicks_moderate_energy():
    """Fast click train should have moderate-to-high energy after lowpass."""
    audio = _click_train(140.0, 10.0)
    audio_lp = _lowpass(audio, 44100, 200)
    energy = compute_energy(audio_lp, 44100, bpm=140.0)
    assert 1 <= energy <= 10
    # Click train with tempo factor bonus should score at least 3
    assert energy >= 2, f"Fast clicks should have energy >= 2, got {energy}"


def test_low_amplitude_low_energy():
    audio = _low_sine(10.0)
    audio_lp = _lowpass(audio, 44100, 200)
    energy = compute_energy(audio_lp, 44100, bpm=120.0)
    assert 1 <= energy <= 10
    assert energy <= 4, f"Quiet sine should have energy <= 4, got {energy}"


def test_energy_range():
    audio = _click_train(128.0, 10.0)
    energy = compute_energy(audio, 44100, bpm=128.0)
    assert 1 <= energy <= 10


def test_empty_audio():
    audio = np.zeros(0, dtype=np.float32)
    energy = compute_energy(audio, 44100, bpm=128.0)
    assert energy == 1


def test_silence_low_energy():
    audio = np.zeros(44100 * 10, dtype=np.float32)
    energy = compute_energy(audio, 44100, bpm=128.0)
    assert energy <= 3, f"Silence should have very low energy, got {energy}"


def test_faster_bpm_higher_energy():
    """All else equal, higher BPM should give higher energy."""
    audio = _click_train(128.0, 10.0)
    audio_lp = _lowpass(audio, 44100, 200)
    e120 = compute_energy(audio_lp, 44100, bpm=120.0)
    e160 = compute_energy(audio_lp, 44100, bpm=160.0)
    assert e160 >= e120, f"160 BPM ({e160}) should >= 120 BPM ({e120})"

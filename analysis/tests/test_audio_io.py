"""Tests for audio I/O: loading, preprocessing, filter chains."""

import tempfile
from pathlib import Path

import numpy as np
import pytest

from fourfour_analysis.audio_io import (
    load_audio,
    resample_audio,
    lowpass_cascade,
    preprocess_tempo,
    preprocess_key,
    preprocess_waveform,
)


# ── Helpers ──────────────────────────────────────────────

def _write_wav(path: Path, samples: np.ndarray, sr: int = 44100) -> None:
    """Write mono f32 samples to WAV using soundfile."""
    import soundfile as sf
    sf.write(str(path), samples, sr)


def _sine(freq: float, duration: float, sr: int = 44100) -> np.ndarray:
    """Generate a sine wave."""
    t = np.arange(int(sr * duration), dtype=np.float32) / sr
    return (np.sin(2 * np.pi * freq * t) * 0.8).astype(np.float32)


def _click_train(bpm: float, duration: float, sr: int = 44100) -> np.ndarray:
    """Generate a click track: short impulses at regular BPM intervals."""
    total_samples = int(sr * duration)
    audio = np.zeros(total_samples, dtype=np.float32)
    beat_interval = int(sr * 60.0 / bpm)
    click_len = int(sr * 0.01)  # 10ms click
    for i in range(0, total_samples, beat_interval):
        end = min(i + click_len, total_samples)
        audio[i:end] = 0.9
    return audio


@pytest.fixture
def sine_440_1s(tmp_path):
    """1-second 440Hz sine wave WAV file."""
    p = tmp_path / "sine_440.wav"
    _write_wav(p, _sine(440.0, 1.0))
    return p


@pytest.fixture
def click_128_5s(tmp_path):
    """5-second click track at 128 BPM."""
    p = tmp_path / "click_128.wav"
    _write_wav(p, _click_train(128.0, 5.0))
    return p


# ── load_audio ──────────────────────────────────────────

def test_load_wav_mono(sine_440_1s):
    samples, sr = load_audio(sine_440_1s)
    assert sr == 44100
    assert samples.ndim == 1
    assert samples.dtype == np.float32
    assert len(samples) == 44100


def test_load_wav_resample(sine_440_1s):
    samples, sr = load_audio(sine_440_1s, sr=22050)
    assert sr == 22050
    assert len(samples) == 22050


def test_load_nonexistent(tmp_path):
    with pytest.raises(FileNotFoundError):
        load_audio(tmp_path / "nonexistent.wav")


# ── resample_audio ──────────────────────────────────────

def test_resample_identity():
    audio = _sine(440.0, 1.0)
    out, sr = resample_audio(audio, 44100, 44100)
    assert sr == 44100
    assert len(out) == len(audio)


def test_resample_down():
    audio = _sine(440.0, 1.0, sr=44100)
    out, sr = resample_audio(audio, 44100, 22050)
    assert sr == 22050
    assert abs(len(out) - 22050) < 10


# ── lowpass_cascade ─────────────────────────────────────

def test_lowpass_removes_high_freq():
    sr = 44100
    # Mix 100Hz + 5000Hz
    audio = _sine(100.0, 0.5, sr) + _sine(5000.0, 0.5, sr)
    filtered = lowpass_cascade(audio, sr, [200], order=4)
    # High frequency energy should be drastically reduced
    # Check by looking at zero crossings — low freq should dominate
    crossings_before = np.sum(np.diff(np.sign(audio)) != 0)
    crossings_after = np.sum(np.diff(np.sign(filtered)) != 0)
    assert crossings_after < crossings_before


def test_lowpass_noop_if_above_nyquist():
    audio = _sine(100.0, 0.5, 44100)
    filtered = lowpass_cascade(audio, 44100, [25000])  # above Nyquist
    # Should be essentially unchanged (signal is well below both cutoffs)
    np.testing.assert_allclose(filtered, audio, atol=1e-4)


# ── preprocess_tempo ────────────────────────────────────

def test_preprocess_tempo_returns_same_sr():
    sr = 44100
    audio = _sine(100.0, 1.0, sr)
    out, out_sr = preprocess_tempo(audio, sr)
    assert out_sr == sr
    assert len(out) == len(audio)


def test_preprocess_tempo_attenuates_highs():
    sr = 44100
    # Low content only
    low = _sine(80.0, 1.0, sr) * 0.5
    # High content only
    high = _sine(5000.0, 1.0, sr) * 0.5
    mixed = low + high

    filtered, _ = preprocess_tempo(mixed, sr)

    # The 5000Hz component should be heavily attenuated
    # Compare energy in the filtered signal to the original low component
    energy_low = np.sum(low ** 2)
    energy_filtered = np.sum(filtered ** 2)
    energy_original = np.sum(mixed ** 2)

    # Filtered should be closer to just the low component
    assert energy_filtered < energy_original
    assert energy_filtered > energy_low * 0.1  # but still has some energy


# ── preprocess_key ──────────────────────────────────────

def test_preprocess_key_decimates():
    sr = 44100
    audio = _sine(440.0, 2.0, sr)
    out, out_sr = preprocess_key(audio, sr)
    assert out_sr < sr  # should be decimated
    assert out_sr == sr // round(sr / 4400)
    assert len(out) < len(audio)


# ── preprocess_waveform ─────────────────────────────────

def test_preprocess_waveform_target_sr():
    sr = 44100
    audio = _sine(440.0, 2.0, sr)
    out, out_sr = preprocess_waveform(audio, sr)
    assert out_sr == 12000
    expected_len = int(len(audio) * 12000 / 44100)
    assert abs(len(out) - expected_len) < 50

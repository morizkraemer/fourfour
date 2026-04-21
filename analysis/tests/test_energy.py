import numpy as np
import soundfile as sf
import pytest


def test_compute_energy_returns_score_1_to_10(tmp_path):
    """Energy score should be between 1 and 10."""
    from fourfour_analysis.energy import compute_energy

    # Generate 5 seconds of white noise (high energy)
    sr = 22050
    signal = np.random.randn(sr * 5).astype(np.float32) * 0.5
    path = tmp_path / "noise.wav"
    sf.write(str(path), signal, sr)

    result = compute_energy(str(path))

    assert result is not None
    assert 1 <= result["score"] <= 10
    assert result["label"] in ("low", "medium", "high")


def test_compute_energy_silence_is_low(tmp_path):
    """Silent audio should have low energy."""
    from fourfour_analysis.energy import compute_energy

    sr = 22050
    signal = np.zeros(sr * 5, dtype=np.float32)
    path = tmp_path / "silence.wav"
    sf.write(str(path), signal, sr)

    result = compute_energy(str(path))

    assert result is not None
    assert result["score"] <= 3
    assert result["label"] == "low"


def test_compute_energy_too_short_returns_none(tmp_path):
    """Audio shorter than 3 seconds should return None."""
    from fourfour_analysis.energy import compute_energy

    sr = 22050
    signal = np.zeros(sr * 1, dtype=np.float32)  # 1 second
    path = tmp_path / "short.wav"
    sf.write(str(path), signal, sr)

    result = compute_energy(str(path))

    assert result is None

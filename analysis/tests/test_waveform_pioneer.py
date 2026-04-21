import pytest
import numpy as np
from unittest.mock import patch, MagicMock


def test_generate_pioneer_detail_entry_count():
    """Detail waveform should have duration*150 entries."""
    sr = 44100
    duration = 4.0
    samples = int(sr * duration)
    audio = np.random.randn(samples).astype(np.float32) * 0.5

    with patch("soundfile.read", return_value=(audio.reshape(-1, 1), sr)):
        with patch("soundfile.info") as mock_info:
            mock_info.return_value = MagicMock(duration=duration, samplerate=sr)
            from fourfour_analysis.waveform import generate_pioneer_3band
            result = generate_pioneer_3band("/fake/track.mp3")

    expected_count = int(duration * 150)
    assert len(result["detail"]) == expected_count
    assert len(result["overview"]) == 1200


def test_generate_pioneer_detail_value_range():
    """Each entry should be 3 values in 0-255 range."""
    sr = 44100
    duration = 2.0
    samples = int(sr * duration)
    audio = np.random.randn(samples).astype(np.float32) * 0.5

    with patch("soundfile.read", return_value=(audio.reshape(-1, 1), sr)):
        with patch("soundfile.info") as mock_info:
            mock_info.return_value = MagicMock(duration=duration, samplerate=sr)
            from fourfour_analysis.waveform import generate_pioneer_3band
            result = generate_pioneer_3band("/fake/track.mp3")

    for entry in result["detail"]:
        assert len(entry) == 3
        assert all(0 <= v <= 255 for v in entry)

    for entry in result["overview"]:
        assert len(entry) == 3
        assert all(0 <= v <= 255 for v in entry)

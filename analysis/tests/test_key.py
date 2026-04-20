import pytest
import numpy as np
from fourfour_analysis.key import detect_key, to_camelot


def test_to_camelot_a_minor():
    assert to_camelot("A minor") == "8A"


def test_to_camelot_c_major():
    assert to_camelot("C major") == "12B"


def test_to_camelot_unknown_returns_none():
    assert to_camelot("X augmented") is None


def test_detect_key_returns_camelot_string(tmp_path):
    """detect_key should return a Camelot string like '8A'."""
    # Generate a 3-second A440 sine wave as a test file
    import soundfile as sf
    sr = 22050
    t = np.linspace(0, 3.0, sr * 3, endpoint=False)
    signal = 0.5 * np.sin(2 * np.pi * 440 * t)  # A440 = A major/minor
    path = tmp_path / "test_tone.wav"
    sf.write(str(path), signal.astype(np.float32), sr)

    result = detect_key(str(path))

    # A440 should detect as A major or A minor — both valid for a pure tone
    assert result is not None
    assert result in ("8A", "9B", "11A", "4A")  # A minor, A major, or nearby keys

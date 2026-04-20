import numpy as np
import soundfile as sf
import pytest


def test_extract_peaks_returns_correct_count(tmp_path):
    """Should return exactly target_points (min, max) pairs."""
    from fourfour_analysis.waveform import extract_peaks

    sr = 44100
    signal = np.random.randn(sr * 10).astype(np.float32) * 0.5
    path = tmp_path / "test.wav"
    sf.write(str(path), signal, sr)

    peaks = extract_peaks(str(path), target_points=2000)

    assert len(peaks) == 2000
    assert all(len(p) == 2 for p in peaks)  # (min, max) pairs
    assert all(p[0] <= p[1] for p in peaks)  # min <= max


def test_extract_color_bands_returns_rgb(tmp_path):
    """Should return points with amp, r, g, b fields."""
    from fourfour_analysis.waveform import extract_color_bands

    sr = 44100
    signal = np.random.randn(sr * 5).astype(np.float32) * 0.3
    path = tmp_path / "test.wav"
    sf.write(str(path), signal, sr)

    bands = extract_color_bands(str(path), points=400)

    assert len(bands) == 400
    assert all("amp" in b and "r" in b and "g" in b and "b" in b for b in bands)
    assert all(0.0 <= b["amp"] <= 1.0 for b in bands)


def test_generate_pwav_preview_400_bytes(tmp_path):
    """PWAV preview must be exactly 400 bytes matching Pioneer format."""
    from fourfour_analysis.waveform import generate_pwav_preview

    sr = 44100
    signal = np.random.randn(sr * 10).astype(np.float32) * 0.5
    path = tmp_path / "test.wav"
    sf.write(str(path), signal, sr)

    preview = generate_pwav_preview(str(path))

    assert len(preview) == 400
    # Each byte: low 5 bits = height (0-31), high 3 bits = whiteness (0-7)
    for byte in preview:
        height = byte & 0x1F
        whiteness = (byte >> 5) & 0x07
        assert 0 <= height <= 31
        assert 0 <= whiteness <= 7

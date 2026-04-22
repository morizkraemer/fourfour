import json
import numpy as np
import soundfile as sf
import pytest


@pytest.fixture
def test_audio(tmp_path):
    """Create a 5-second sine wave test file."""
    sr = 44100
    t = np.linspace(0, 5.0, sr * 5, endpoint=False)
    signal = (0.5 * np.sin(2 * np.pi * 440 * t)).astype(np.float32)
    path = tmp_path / "test.wav"
    sf.write(str(path), signal, sr)
    return str(path)


def test_analyze_track_returns_all_fields(test_audio):
    from fourfour_analysis.analyze import analyze_track

    result = analyze_track(test_audio)

    assert "path" in result
    assert "bpm" in result
    assert "key" in result
    assert "energy" in result
    assert "beats" in result
    assert "cue_points" in result
    assert "waveform_preview" in result
    assert "waveform_color" in result
    assert "waveform_peaks" in result
    assert "errors" in result
    assert result["path"] == test_audio


def test_analyze_track_waveform_preview_is_400_ints(test_audio):
    from fourfour_analysis.analyze import analyze_track

    result = analyze_track(test_audio)

    assert len(result["waveform_preview"]) == 400
    assert all(isinstance(b, int) and 0 <= b <= 255 for b in result["waveform_preview"])


def test_analyze_track_result_is_json_serializable(test_audio):
    from fourfour_analysis.analyze import analyze_track

    result = analyze_track(test_audio)

    # Must not raise
    serialized = json.dumps(result)
    assert isinstance(serialized, str)

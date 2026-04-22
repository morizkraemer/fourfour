import json
import numpy as np
import soundfile as sf
import pytest


@pytest.fixture
def test_audio(tmp_path):
    sr = 44100
    t = np.linspace(0, 5.0, sr * 5, endpoint=False)
    signal = (0.5 * np.sin(2 * np.pi * 440 * t)).astype(np.float32)
    path = tmp_path / "test.wav"
    sf.write(str(path), signal, sr)
    return str(path)


def test_cli_single_file_json_output_payload(test_audio, capsys):
    from fourfour_analysis.cli import _module_analyze_main

    _module_analyze_main([test_audio, "--json"])

    data = json.loads(capsys.readouterr().out)
    assert isinstance(data, list)
    assert len(data) == 1
    assert data[0]["path"] == test_audio


def test_fourfour_analyze_json_output_payload(test_audio, capsys, monkeypatch):
    from fourfour_analysis.cli import analyze_main

    monkeypatch.setattr("sys.argv", ["fourfour-analyze", test_audio, "--json"])

    analyze_main()

    data = json.loads(capsys.readouterr().out)
    assert data["path"] == test_audio
    assert "bpm" in data
    assert "key" in data
    assert "energy" in data
    assert "beats" in data
    assert "cue_points" in data
    assert "waveform_preview" in data
    assert "waveform_color" in data
    assert "waveform_peaks" in data
    assert "pioneer_3band_detail" in data
    assert "pioneer_3band_overview" in data


def test_cli_multiple_files(test_audio, tmp_path, capsys):
    from fourfour_analysis.cli import _module_analyze_main

    # Create a second file
    sr = 44100
    signal = np.zeros(sr * 5, dtype=np.float32)
    path2 = tmp_path / "test2.wav"
    sf.write(str(path2), signal, sr)

    _module_analyze_main([test_audio, str(path2), "--json", "--workers", "1"])

    data = json.loads(capsys.readouterr().out)
    assert len(data) == 2

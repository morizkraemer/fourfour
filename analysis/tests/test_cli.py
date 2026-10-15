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


def test_module_analyze_json_output_payload(test_audio, capsys, monkeypatch):
    from fourfour_analysis.cli import main

    monkeypatch.setattr("sys.argv", ["fourfour_analysis", "analyze", test_audio, "--json"])

    main()

    data = json.loads(capsys.readouterr().out)
    assert list(data) == ["deeprhythm_essentia"]
    assert data["deeprhythm_essentia"]["status"] == "ok"


def test_fourfour_analyze_json_output_payload(test_audio, capsys, monkeypatch):
    from fourfour_analysis.cli import analyze_main

    monkeypatch.setattr("sys.argv", ["fourfour-analyze", test_audio, "--json"])

    analyze_main()

    data = json.loads(capsys.readouterr().out)["deeprhythm_essentia"]
    assert data["status"] == "ok"
    assert "bpm" in data
    assert "key" in data
    assert "energy" in data
    assert "beats" in data
    assert "cue_points" in data
    assert "waveform_preview" in data
    assert "waveform_colors" in data
    assert "waveform_peaks" in data


def test_fourfour_analyze_missing_file_exits(tmp_path, monkeypatch):
    from fourfour_analysis.cli import analyze_main

    monkeypatch.setattr("sys.argv", ["fourfour-analyze", str(tmp_path / "missing.wav"), "--json"])

    with pytest.raises(SystemExit) as exc:
        analyze_main()
    assert exc.value.code == 1

"""Tests for corpus builder (manifest)."""

import json
import tempfile
from pathlib import Path

import numpy as np
import pytest
import soundfile as sf

from fourfour_analysis.manifest import build_corpus, load_corpus


def _write_wav(path: Path, samples: np.ndarray, sr: int = 44100) -> None:
    sf.write(str(path), samples, sr)


def _sine(freq: float, duration: float, sr: int = 44100) -> np.ndarray:
    t = np.arange(int(sr * duration), dtype=np.float32) / sr
    return (np.sin(2 * np.pi * freq * t) * 0.8).astype(np.float32)


@pytest.fixture
def corpus_dir(tmp_path):
    """Create a temp directory with a few WAV files."""
    for i in range(3):
        p = tmp_path / f"track_{i:03d}.wav"
        _write_wav(p, _sine(440 + i * 100, 2.0))
    return tmp_path


def test_build_corpus_creates_json(corpus_dir, tmp_path):
    out = tmp_path / "output"
    result = build_corpus(corpus_dir, "test-corpus", out)
    assert result.is_file()
    assert result.name == "test-corpus.corpus.json"


def test_build_corpus_has_entries(corpus_dir, tmp_path):
    out = tmp_path / "output"
    result = build_corpus(corpus_dir, "test", out)
    data = json.loads(result.read_text())
    assert data["num_entries"] == 3
    assert len(data["entries"]) == 3


def test_build_corpus_entries_have_fingerprints(corpus_dir, tmp_path):
    out = tmp_path / "output"
    result = build_corpus(corpus_dir, "test", out)
    data = json.loads(result.read_text())
    for entry in data["entries"]:
        assert "content_fingerprint" in entry
        assert len(entry["content_fingerprint"]) == 64  # SHA256 hex


def test_build_corpus_ignores_non_audio(tmp_path):
    # Create a text file alongside a wav
    (tmp_path / "readme.txt").write_text("not audio")
    p = tmp_path / "track.wav"
    _write_wav(p, _sine(440, 1.0))

    out = tmp_path / "output"
    result = build_corpus(tmp_path, "test", out)
    data = json.loads(result.read_text())
    assert data["num_entries"] == 1


def test_load_corpus_roundtrip(corpus_dir, tmp_path):
    out = tmp_path / "output"
    result = build_corpus(corpus_dir, "roundtrip", out)

    entries = load_corpus(result)
    assert len(entries) == 3
    assert all(e.id for e in entries)
    assert all(e.path for e in entries)
    assert all(len(e.content_fingerprint) == 64 for e in entries)


def test_load_corpus_missing_file(tmp_path):
    with pytest.raises(FileNotFoundError):
        load_corpus(tmp_path / "nonexistent.corpus.json")


def test_build_corpus_nonexistent_dir(tmp_path):
    with pytest.raises(FileNotFoundError):
        build_corpus(tmp_path / "nope", "test", tmp_path)

"""Tests for ground truth extraction."""

import pytest

from fourfour_analysis.groundtruth import normalize_key, extract_tags, extract_ground_truth


# ── normalize_key ────────────────────────────────────────

class TestNormalizeKey:
    def test_camelot_already(self):
        assert normalize_key("8A") == "8A"
        assert normalize_key("12B") == "12B"
        assert normalize_key("1A") == "1A"

    def test_note_names_major(self):
        assert normalize_key("C major") == "8B"
        assert normalize_key("G major") == "9B"
        assert normalize_key("A major") == "11B"

    def test_note_names_minor(self):
        assert normalize_key("A minor") == "11A"
        assert normalize_key("C minor") == "8A"
        assert normalize_key("E minor") == "12A"

    def test_shorthand_minor(self):
        assert normalize_key("Am") == "11A"
        assert normalize_key("Cm") == "8A"
        assert normalize_key("F#m") == "2A"

    def test_flats(self):
        assert normalize_key("Abm") == "4A"
        assert normalize_key("Bbm") == "6A"
        assert normalize_key("Eb major") == "5B"
        assert normalize_key("Db major") == "3B"

    def test_sharps(self):
        assert normalize_key("G#m") == "4A"
        assert normalize_key("F# major") == "2B"

    def test_empty(self):
        assert normalize_key("") is None
        assert normalize_key("   ") is None

    def test_unparseable(self):
        assert normalize_key("blah") is None

    def test_edge_12a(self):
        assert normalize_key("12A") == "12A"
        assert normalize_key("1B") == "1B"


# ── extract_tags / extract_ground_truth ──────────────────

class TestExtractTags:
    def test_wav_no_tags(self, tmp_path):
        """WAV files typically have no ID3 tags."""
        import numpy as np
        import soundfile as sf
        p = tmp_path / "test.wav"
        audio = np.zeros(44100, dtype=np.float32)
        sf.write(str(p), audio, 44100)

        tags = extract_tags(p)
        # WAV may or may not have tags — just verify it doesn't crash
        assert isinstance(tags, dict)

    def test_nonexistent_file(self, tmp_path):
        tags = extract_tags(tmp_path / "nonexistent.wav")
        assert isinstance(tags, dict)

    def test_ground_truth_none_for_untagged(self, tmp_path):
        import numpy as np
        import soundfile as sf
        p = tmp_path / "test.wav"
        audio = np.zeros(44100, dtype=np.float32)
        sf.write(str(p), audio, 44100)

        gt = extract_ground_truth(p, "t1")
        # No tags → no ground truth
        assert gt is None or gt.bpm is None

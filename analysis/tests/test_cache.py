"""Tests for content-addressed cache."""

import json
import tempfile
from pathlib import Path

from fourfour_analysis.cache import cache_key, load_cache, save_cache
from fourfour_analysis.types import AnalysisRecord, AnalysisResult


def test_cache_key_deterministic():
    key1 = cache_key("abc123", "lexicon_port", "cfg_v1")
    key2 = cache_key("abc123", "lexicon_port", "cfg_v1")
    assert key1 == key2
    assert len(key1) == 24


def test_cache_key_differs_on_inputs():
    key_a = cache_key("abc123", "lexicon_port", "cfg_v1")
    key_b = cache_key("abc123", "python_deeprhythm", "cfg_v1")
    assert key_a != key_b


def test_save_and_load():
    with tempfile.TemporaryDirectory() as tmp:
        cache_dir = Path(tmp)
        record = AnalysisRecord(
            track_id="t1",
            backend_id="lexicon_port",
            status="ok",
            result=AnalysisResult(bpm=128.0, key="8A", energy=7),
        )
        key = cache_key("fp1", "lexicon_port", "cfg1")
        save_cache(cache_dir, key, record)

        loaded = load_cache(cache_dir, key)
        assert loaded is not None
        assert loaded.track_id == "t1"
        assert loaded.backend_id == "lexicon_port"
        assert loaded.status == "ok"
        assert loaded.result is not None
        assert loaded.result.bpm == 128.0
        assert loaded.result.key == "8A"
        assert loaded.result.energy == 7


def test_cache_miss():
    with tempfile.TemporaryDirectory() as tmp:
        cache_dir = Path(tmp)
        assert load_cache(cache_dir, "nonexistent_key_123456") is None


def test_cache_roundtrip_with_beats():
    from fourfour_analysis.types import BeatPosition

    with tempfile.TemporaryDirectory() as tmp:
        cache_dir = Path(tmp)
        beats = [
            BeatPosition(time_seconds=0.0, bar_position=1),
            BeatPosition(time_seconds=0.5, bar_position=2),
            BeatPosition(time_seconds=1.0, bar_position=3),
        ]
        record = AnalysisRecord(
            track_id="t2",
            backend_id="lexicon_port",
            status="ok",
            result=AnalysisResult(bpm=120.0, beats=beats),
        )
        key = cache_key("fp2", "lexicon_port", "cfg1")
        save_cache(cache_dir, key, record)

        loaded = load_cache(cache_dir, key)
        assert loaded is not None
        assert loaded.result is not None
        assert len(loaded.result.beats) == 3
        assert loaded.result.beats[1].time_seconds == 0.5
        assert loaded.result.beats[1].bar_position == 2

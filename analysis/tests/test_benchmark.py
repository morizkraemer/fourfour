import pytest
from fourfour_analysis.benchmark import compare_bpm, compare_key, BenchmarkResult


def test_compare_bpm_exact_match():
    result = compare_bpm(128.0, 12800)  # master.db stores BPM * 100
    assert result.match is True
    assert result.difference == 0.0


def test_compare_bpm_within_tolerance():
    result = compare_bpm(127.5, 12800)  # 0.5 BPM off
    assert result.match is True  # within ±1 BPM tolerance
    assert abs(result.difference) <= 1.0


def test_compare_bpm_octave_error_detected():
    result = compare_bpm(64.0, 12800)  # half of 128
    assert result.match is False
    assert result.octave_error is True


def test_compare_bpm_detection_failed():
    result = compare_bpm(None, 12800)
    assert result.match is False


def test_compare_key_exact_match():
    result = compare_key("8A", "8A")
    assert result.match is True


def test_compare_key_relative_major_minor():
    """A minor (8A) vs C major (12B) — relative key, common confusion."""
    result = compare_key("8A", "12B")
    assert result.match is False
    assert result.relative_key is True


def test_compare_key_detection_failed():
    result = compare_key(None, "8A")
    assert result.match is False

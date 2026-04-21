"""Tests for comparison logic — BPM, key, energy diffs."""

import pytest

from fourfour_analysis.compare import compare_tempo, compare_key, compare_energy, compare_track
from fourfour_analysis.types import AnalysisResult, GroundTruth, TempoComparison, KeyComparison


# ── compare_tempo ────────────────────────────────────────

class TestCompareTempo:
    def test_exact_match(self):
        c = compare_tempo(128.0, 128.0)
        assert c.within_1pct
        assert c.within_4pct
        assert c.bpm_delta == 0.0
        assert not c.octave_error

    def test_within_1pct(self):
        c = compare_tempo(128.5, 128.0)
        assert c.within_1pct  # 0.5/128 = 0.39%

    def test_within_4pct_not_1pct(self):
        c = compare_tempo(131.0, 128.0)
        assert not c.within_1pct  # 3/128 = 2.3%
        assert c.within_4pct

    def test_outside_4pct(self):
        c = compare_tempo(140.0, 128.0)
        assert not c.within_1pct
        assert not c.within_4pct  # 12/128 = 9.4%

    def test_octave_error_double(self):
        c = compare_tempo(256.0, 128.0)
        assert c.octave_error
        assert not c.within_1pct

    def test_octave_error_half(self):
        c = compare_tempo(64.0, 128.0)
        assert c.octave_error

    def test_octave_error_1_5x(self):
        c = compare_tempo(192.0, 128.0)
        assert c.octave_error

    def test_no_octave_at_small_delta(self):
        c = compare_tempo(130.0, 128.0)
        assert not c.octave_error


# ── compare_key ──────────────────────────────────────────

class TestCompareKey:
    def test_exact_match(self):
        c = compare_key("8A", "8A")
        assert c.exact
        assert c.error_type == "exact"

    def test_relative(self):
        # 8A ↔ 8B (same position, different mode)
        c = compare_key("8A", "8B")
        assert not c.exact
        assert c.error_type == "relative"

    def test_fifth_up(self):
        # 8A → 9A (adjacent clockwise)
        c = compare_key("8A", "9A")
        assert c.error_type == "fifth"

    def test_fifth_down(self):
        # 8A → 7A (adjacent counter-clockwise)
        c = compare_key("8A", "7A")
        assert c.error_type == "fifth"

    def test_parallel(self):
        # 8A → 11A (±3 same letter)
        c = compare_key("8A", "11A")
        assert c.error_type == "parallel"

    def test_other(self):
        # 8A → 1A (far away)
        c = compare_key("8A", "1A")
        assert c.error_type == "other"

    def test_wrapping_fifth(self):
        # 12A → 1A (wraps around)
        c = compare_key("12A", "1A")
        assert c.error_type == "fifth"

    def test_cross_mode_fifth(self):
        # 8A → 9B (fifth + mode change)
        c = compare_key("8A", "9B")
        # dist = 1 (ring) + 1 (mode) = 2, so "other"
        assert c.error_type == "other"

    def test_unparseable(self):
        c = compare_key("8A", "garbage")
        assert c.error_type == "other"


# ── compare_energy ───────────────────────────────────────

class TestCompareEnergy:
    def test_exact(self):
        assert compare_energy(7, 7) == 0

    def test_delta(self):
        assert compare_energy(5, 8) == 3

    def test_negative(self):
        assert compare_energy(9, 3) == 6


# ── compare_track ────────────────────────────────────────

class TestCompareTrack:
    def test_full_comparison(self):
        result = AnalysisResult(bpm=128.0, key="8A", energy=7)
        gt = GroundTruth(track_id="t1", bpm=128.0, key="8A", energy=7, bpm_source="tag", key_source="tag")
        comp = compare_track(result, gt, "lexicon_port")

        assert comp.track_id == "t1"
        assert comp.backend_id == "lexicon_port"
        assert comp.tempo is not None
        assert comp.tempo.within_1pct
        assert comp.key is not None
        assert comp.key.exact
        assert comp.energy_delta == 0

    def test_partial_ground_truth(self):
        result = AnalysisResult(bpm=130.0, key="8A", energy=5)
        gt = GroundTruth(track_id="t2", bpm=128.0)  # no key/energy GT
        comp = compare_track(result, gt, "lexicon_port")

        assert comp.tempo is not None
        assert comp.key is None  # can't compare without GT
        assert comp.energy_delta is None

    def test_missing_analysis(self):
        result = AnalysisResult()  # all None
        gt = GroundTruth(track_id="t3", bpm=128.0, key="8A")
        comp = compare_track(result, gt, "lexicon_port")

        assert comp.tempo is None
        assert comp.key is None

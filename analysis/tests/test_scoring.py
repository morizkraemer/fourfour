"""Tests for scoring module."""

from fourfour_analysis.scoring import compute_scores, _decision_score, format_report
from fourfour_analysis.types import (
    AnalysisRecord, AnalysisResult, TrackComparison,
    TempoComparison, KeyComparison, BackendMetadata,
)


def _make_comps(backend_id: str, bpm_pct_within4: float, key_exact_pct: float) -> list[TrackComparison]:
    """Create synthetic comparisons for scoring tests."""
    n = 100
    n_bpm_ok = int(bpm_pct_within4 / 100 * n)
    n_key_ok = int(key_exact_pct / 100 * n)

    comps = []
    for i in range(n):
        tempo = TempoComparison(
            bpm_delta=0.5 if i < n_bpm_ok else 10.0,
            within_1pct=i < n_bpm_ok * 0.5,
            within_4pct=i < n_bpm_ok,
            octave_error=i >= n_bpm_ok,
        )
        key = KeyComparison(
            exact=(i < n_key_ok),
            error_type="exact" if i < n_key_ok else "other",
        )
        comps.append(TrackComparison(
            track_id=f"t{i}",
            backend_id=backend_id,
            tempo=tempo,
            key=key,
            energy_delta=1 if i % 3 == 0 else 0,
        ))
    return comps


def _make_records(backend_id: str, n: int, mean_time: float = 0.5) -> list[AnalysisRecord]:
    """Create synthetic records for scoring tests."""
    meta = BackendMetadata(id=backend_id, label="test", version="1", config_hash="v1")
    records = []
    for i in range(n):
        result = AnalysisResult(bpm=128.0, key="8A", energy=7, elapsed_seconds=mean_time, backend_metadata=meta)
        records.append(AnalysisRecord(
            track_id=f"t{i}",
            backend_id=backend_id,
            status="ok",
            result=result,
        ))
    return records


def test_compute_scores_basic():
    comps = {"lexicon_port": _make_comps("lexicon_port", 90, 70)}
    records = {"lexicon_port": _make_records("lexicon_port", 100)}
    scores = compute_scores(comps, records, ["lexicon_port"])

    assert "lexicon_port" in scores
    s = scores["lexicon_port"]
    assert s["bpm"]["acc2_pct"] == 90.0
    assert s["key"]["exact_match_pct"] == 70.0
    assert "decision_score" in s


def test_decision_score_weights():
    # Perfect BPM, perfect key → high score
    s1 = {"bpm": {"acc2_pct": 100}, "key": {"exact_match_pct": 100},
          "operational": {"mean_time_seconds": 0.1}, "variant_id": "lexicon_port"}
    d1 = _decision_score(s1)

    # Bad BPM, bad key → low score
    s2 = {"bpm": {"acc2_pct": 0}, "key": {"exact_match_pct": 0},
          "operational": {"mean_time_seconds": 5.0}, "variant_id": "lexicon_port"}
    d2 = _decision_score(s2)

    assert d1 > d2


def test_format_report():
    comps = {"lexicon_port": _make_comps("lexicon_port", 85, 65)}
    records = {"lexicon_port": _make_records("lexicon_port", 100)}
    scores = compute_scores(comps, records, ["lexicon_port"])
    report = format_report(scores, "run-test")
    assert "lexicon_port" in report
    assert "Recommendation" in report


def test_multiple_backends():
    comps = {
        "lexicon_port": _make_comps("lexicon_port", 80, 65),
        "python_deeprhythm": _make_comps("python_deeprhythm", 97, 70),
    }
    records = {
        "lexicon_port": _make_records("lexicon_port", 100, 0.3),
        "python_deeprhythm": _make_records("python_deeprhythm", 100, 0.8),
    }
    scores = compute_scores(comps, records, ["lexicon_port", "python_deeprhythm"])

    assert "_recommendation" in scores
    # python_deeprhythm has better BPM accuracy but dep penalty
    assert scores["python_deeprhythm"]["bpm"]["acc2_pct"] == 97.0

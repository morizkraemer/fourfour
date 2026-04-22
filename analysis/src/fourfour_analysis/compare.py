"""Comparison logic — diff analysis output vs ground truth.

Provides comparison functions for BPM, key, energy, and beats.
"""

from __future__ import annotations

from typing import Optional

from fourfour_analysis.types import (
    AnalysisResult,
    GroundTruth,
    TempoComparison,
    KeyComparison,
    TrackComparison,
)


# ── Camelot wheel adjacency (for error taxonomy) ──────────

def _camelot_distance(key1: str, key2: str) -> int | None:
    """Compute distance on the Camelot wheel.

    Returns None if keys can't be parsed.
    Adjacent = 1, opposite = 6.
    """
    n1, m1 = _parse_camelot(key1)
    n2, m2 = _parse_camelot(key2)
    if n1 is None or n2 is None:
        return None

    # Same position, different mode (relative major/minor)
    if n1 == n2 and m1 != m2:
        return 1  # "relative"

    # Distance on the wheel (same mode)
    ring_dist = min(abs(n1 - n2), 12 - abs(n1 - n2))

    # Cross-mode: add 0.5 (but we return int)
    # For simplicity: if same letter just ring_dist, else ring_dist + 1
    if m1 != m2:
        return ring_dist + 1
    return ring_dist


def _parse_camelot(key: str) -> tuple[Optional[int], Optional[str]]:
    """Parse Camelot notation like '8A' → (8, 'A')."""
    if not key or len(key) < 2:
        return None, None
    num_part = key[:-1]
    letter = key[-1].upper()
    if letter not in ("A", "B") or not num_part.isdigit():
        return None, None
    return int(num_part), letter


def compare_tempo(bpm_ours: float, bpm_gt: float) -> TempoComparison:
    """Compare detected BPM against ground truth.

    Args:
        bpm_ours: Our detected BPM.
        bpm_gt: Ground truth BPM.

    Returns:
        TempoComparison with delta, thresholds, and octave check.
    """
    delta = abs(bpm_ours - bpm_gt)
    pct = delta / bpm_gt * 100 if bpm_gt > 0 else float("inf")

    # Octave error: half or double
    octave_error = False
    for mult in (2.0, 0.5, 1.5, 0.67):
        if abs(bpm_ours - bpm_gt * mult) < 2.0:
            octave_error = True
            break

    return TempoComparison(
        bpm_delta=delta,
        within_1pct=pct <= 1.0,
        within_4pct=pct <= 4.0,
        octave_error=octave_error,
    )


def compare_key(key_ours: str, key_gt: str) -> KeyComparison:
    """Compare detected key against ground truth.

    Uses Camelot wheel taxonomy:
      - exact: same code
      - relative: same number, different letter (e.g. 8A ↔ 8B)
      - parallel: ±3 same letter (e.g. 8A ↔ 11A)
      - fifth: adjacent on wheel (e.g. 8A ↔ 7A or 8A ↔ 9A)
      - other: everything else
    """
    if key_ours == key_gt:
        return KeyComparison(exact=True, error_type="exact")

    dist = _camelot_distance(key_ours, key_gt)
    if dist is None:
        return KeyComparison(exact=False, error_type="other")

    n1, m1 = _parse_camelot(key_ours)
    n2, m2 = _parse_camelot(key_gt)

    # Relative: same number, different mode
    if n1 == n2 and m1 != m2:
        return KeyComparison(exact=False, error_type="relative")

    # Adjacent on wheel (fifth)
    if dist == 1:
        return KeyComparison(exact=False, error_type="fifth")

    # Parallel: ±3 same letter
    if m1 == m2:
        ring_dist = min(abs(n1 - n2), 12 - abs(n1 - n2))  # type: ignore
        if ring_dist == 3:
            return KeyComparison(exact=False, error_type="parallel")

    return KeyComparison(exact=False, error_type="other")


def compare_energy(energy_ours: int, energy_gt: int) -> int:
    """Compare energy ratings. Returns absolute delta."""
    return abs(energy_ours - energy_gt)


def compare_beats(
    beats_ours: list,
    beats_gt: list,
    tol_ms: float = 50.0,
) -> dict:
    """Compare beat grids using F-measure and median offset.

    Args:
        beats_ours: Our detected beats (list of BeatPosition or float seconds).
        beats_gt: Ground truth beats (list of BeatPosition or float seconds).

    Returns:
        Dict with f_measure (0-1), median_offset_ms, and counts.
    """
    # Extract time in seconds
    ours_sec = [b.time_seconds if hasattr(b, "time_seconds") else float(b) for b in beats_ours]
    gt_sec = [b.time_seconds if hasattr(b, "time_seconds") else float(b) for b in beats_gt]

    if not ours_sec or not gt_sec:
        return {"f_measure": 0.0, "median_offset_ms": None, "matched": 0, "total_gt": len(gt_sec), "total_ours": len(ours_sec)}

    tol_s = tol_ms / 1000.0
    import numpy as np
    ours = np.array(sorted(ours_sec))
    gts = np.array(sorted(gt_sec))

    # For each GT beat, find closest ours
    matched_gt = 0
    offsets = []
    for gt_t in gts:
        diffs = np.abs(ours - gt_t)
        min_idx = np.argmin(diffs)
        if diffs[min_idx] <= tol_s:
            matched_gt += 1
            offsets.append(float(ours[min_idx] - gt_t) * 1000)  # ms

    # Precision = matched / total_ours, Recall = matched / total_gt
    precision = matched_gt / len(ours) if len(ours) > 0 else 0.0
    recall = matched_gt / len(gts) if len(gts) > 0 else 0.0
    f_measure = 2 * precision * recall / (precision + recall) if (precision + recall) > 0 else 0.0

    median_offset = float(np.median(offsets)) if offsets else None

    return {
        "f_measure": round(f_measure, 4),
        "median_offset_ms": round(median_offset, 2) if median_offset is not None else None,
        "matched": matched_gt,
        "total_gt": len(gts),
        "total_ours": len(ours),
    }


def compare_track(
    result: AnalysisResult,
    gt: GroundTruth,
    backend_id: str,
) -> TrackComparison:
    """Compare a full analysis result against ground truth.

    Args:
        result: Analysis output.
        gt: Ground truth for this track.
        backend_id: Which backend produced the result.

    Returns:
        TrackComparison with per-dimension comparisons.
    """
    tempo = None
    if result.bpm is not None and gt.bpm is not None:
        tempo = compare_tempo(result.bpm, gt.bpm)

    key = None
    if result.key is not None and gt.key is not None:
        key = compare_key(result.key, gt.key)

    energy_delta = None
    if result.energy is not None and gt.energy is not None:
        energy_delta = compare_energy(result.energy, gt.energy)

    return TrackComparison(
        track_id=gt.track_id,
        backend_id=backend_id,
        tempo=tempo,
        key=key,
        energy_delta=energy_delta,
    )

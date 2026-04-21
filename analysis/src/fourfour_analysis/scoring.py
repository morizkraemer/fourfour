"""Scoring — aggregate comparison metrics and produce recommendations."""

from __future__ import annotations

from typing import Any

from fourfour_analysis.types import AnalysisRecord, TrackComparison


def compute_scores(
    all_comparisons: dict[str, list[TrackComparison]],
    all_results: dict[str, list[AnalysisRecord]],
    variant_ids: list[str],
) -> dict[str, Any]:
    """Compute aggregate scores for each backend.

    Args:
        all_comparisons: {variant_id: [TrackComparison, ...]}
        all_results: {variant_id: [AnalysisRecord, ...]}
        variant_ids: Ordered list of variant IDs.

    Returns:
        Dict suitable for JSON serialization with scoring.json schema.
    """
    scores = {}

    for vid in variant_ids:
        comps = all_comparisons.get(vid, [])
        records = all_results.get(vid, [])
        scores[vid] = _score_backend(vid, comps, records)

    # Decision score
    for vid, s in scores.items():
        s["decision_score"] = _decision_score(s)

    # Recommendation
    recommendation = _recommend(scores, variant_ids)
    scores["_recommendation"] = recommendation

    return scores


def _score_backend(
    variant_id: str,
    comparisons: list[TrackComparison],
    records: list[AnalysisRecord],
) -> dict[str, Any]:
    """Score a single backend."""
    n = len(comparisons)
    result: dict[str, Any] = {"variant_id": variant_id, "num_comparisons": n}

    # BPM scoring
    tempo_comps = [c for c in comparisons if c.tempo is not None]
    if tempo_comps:
        acc1 = sum(1 for c in tempo_comps if c.tempo.within_1pct) / len(tempo_comps) * 100
        acc2 = sum(1 for c in tempo_comps if c.tempo.within_4pct) / len(tempo_comps) * 100
        octave_errors = sum(1 for c in tempo_comps if c.tempo.octave_error) / len(tempo_comps) * 100
        mean_delta = sum(c.tempo.bpm_delta for c in tempo_comps) / len(tempo_comps)
        result["bpm"] = {
            "acc1_pct": round(acc1, 1),
            "acc2_pct": round(acc2, 1),
            "octave_error_pct": round(octave_errors, 1),
            "mean_delta": round(mean_delta, 2),
        }
    else:
        result["bpm"] = None

    # Key scoring
    key_comps = [c for c in comparisons if c.key is not None]
    if key_comps:
        exact = sum(1 for c in key_comps if c.key.exact) / len(key_comps) * 100
        adjacent = sum(1 for c in key_comps if c.key.error_type in ("exact", "relative", "fifth")) / len(key_comps) * 100
        error_dist = {}
        for c in key_comps:
            error_dist[c.key.error_type] = error_dist.get(c.key.error_type, 0) + 1
        result["key"] = {
            "exact_match_pct": round(exact, 1),
            "adjacent_match_pct": round(adjacent, 1),
            "error_distribution": error_dist,
        }
    else:
        result["key"] = None

    # Energy scoring
    energy_comps = [c for c in comparisons if c.energy_delta is not None]
    if energy_comps:
        mean_delta = sum(c.energy_delta for c in energy_comps) / len(energy_comps)
        within_2 = sum(1 for c in energy_comps if c.energy_delta <= 2) / len(energy_comps) * 100
        result["energy"] = {
            "mean_delta": round(mean_delta, 2),
            "within_2_pct": round(within_2, 1),
        }
    else:
        result["energy"] = None

    # Operational metrics
    ok_records = [r for r in records if r.status == "ok"]
    fail_records = [r for r in records if r.status == "failed"]
    times = [r.result.elapsed_seconds for r in ok_records if r.result is not None]

    result["operational"] = {
        "total_tracks": len(records),
        "ok": len(ok_records),
        "failed": len(fail_records),
        "failure_rate": round(len(fail_records) / max(len(records), 1) * 100, 1),
        "mean_time_seconds": round(sum(times) / len(times), 3) if times else None,
        "p95_time_seconds": round(sorted(times)[int(len(times) * 0.95)], 3) if len(times) >= 20 else None,
    }

    return result


def _decision_score(scores: dict) -> float:
    """Compute weighted decision score for backend selection.

    Formula:
      score = 0.40 × bpm_acc2 + 0.35 × key_exact + 0.15 × speed_factor + 0.10 × dep_factor
    """
    bpm_acc2 = (scores.get("bpm") or {}).get("acc2_pct", 0) or 0
    key_exact = (scores.get("key") or {}).get("exact_match_pct", 0) or 0

    # Speed factor: normalize time to 0-1 (5s+ = 0, <0.5s = 1)
    mean_time = scores.get("operational", {}).get("mean_time_seconds") or 5.0
    speed_factor = max(0, 1 - min(mean_time / 5.0, 1.0)) * 100

    # Dep factor: no heavy deps = 100, torch = 20
    dep_factor = 100  # default for lexicon_port
    variant_id = scores.get("variant_id", "")
    if "deeprhythm" in variant_id or "python" in variant_id:
        dep_factor = 20
    elif "stratum" in variant_id:
        dep_factor = 80

    score = 0.40 * bpm_acc2 + 0.35 * key_exact + 0.15 * speed_factor + 0.10 * dep_factor
    return round(score, 2)


def _recommend(scores: dict, variant_ids: list[str]) -> str:
    """Generate recommendation string based on scores."""
    # Remove meta keys
    backend_scores = {vid: scores[vid]["decision_score"] for vid in variant_ids if vid in scores}

    if not backend_scores:
        return "no data"

    best = max(backend_scores, key=backend_scores.get)  # type: ignore

    # Check if hybrid would be better
    # Simple heuristic: if BPM winner != key winner, suggest hybrid
    bpm_scores = {vid: (scores.get(vid, {}).get("bpm", {}) or {}).get("acc2_pct", 0) for vid in variant_ids}
    key_scores = {vid: (scores.get(vid, {}).get("key", {}) or {}).get("exact_match_pct", 0) for vid in variant_ids}

    if bpm_scores and key_scores:
        bpm_winner = max(bpm_scores, key=bpm_scores.get)  # type: ignore
        key_winner = max(key_scores, key=key_scores.get)  # type: ignore

        if bpm_winner != key_winner:
            return f"hybrid: {bpm_winner} BPM + {key_winner} key/energy/cues (decision: {best})"
        else:
            return f"single: {best} (score {backend_scores[best]:.1f})"
    else:
        return f"single: {best} (score {backend_scores[best]:.1f})"


def format_report(scores: dict, run_id: str) -> str:
    """Format scores into a printable report."""
    lines = [f"\n{'='*60}", f"Benchmark Results: {run_id}", f"{'='*60}"]

    for vid in [k for k in scores if not k.startswith("_")]:
        s = scores[vid]
        lines.append(f"\n  {vid} (decision score: {s.get('decision_score', 'N/A')})")

        if s.get("bpm"):
            lines.append(f"    BPM:  Acc1={s['bpm']['acc1_pct']:.1f}%  "
                        f"Acc2={s['bpm']['acc2_pct']:.1f}%  "
                        f"Δ={s['bpm']['mean_delta']:.1f}")

        if s.get("key"):
            lines.append(f"    Key:  Exact={s['key']['exact_match_pct']:.1f}%  "
                        f"Adjacent={s['key']['adjacent_match_pct']:.1f}%")

        if s.get("energy"):
            lines.append(f"    Energy:  Δ={s['energy']['mean_delta']:.2f}  "
                        f"within 2={s['energy']['within_2_pct']:.1f}%")

        ops = s.get("operational", {})
        if ops:
            lines.append(f"    Ops:  {ops.get('ok', 0)} ok / {ops.get('failed', 0)} failed  "
                        f"({ops.get('mean_time_seconds', '?')}s/track)")

    rec = scores.get("_recommendation", "no data")
    lines.append(f"\n  Recommendation: {rec}")
    lines.append(f"{'='*60}")

    return "\n".join(lines)

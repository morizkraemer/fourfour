"""Benchmark runner — orchestrate analysis + comparison + scoring."""

from __future__ import annotations

import json
import time
from concurrent.futures import ProcessPoolExecutor, as_completed
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

from fourfour_analysis.config import Settings
from fourfour_analysis.manifest import load_corpus
from fourfour_analysis.backends.base import AnalysisBackend
from fourfour_analysis.backends.registry import load_backend
from fourfour_analysis.types import AnalysisRecord, TrackEntry, TrackComparison
from fourfour_analysis.compare import compare_track


def run_benchmark(
    corpus_path: str | Path,
    variant_ids: list[str],
    settings: Settings,
    parallel: int = 1,
    speed_only: bool = False,
    features: set[str] | None = None,
) -> str:
    """Run benchmark: analyze all tracks with each backend, compare, score.

    Args:
        corpus_path: Path to corpus JSON.
        variant_ids: Backend variant IDs to test.
        settings: Project settings.
        parallel: Number of parallel workers.
        speed_only: Skip comparison, only measure timing.

    Returns:
        Run ID string.
    """
    corpus_path = Path(corpus_path)
    run_id = f"run-{datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')}"
    run_dir = settings.results_dir / run_id
    run_dir.mkdir(parents=True, exist_ok=True)
    raw_dir = run_dir / "raw"
    raw_dir.mkdir(exist_ok=True)

    # Load corpus
    entries = load_corpus(corpus_path)
    print(f"Loaded {len(entries)} tracks from corpus")

    # Filter to entries with ground truth (unless speed_only)
    scorable = [e for e in entries if e.ground_truth is not None]
    print(f"  {len(scorable)} have ground truth for scoring")
    print(f"  {len(entries) - len(scorable)} will be analyzed but not scored")

    # Run each backend
    all_results: dict[str, list[AnalysisRecord]] = {}
    all_comparisons: dict[str, list[TrackComparison]] = {}

    for variant_id in variant_ids:
        print(f"\nRunning backend: {variant_id}")
        backend = load_backend(variant_id, settings, features=features)

        records = _analyze_tracks(backend, entries, parallel)
        all_results[variant_id] = records

        # Write raw results
        raw_path = raw_dir / f"{variant_id}.json"
        raw_path.write_text(json.dumps(
            [_record_to_dict(r) for r in records],
            indent=2, default=str,
        ))

        # Compare against ground truth
        if not speed_only:
            comparisons = _compare_results(records, entries)
            all_comparisons[variant_id] = comparisons
        else:
            print(f"  Speed-only mode — skipping comparison")

    # Write comparisons
    if not speed_only:
        comp_path = run_dir / "comparisons.json"
        comp_data = {}
        for vid, comps in all_comparisons.items():
            comp_data[vid] = [_comp_to_dict(c) for c in comps]
        comp_path.write_text(json.dumps(comp_data, indent=2, default=str))

    # Score and write scoring.json
    from fourfour_analysis.scoring import compute_scores, format_report
    scores = compute_scores(all_comparisons, all_results, variant_ids)
    scoring_path = run_dir / "scoring.json"
    scoring_path.write_text(json.dumps(scores, indent=2, default=str))

    # Print summary
    report = format_report(scores, run_id)
    print(report)

    # Write run metadata
    meta = {
        "run_id": run_id,
        "corpus": str(corpus_path),
        "variants": variant_ids,
        "num_tracks": len(entries),
        "num_scorable": len(scorable),
        "speed_only": speed_only,
        "features": sorted(features) if features is not None else None,
        "timestamp": datetime.now(timezone.utc).isoformat(),
    }
    (run_dir / "meta.json").write_text(json.dumps(meta, indent=2))

    return run_id


def _analyze_tracks(
    backend: AnalysisBackend,
    entries: list[TrackEntry],
    parallel: int,
) -> list[AnalysisRecord]:
    """Analyze all tracks with a backend, report progress."""
    records = []
    ok_count = 0
    fail_count = 0
    start = time.monotonic()

    for i, entry in enumerate(entries):
        record = backend.analyze_track_cached(entry)
        records.append(record)

        if record.status == "ok":
            ok_count += 1
        else:
            fail_count += 1

        if (i + 1) % 10 == 0 or (i + 1) == len(entries):
            elapsed = time.monotonic() - start
            rate = (i + 1) / elapsed if elapsed > 0 else 0
            print(f"  [{i+1}/{len(entries)}] {ok_count} ok, {fail_count} failed "
                  f"({rate:.1f} tracks/s)")

    elapsed = time.monotonic() - start
    print(f"  Done: {ok_count} ok, {fail_count} failed in {elapsed:.1f}s")
    return records


def _compare_results(
    records: list[AnalysisRecord],
    entries: list[TrackEntry],
) -> list[TrackComparison]:
    """Compare analysis records against ground truth."""
    comparisons = []

    # Build entry lookup
    entry_map = {e.id: e for e in entries}

    for record in records:
        if record.status != "ok" or record.result is None:
            continue

        entry = entry_map.get(record.track_id)
        if entry is None or entry.ground_truth is None:
            continue

        comp = compare_track(record.result, entry.ground_truth, record.backend_id)
        comparisons.append(comp)

    return comparisons


def _record_to_dict(record: AnalysisRecord) -> dict:
    from dataclasses import asdict
    d = asdict(record)
    return d


def _comp_to_dict(comp: TrackComparison) -> dict:
    from dataclasses import asdict
    return asdict(comp)

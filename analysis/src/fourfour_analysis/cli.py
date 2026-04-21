"""CLI entry points: fourfour-analyze and fourfour-benchmark."""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

from fourfour_analysis import __version__


def _build_analyze_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="fourfour-analyze",
        description="Analyze audio files for BPM, key, energy, waveforms, and cue points.",
    )
    parser.add_argument("file", help="Path to audio file")
    parser.add_argument(
        "--backend",
        action="append",
        dest="backends",
        choices=["lexicon_port", "python_deeprhythm", "stratum_dsp"],
        help="Backend(s) to use. Defaults to lexicon_port.",
    )
    parser.add_argument("--json", action="store_true", dest="json_output", help="Output as JSON")
    parser.add_argument("--version", action="version", version=f"%(prog)s {__version__}")
    return parser


def _build_benchmark_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="fourfour-benchmark",
        description="Benchmark analysis backends against ground truth.",
    )
    sub = parser.add_subparsers(dest="command")

    # init
    init_p = sub.add_parser("init", help="Build corpus from tagged audio files")
    init_p.add_argument("directory", help="Directory of tagged audio files")
    init_p.add_argument("--name", required=True, help="Corpus name")

    # run
    run_p = sub.add_parser("run", help="Run benchmark")
    run_p.add_argument("--corpus", required=True, help="Path to corpus JSON")
    run_p.add_argument(
        "--variants",
        nargs="+",
        choices=["lexicon_port", "python_deeprhythm", "stratum_dsp"],
        default=["lexicon_port"],
        help="Backends to benchmark",
    )
    run_p.add_argument("--parallel", type=int, default=1, help="Parallel workers")

    # show
    show_p = sub.add_parser("show", help="Show benchmark results")
    show_p.add_argument("run_id", help="Run ID to display")

    # compare
    cmp_p = sub.add_parser("compare", help="Compare two runs")
    cmp_p.add_argument("run1", help="First run ID")
    cmp_p.add_argument("run2", help="Second run ID")

    # list
    sub.add_parser("list", help="List all benchmark runs")

    # config-dirs
    sub.add_parser("config-dirs", help="Show resolved paths")

    parser.add_argument("--version", action="version", version=f"%(prog)s {__version__}")
    return parser


def _analyze_with_backend(backend_id: str, file_path: Path) -> dict:
    """Run analysis with a single backend, return result dict."""
    from fourfour_analysis.config import Settings
    from fourfour_analysis.backends.registry import load_backend

    settings = Settings.from_cwd()
    backend = load_backend(backend_id, settings)

    start = time.monotonic()
    try:
        result = backend.analyze_track(str(file_path))
        elapsed = time.monotonic() - start

        from dataclasses import asdict
        result_dict = asdict(result)
        result_dict["elapsed_seconds"] = elapsed
        result_dict["status"] = "ok"
        return result_dict
    except Exception as e:
        elapsed = time.monotonic() - start
        return {"status": "error", "error": str(e), "elapsed_seconds": elapsed, "backend": backend_id}


def analyze_main() -> None:
    """Entry point for fourfour-analyze."""
    parser = _build_analyze_parser()
    args = parser.parse_args()

    file_path = Path(args.file)
    if not file_path.is_file():
        print(f"Error: file not found: {args.file}", file=sys.stderr)
        sys.exit(1)

    backends = args.backends or ["lexicon_port"]

    results = {}
    for backend_id in backends:
        results[backend_id] = _analyze_with_backend(backend_id, file_path)

    if args.json_output:
        print(json.dumps(results, indent=2, default=str))
    else:
        for backend_id, result in results.items():
            print(f"\n{'='*50}")
            print(f"Backend: {backend_id}")
            print(f"{'='*50}")
            if result.get("status") == "error":
                print(f"  Error: {result['error']}")
                continue
            print(f"  BPM:    {result.get('bpm', 'N/A')}")
            print(f"  Key:    {result.get('key', 'N/A')}")
            print(f"  Energy: {result.get('energy', 'N/A')}")
            print(f"  Beats:  {len(result.get('beats', []))}")
            print(f"  Cues:   {len(result.get('cue_points', []))}")
            print(f"  Time:   {result.get('elapsed_seconds', 0):.2f}s")


def benchmark_main() -> None:
    """Entry point for fourfour-benchmark."""
    parser = _build_benchmark_parser()
    args = parser.parse_args()

    if args.command is None:
        parser.print_help()
        sys.exit(1)

    if args.command == "config-dirs":
        from fourfour_analysis.config import Settings

        settings = Settings.from_cwd()
        print(f"Root:        {settings.root_dir}")
        print(f"Benchmark:   {settings.benchmark_dir}")
        print(f"Manifests:   {settings.manifests_dir}")
        print(f"Results:     {settings.results_dir}")
        print(f"Cache:       {settings.cache_dir}")
        return

    if args.command == "init":
        from fourfour_analysis.config import Settings
        from fourfour_analysis.manifest import build_corpus

        settings = Settings.from_cwd()
        corpus_path = build_corpus(
            args.directory, args.name, settings.manifests_dir
        )
        print(f"Corpus written: {corpus_path}")

        # Summary
        from fourfour_analysis.manifest import load_corpus
        entries = load_corpus(corpus_path)
        scorable = sum(1 for e in entries if e.ground_truth is not None)
        print(f"  {len(entries)} tracks ({scorable} with ground truth)")
        return

    if args.command == "run":
        from fourfour_analysis.config import Settings
        from fourfour_analysis.runner import run_benchmark

        settings = Settings.from_cwd()
        run_id = run_benchmark(
            corpus_path=args.corpus,
            variant_ids=args.variants,
            settings=settings,
            parallel=args.parallel,
        )
        print(f"\nRun ID: {run_id}")
        return

    if args.command == "show":
        from fourfour_analysis.config import Settings
        settings = Settings.from_cwd()
        scoring_path = settings.results_dir / args.run_id / "scoring.json"
        if not scoring_path.is_file():
            print(f"No results for run: {args.run_id}", file=sys.stderr)
            sys.exit(1)
        import json as _json
        scores = _json.loads(scoring_path.read_text())
        from fourfour_analysis.scoring import format_report
        print(format_report(scores, args.run_id))
        return

    if args.command == "list":
        from fourfour_analysis.config import Settings
        settings = Settings.from_cwd()
        results_dir = settings.results_dir
        if not results_dir.is_dir():
            print("No benchmark runs yet.")
            return
        runs = sorted([d.name for d in results_dir.iterdir() if d.is_dir()])
        if not runs:
            print("No benchmark runs yet.")
            return
        for run in runs:
            print(f"  {run}")
        return

    if args.command == "compare":
        from fourfour_analysis.config import Settings
        settings = Settings.from_cwd()
        import json as _json
        s1_path = settings.results_dir / args.run1 / "scoring.json"
        s2_path = settings.results_dir / args.run2 / "scoring.json"
        if not s1_path.is_file() or not s2_path.is_file():
            print("One or both runs not found.", file=sys.stderr)
            sys.exit(1)
        s1 = _json.loads(s1_path.read_text())
        s2 = _json.loads(s2_path.read_text())
        # Simple diff: show backends present in both
        all_keys = set(s1.keys()) | set(s2.keys())
        all_keys.discard("_recommendation")
        for k in sorted(all_keys):
            d1 = s1.get(k, {}).get("decision_score", "N/A")
            d2 = s2.get(k, {}).get("decision_score", "N/A")
            delta = ""
            if isinstance(d1, (int, float)) and isinstance(d2, (int, float)):
                delta = f"  ({d2 - d1:+.1f})"
            print(f"  {k}: {d1} → {d2}{delta}")
        return


def main() -> None:
    """Fallback entry point for python -m fourfour_analysis."""
    print(f"fourfour-analysis v{__version__}")
    print("Use: fourfour-analyze <file>  or  fourfour-benchmark <command>")

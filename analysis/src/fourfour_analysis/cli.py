"""CLI entry points: fourfour-analyze and fourfour-benchmark.

Provides two commands for LLM and human use:
  fourfour-analyze <file>         — analyze a single audio file
  fourfour-benchmark <command>    — build corpora and run benchmarks

Use --help on any command for details.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from fourfour_analysis import __version__
from fourfour_analysis.backends.registry import ANALYSIS_VARIANTS

_HELP_EPILOG_ANALYZE = """
examples:
  fourfour-analyze track.mp3                              analyze with the final production stack
  fourfour-analyze track.mp3 --json                       output as JSON (for piping)

output fields (JSON mode):
  path                     Input audio file path
  bpm                      Detected tempo in BPM (float, e.g. 128.0)
  key                      Musical key in Camelot notation (string, e.g. "8A")
  energy                   Energy dict: {score: 1-10, label: low|medium|high}
  beats                    Currently empty; beatgrid/first-beat analyzer is separate
  cue_points               Currently empty until beatgrid/phrase analysis lands
  waveform_preview         400-byte Pioneer PWAV preview as integers
  waveform_color           2000 RGB amplitude/color points
  waveform_peaks           2000 min/max peak pairs
  pioneer_3band_detail     Native-resolution 3-band detail waveform bytes
  pioneer_3band_overview   400-point 3-band overview waveform bytes
  errors                   Non-fatal extractor errors
  elapsed_seconds          Wall time for analysis

backends:
  final_stack        Production stack: DeepRhythm BPM + librosa energy + Essentia bgate key
"""

_HELP_EPILOG_BENCHMARK = """
workflow:
  1. fourfour-benchmark init ~/Music/corpus --name my-corpus
     Scan a directory of tagged audio files. BPM/key tags become ground truth.
     Writes benchmark/manifests/<name>.corpus.json

  2. fourfour-benchmark run --corpus benchmark/manifests/my-corpus.corpus.json --variants lexicon_port
     Analyze all tracks with selected backends, compare against ground truth,
     compute accuracy metrics and a recommendation.
     Writes benchmark/results/<run_id>/ with raw/, comparisons.json, scoring.json

  3. fourfour-benchmark show <run_id>
     Print the scoring report for a run.

  4. fourfour-benchmark compare <run1> <run2>
     Diff decision scores between two runs.

ground truth:
  BPM and key are extracted from audio file tags (ID3 TBPM/TKEY, Vorbis BPM/INITIALKEY).
  Tracks without BPM/key tags are analyzed but not scored.
  Key values are normalized to Camelot notation (1A-12A, 1B-12B).

scoring formula:
  decision_score = 0.40 * bpm_acc2 + 0.35 * key_exact + 0.15 * speed + 0.10 * deps
  Where bpm_acc2 = % within 4% of ground truth, key_exact = % exact match.

key benchmark:
  fourfour-benchmark run --corpus benchmark/manifests/beatport-edm-key-keyonly-clean-full.corpus.json --variants essentia_key_bgate --features key
"""

_BENCHMARK_FEATURES = {"bpm", "key", "energy", "waveform", "cues"}
_BACKEND_CHOICES = sorted(ANALYSIS_VARIANTS)


def _build_analyze_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="fourfour-analyze",
        description="Analyze one audio file with the complete production stack.",
        epilog=_HELP_EPILOG_ANALYZE,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("file", help="Path to audio file (WAV, FLAC, MP3, AAC, etc.)")
    parser.add_argument("--json", action="store_true", dest="json_output", help="Output as JSON (machine-readable)")
    parser.add_argument("--version", action="version", version=f"%(prog)s {__version__}")
    return parser


def _build_benchmark_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="fourfour-benchmark",
        description="Benchmark analysis backends against ground truth from audio file tags.",
        epilog=_HELP_EPILOG_BENCHMARK,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    sub = parser.add_subparsers(dest="command")

    # init
    init_p = sub.add_parser(
        "init",
        help="Build a corpus JSON from tagged audio files",
        description="Scan a directory recursively for audio files and extract BPM/key tags as ground truth. "
                    "Produces a .corpus.json file used by 'run'.",
    )
    init_p.add_argument("directory", help="Directory of tagged audio files (scanned recursively)")
    init_p.add_argument("--name", required=True, help="Corpus name (used as filename: <name>.corpus.json)")

    # run
    run_p = sub.add_parser(
        "run",
        help="Run benchmark: analyze + compare + score",
        description="Analyze all tracks in a corpus with selected backends, compare against ground truth, "
                    "and produce scoring.json with accuracy metrics and a recommendation.",
    )
    run_p.add_argument("--corpus", required=True, help="Path to .corpus.json file")
    run_p.add_argument(
        "--variants",
        nargs="+",
        choices=_BACKEND_CHOICES,
        default=["lexicon_port"],
        help="Backend variant(s) to benchmark (default: lexicon_port)",
    )
    run_p.add_argument("--parallel", type=int, default=1, help="Number of parallel workers (default: 1)")
    run_p.add_argument(
        "--features",
        default="all",
        help="Comma-separated analysis features: bpm,key,energy,waveform,cues or 'all' (default).",
    )
    run_p.add_argument(
        "--no-waveform",
        action="store_true",
        help="Remove waveform generation from the selected feature set.",
    )
    run_p.add_argument("--speed-only", action="store_true", dest="speed_only",
                       help="Skip comparison. Only measure timing (operational metrics).")

    # show
    show_p = sub.add_parser(
        "show",
        help="Display results from a benchmark run",
        description="Print the scoring report for a completed benchmark run.",
    )
    show_p.add_argument("run_id", help="Run ID to display (e.g. run-20260421T120000Z)")

    # compare
    cmp_p = sub.add_parser(
        "compare",
        help="Compare decision scores between two runs",
        description="Show side-by-side comparison of decision scores for two benchmark runs.",
    )
    cmp_p.add_argument("run1", help="First run ID (baseline)")
    cmp_p.add_argument("run2", help="Second run ID (new)")

    # list
    sub.add_parser("list", help="List all benchmark run IDs")

    # config-dirs
    sub.add_parser("config-dirs", help="Show resolved project paths")

    parser.add_argument("--version", action="version", version=f"%(prog)s {__version__}")
    return parser


def _parse_benchmark_features(raw: str, no_waveform: bool) -> set[str] | None:
    """Parse benchmark feature flags.

    None means each backend should run its default full feature set.
    """
    if raw.strip().lower() == "all":
        features = None
    else:
        features = {part.strip().lower() for part in raw.split(",") if part.strip()}
        unknown = features - _BENCHMARK_FEATURES
        if unknown:
            valid = ", ".join(sorted(_BENCHMARK_FEATURES))
            raise ValueError(f"unknown feature(s): {', '.join(sorted(unknown))}; valid: {valid}")
        if not features:
            raise ValueError("--features must not be empty")

    if no_waveform:
        features = set(_BENCHMARK_FEATURES if features is None else features)
        features.discard("waveform")

    return features


def analyze_main() -> None:
    """Entry point for fourfour-analyze."""
    parser = _build_analyze_parser()
    args = parser.parse_args()

    file_path = Path(args.file)
    if not file_path.is_file():
        print(f"Error: file not found: {args.file}", file=sys.stderr)
        sys.exit(1)

    from fourfour_analysis.analyze import analyze_track

    result = analyze_track(str(file_path))

    if args.json_output:
        print(json.dumps(result, indent=2))
    else:
        print(f"\n{'='*50}")
        print("fourfour analysis")
        print(f"{'='*50}")
        print(f"  BPM:    {result.get('bpm', 'N/A')}")
        print(f"  Key:    {result.get('key', 'N/A')}")
        print(f"  Energy: {result.get('energy', 'N/A')}")
        print(f"  Beats:  {len(result.get('beats', []))}")
        print(f"  Cues:   {len(result.get('cue_points', []))}")
        print(f"  Preview bytes:  {len(result.get('waveform_preview', []))}")
        print(f"  Color points:   {len(result.get('waveform_color', []))}")
        print(f"  Peak points:    {len(result.get('waveform_peaks', []))}")
        print(f"  3-band detail:  {len(result.get('pioneer_3band_detail', []))}")
        print(f"  3-band overview: {len(result.get('pioneer_3band_overview', []))}")
        print(f"  Errors: {len(result.get('errors', []))}")
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

        from fourfour_analysis.manifest import load_corpus
        entries = load_corpus(corpus_path)
        scorable = sum(1 for e in entries if e.ground_truth is not None)
        print(f"  {len(entries)} tracks ({scorable} with ground truth)")
        return

    if args.command == "run":
        from fourfour_analysis.config import Settings
        from fourfour_analysis.runner import run_benchmark

        settings = Settings.from_cwd()
        try:
            features = _parse_benchmark_features(args.features, args.no_waveform)
        except ValueError as e:
            print(f"Error: {e}", file=sys.stderr)
            sys.exit(2)

        run_id = run_benchmark(
            corpus_path=args.corpus,
            variant_ids=args.variants,
            settings=settings,
            parallel=args.parallel,
            speed_only=getattr(args, "speed_only", False),
            features=features,
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
        scores = json.loads(scoring_path.read_text())
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
        s1_path = settings.results_dir / args.run1 / "scoring.json"
        s2_path = settings.results_dir / args.run2 / "scoring.json"
        if not s1_path.is_file() or not s2_path.is_file():
            print("One or both runs not found.", file=sys.stderr)
            sys.exit(1)
        s1 = json.loads(s1_path.read_text())
        s2 = json.loads(s2_path.read_text())
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


def _module_analyze_main(argv: list[str]) -> None:
    """Compatibility entry point for `python -m fourfour_analysis analyze`.

    The Tauri app calls this command shape and expects a JSON list of per-track
    dicts containing Pioneer waveform fields.
    """
    parser = argparse.ArgumentParser(
        prog="python -m fourfour_analysis analyze",
        description="Analyze audio files with the final stack and Pioneer waveform outputs.",
    )
    parser.add_argument("paths", nargs="*", help="Audio file path(s)")
    parser.add_argument("--dir", dest="directory", help="Analyze all audio files in a directory")
    parser.add_argument("--json", action="store_true", dest="json_output", help="Output JSON to stdout")
    parser.add_argument("--output", "-o", help="Write JSON output to file")
    parser.add_argument("--workers", "-w", type=int, default=4, help="Number of parallel workers")
    args = parser.parse_args(argv)

    file_list = list(args.paths)
    if args.directory:
        dir_path = Path(args.directory)
        extensions = {".mp3", ".wav", ".flac", ".aiff", ".aif", ".m4a", ".ogg"}
        file_list.extend(str(p) for p in dir_path.rglob("*") if p.suffix.lower() in extensions)

    if not file_list:
        print("No audio files specified.", file=sys.stderr)
        sys.exit(1)

    from fourfour_analysis.analyze import analyze_batch, analyze_track

    results = [analyze_track(file_list[0])] if len(file_list) == 1 else analyze_batch(file_list, workers=args.workers)
    json_str = json.dumps(results, indent=2)

    if args.output:
        Path(args.output).write_text(json_str)
        print(f"Results written to {args.output}", file=sys.stderr)
    elif args.json_output:
        print(json_str)
    else:
        for result in results:
            name = Path(result["path"]).name
            bpm = result.get("bpm", "?")
            key = result.get("key", "?")
            energy = result.get("energy") or {}
            e_score = energy.get("score", "?")
            errors = len(result.get("errors", []))
            print(f"{name}: BPM={bpm} Key={key} Energy={e_score}/10 Errors={errors}")


def main() -> None:
    """Fallback entry point for python -m fourfour_analysis."""
    if len(sys.argv) > 1 and sys.argv[1] == "analyze":
        _module_analyze_main(sys.argv[2:])
        return

    print(f"fourfour-analysis v{__version__}")
    print("Use: fourfour-analyze <file>, fourfour-benchmark <command>, or python -m fourfour_analysis analyze <file>")

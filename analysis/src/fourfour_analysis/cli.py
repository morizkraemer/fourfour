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
import time
from pathlib import Path

from fourfour_analysis import __version__
from fourfour_analysis.backends.registry import ANALYSIS_VARIANTS

_HELP_EPILOG_ANALYZE = """
examples:
  fourfour-analyze track.mp3                              analyze with default backend (deeprhythm_essentia)
  fourfour-analyze track.mp3 --json                       output as JSON (for piping)
  fourfour-analyze track.mp3 --backend deeprhythm_essentia  use DeepRhythm BPM + Essentia key
  fourfour-analyze track.mp3 --backend lexicon_port       use Lexicon algorithms only

output fields (JSON mode):
  bpm           Detected tempo in BPM (float, e.g. 128.0)
  key           Musical key in Camelot notation (string, e.g. "8A", "3B")
  energy        Energy rating 1-10 (int)
  beats         List of beat positions with bar_position (1-4)
  waveform_peaks  List of {min_val, max_val} per segment
  waveform_colors  List of {r, g, b} per segment (0-255)
  cue_points    List of {label, time_seconds, loop_end_seconds?}
  elapsed_seconds  Wall time for analysis

backends:
  lexicon_port       Lexicon algorithms ported to Python (numpy+scipy, no ML)
  python_deeprhythm  DeepRhythm (torch) for BPM + librosa for key (needs [ml] extras)
  stratum_dsp        Rust subprocess wrapping stratum-dsp (needs stratum-cli binary)
  essentia_key_bgate Essentia KeyExtractor bgate profile (key only, needs [key] extra)
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
        description="Analyze a single audio file for BPM, key, energy, waveforms, and cue points.",
        epilog=_HELP_EPILOG_ANALYZE,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("file", help="Path to audio file (WAV, FLAC, MP3, AAC, etc.)")
    parser.add_argument(
        "-b", "--backend",
        action="append",
        dest="backends",
        choices=_BACKEND_CHOICES,
        help="Backend(s) to use. May be specified multiple times. Default: deeprhythm_essentia.",
    )
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

    backends = args.backends or ["deeprhythm_essentia"]

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


def _waveform_cols_to_list(columns: list) -> list[dict]:
    """Convert WaveformColumn list to serialisable dicts."""
    return [
        {"min_val": c.min_val, "max_val": c.max_val, "r": c.r, "g": c.g, "b": c.b}
        for c in columns
    ]


def waveform_compare_main() -> None:
    """Compare waveform generation backends: Lexicon vs Librosa vs Essentia."""
    parser = argparse.ArgumentParser(
        prog="fourfour-waveform-compare",
        description="Generate waveforms from Lexicon, Librosa, and Essentia backends for comparison.",
    )
    parser.add_argument("file", help="Path to audio file")
    parser.add_argument("--json", action="store_true", dest="json_output", help="Output as JSON")
    args = parser.parse_args()

    file_path = Path(args.file)
    if not file_path.is_file():
        print(f"Error: file not found: {args.file}", file=sys.stderr)
        sys.exit(1)

    # Full analysis for BPM / key / beats + lexicon waveform
    main_result = _analyze_with_backend("deeprhythm_essentia", file_path)

    # Load + preprocess audio once for additional backends
    from fourfour_analysis.audio_io import load_audio, preprocess_waveform
    audio, sr = load_audio(str(file_path))
    audio_12k, sr_12k = preprocess_waveform(audio, sr)

    from fourfour_analysis.backends.lexicon_waveform import generate_waveform as _lexicon

    # Lexicon waveform columns (re-use already-preprocessed audio)
    lexicon_cols = _lexicon(audio_12k, sr_12k)
    waveforms: dict[str, list[dict] | None] = {
        "Lexicon": _waveform_cols_to_list(lexicon_cols),
    }

    try:
        from fourfour_analysis.backends.librosa_waveform import generate_waveform_librosa
        waveforms["Librosa"] = _waveform_cols_to_list(
            generate_waveform_librosa(audio_12k, sr_12k)
        )
    except ImportError:
        waveforms["Librosa"] = None

    try:
        from fourfour_analysis.backends.essentia_waveform import generate_waveform_essentia
        waveforms["Essentia"] = _waveform_cols_to_list(
            generate_waveform_essentia(audio_12k, sr_12k)
        )
    except ImportError:
        waveforms["Essentia"] = None

    output = {
        "bpm": main_result.get("bpm", 0.0),
        "key": main_result.get("key", ""),
        "beats": main_result.get("beats", []),
        "waveform_peaks": main_result.get("waveform_peaks", []),
        "waveform_fft_bands": main_result.get("waveform_fft_bands", []),
        "waveforms": waveforms,
    }

    if args.json_output:
        print(json.dumps(output, indent=2, default=str))
    else:
        print(f"BPM: {output['bpm']}  Key: {output['key']}")
        for name, cols in waveforms.items():
            count = len(cols) if cols else 0
            print(f"  {name}: {count} columns")


def main() -> None:
    """Entry point for python -m fourfour_analysis [analyze|benchmark|waveform-compare] ..."""
    import sys
    if len(sys.argv) > 1 and sys.argv[1] == "analyze":
        sys.argv = [sys.argv[0]] + sys.argv[2:]  # strip "analyze" subcommand
        analyze_main()
    elif len(sys.argv) > 1 and sys.argv[1] == "benchmark":
        sys.argv = [sys.argv[0]] + sys.argv[2:]
        benchmark_main()
    elif len(sys.argv) > 1 and sys.argv[1] == "waveform-compare":
        sys.argv = [sys.argv[0]] + sys.argv[2:]
        waveform_compare_main()
    else:
        print(f"fourfour-analysis v{__version__}")
        print("Use: fourfour-analyze <file>  or  fourfour-benchmark <command>  or  fourfour-waveform-compare <file>")

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
        # bytes → list[int] for JSON serialization
        if isinstance(result_dict.get("waveform_preview"), bytes):
            result_dict["waveform_preview"] = list(result_dict["waveform_preview"])
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


def _normalized_cols_to_display(cols: np.ndarray) -> list[dict]:
    """Convert normalized (N, 3) float64 array to display format.

    Outputs raw {r, g, b} in 0-127 range, matching production's
    WaveformColumn format. The dev tool's cols_to_display() will
    compute per-column amplitude from these raw values.
    """
    out = []
    for i in range(len(cols)):
        low, mid, high = cols[i]
        # Pioneer PWV7 uses 0-127 range; match production output
        out.append({
            "r": min(127, int(round(low * 127))),
            "g": min(127, int(round(mid * 127))),
            "b": min(127, int(round(high * 127))),
        })
    return out


def waveform_compare_main() -> None:
    """Compare waveform generation backends: Lexicon vs Librosa vs Essentia."""
    parser = argparse.ArgumentParser(
        prog="fourfour-waveform-compare",
        description="Generate waveforms from Lexicon, Librosa, and Essentia backends for comparison.",
    )
    parser.add_argument("file", help="Path to audio file")
    parser.add_argument("--json", action="store_true", dest="json_output", help="Output as JSON")
    parser.add_argument("--sweep", action="store_true", dest="sweep", help="Generate many Lexicon parameter variations")
    parser.add_argument("--hypotheses", action="store_true", dest="hypotheses", help="Generate PWV7 hypothesis variants for visual comparison")
    parser.add_argument("--out", type=str, dest="out_path", help="Write JSON output to file (for dev harness)")
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

    from fourfour_analysis.backends.lexicon_waveform import (
        generate_waveform as _lexicon,
        generate_waveform_with_params,
        generate_waveform_filterbank,
        SWEEP_VARIANTS,
        FILTERBANK_VARIANTS,
    )

    waveforms: dict[str, list[dict] | None] = {}

    if args.hypotheses:
        from fourfour_analysis.pwv7_research import HypothesisConfig, generate_pwv7_hypothesis

        # Focused set of visually distinguishable hypotheses
        hypothesis_configs = [
            ("Baseline (sqrt)", HypothesisConfig()),
            ("Power=1.0 linear", HypothesisConfig(power=1.0)),
            ("Power=0.7", HypothesisConfig(power=0.7)),
            ("Envelope + linear", HypothesisConfig(smoothing="envelope", envelope_lookahead=2, power=1.0)),
            ("Track peak + linear", HypothesisConfig(normalize="track_peak", power=1.0)),
            ("Sigmoid", HypothesisConfig(normalize="sigmoid")),
            ("Peak measure", HypothesisConfig(measure="peak")),
            ("True peak", HypothesisConfig(measure="true_peak")),
            ("Mean measure", HypothesisConfig(measure="mean")),
            ("No smoothing", HypothesisConfig(smoothing="none", mix_factor=0.0)),
            ("Mix 0.3", HypothesisConfig(smoothing="mix", mix_factor=0.3)),
            ("Block max 3", HypothesisConfig(smoothing="block_max", block_max_width=3)),
            ("Block max 5", HypothesisConfig(smoothing="block_max", block_max_width=5)),
            ("Compressor", HypothesisConfig(smoothing="compressor", compressor_attack_ms=1.0, compressor_release_ms=100.0)),
            ("LR crossover", HypothesisConfig(filter_type="linkwitz_riley")),
            ("Shelving", HypothesisConfig(filter_type="shelving", filter_order=1)),
            ("FFT bins", HypothesisConfig(filter_type="fft")),
            ("Seg=160", HypothesisConfig(segment_width=160)),
            ("Seg=60", HypothesisConfig(segment_width=60)),
            ("Overlap 50%", HypothesisConfig(overlap=0.5)),
            ("22kHz", HypothesisConfig(target_sr=22000, segment_width=147)),
            ("Low=100/Mid=2k", HypothesisConfig(low_cutoff=100, mid_cutoff=2000)),
            ("Low=200/Mid=4k", HypothesisConfig(low_cutoff=200, mid_cutoff=4000)),
            ("Gain 180/100/60", HypothesisConfig(gain_low=180, gain_mid=100, gain_high=60)),
            ("Gain 200/90/50", HypothesisConfig(gain_low=200, gain_mid=90, gain_high=50)),
            # Peak-hold variants (the key to matching Rekordbox transients)
            ("Peak-hold 3", HypothesisConfig(peak_hold=3)),
            ("Peak-hold 3 + linear", HypothesisConfig(peak_hold=3, power=1.0)),
            ("Peak-hold 5", HypothesisConfig(peak_hold=5)),
        ]

        audio_mono = audio_12k.mean(axis=0) if audio_12k.ndim > 1 else audio_12k
        for label, cfg in hypothesis_configs:
            cols = generate_pwv7_hypothesis(audio_mono, sr_12k, cfg)
            waveforms[label] = _normalized_cols_to_display(cols)

        # Also include production code output
        prod_cols = generate_waveform_filterbank(audio_12k, sr_12k)
        waveforms["Production"] = _waveform_cols_to_list(prod_cols)

    elif args.sweep:
        total = len(FILTERBANK_VARIANTS)
        print(f"Generating {total} filter-bank variants...", file=sys.stderr)
        for i, (label, params) in enumerate(FILTERBANK_VARIANTS, 1):
            print(f"  [{i}/{total}] {label}...", file=sys.stderr, flush=True)
            cols = generate_waveform_filterbank(audio_12k, sr_12k, params)
            waveforms[f"{label}"] = _waveform_cols_to_list(cols)
        print("Done.", file=sys.stderr)
    else:
        # Lexicon waveform columns (re-use already-preprocessed audio)
        lexicon_cols = _lexicon(audio_12k, sr_12k)
        waveforms["Lexicon"] = _waveform_cols_to_list(lexicon_cols)

    # Librosa and Essentia backends disabled — focusing on filter bank tuning.
    # Re-enable when needed for cross-backend comparison.

    output = {
        "bpm": main_result.get("bpm", 0.0),
        "key": main_result.get("key", ""),
        "beats": main_result.get("beats", []),
        "waveform_peaks": main_result.get("waveform_peaks", []),
        "waveform_preview": main_result.get("waveform_preview", []),
        "waveform_overview": main_result.get("waveform_overview", []),
        "waveform_fft_bands": main_result.get("waveform_fft_bands", []),
        "waveforms": waveforms,
    }

    if args.out_path:
        Path(args.out_path).write_text(json.dumps(output, indent=2, default=str))
        print(f"Written to {args.out_path}")

    if args.json_output or args.out_path:
        print(json.dumps(output, indent=2, default=str))
    else:
        print(f"BPM: {output['bpm']}  Key: {output['key']}")
        for name, cols in waveforms.items():
            count = len(cols) if cols else 0
            print(f"  {name}: {count} columns")


def pwv7_hypotheses_main() -> None:
    """Test PWV7 generation hypotheses against Rekordbox reference data."""
    parser = argparse.ArgumentParser(
        prog="fourfour-pwv7-hypotheses",
        description="Systematically test waveform generation hypotheses against Rekordbox PWV7 data.",
    )
    parser.add_argument("path", nargs="+", help="Audio file(s) or directory of audio files to test")
    parser.add_argument("--top-n", type=int, default=20, help="Number of top results to display")
    parser.add_argument("--json-out", type=str, default=None, help="Export all results to JSON file")
    parser.add_argument("--quick", action="store_true", help="Quick mode: test only key hypotheses (~50 variants)")
    args = parser.parse_args()

    from fourfour_analysis.pwv7_research import (
        HypothesisConfig, run_hypothesis_test, print_results, print_band_breakdown, export_results_json
    )

    audio_paths: list[Path] = []
    for p in args.path:
        target = Path(p)
        if target.is_dir():
            audio_paths.extend(sorted([f for f in target.iterdir() if f.suffix.lower() in (".mp3", ".flac", ".wav", ".m4a", ".aac")]))
        else:
            audio_paths.append(target)

    if not audio_paths:
        print(f"No audio files found at {args.path}", file=sys.stderr)
        sys.exit(1)

    print(f"Testing {len(audio_paths)} track(s) against Rekordbox references...")

    if args.quick:
        # Focused subset: ~50 most informative variants
        configs = [
            # Exact production baseline (matches FilterbankParams defaults)
            HypothesisConfig(),
            # Filter topology
            HypothesisConfig(filter_type="linkwitz_riley"),
            HypothesisConfig(filter_type="shelving", filter_order=1),
            HypothesisConfig(filter_type="fft"),
            # Measurement
            HypothesisConfig(measure="peak"),
            HypothesisConfig(measure="true_peak"),
            HypothesisConfig(measure="mean"),
            # Smoothing
            HypothesisConfig(smoothing="none", mix_factor=0.0),
            HypothesisConfig(smoothing="mix", mix_factor=0.05),
            HypothesisConfig(smoothing="mix", mix_factor=0.2),
            HypothesisConfig(smoothing="mix", mix_factor=0.3),
            HypothesisConfig(smoothing="block_max", block_max_width=3),
            HypothesisConfig(smoothing="block_max", block_max_width=5),
            HypothesisConfig(smoothing="envelope", envelope_lookahead=2),
            HypothesisConfig(smoothing="compressor", compressor_attack_ms=1.0, compressor_release_ms=100.0),
            # Normalization
            HypothesisConfig(normalize="track_peak"),
            HypothesisConfig(normalize="track_loudness"),
            HypothesisConfig(normalize="sigmoid"),
            # Power — key finding: linear (1.0) may be better than sqrt (0.5)
            HypothesisConfig(power=0.3),
            HypothesisConfig(power=0.4),
            HypothesisConfig(power=0.6),
            HypothesisConfig(power=0.7),
            HypothesisConfig(power=0.8),
            HypothesisConfig(power=0.9),
            HypothesisConfig(power=1.0),
            # Gains
            HypothesisConfig(gain_low=180, gain_mid=100, gain_high=60),
            HypothesisConfig(gain_low=200, gain_mid=90, gain_high=50),
            HypothesisConfig(gain_low=100, gain_mid=140, gain_high=80),
            # Crossover
            HypothesisConfig(low_cutoff=100, mid_cutoff=2000),
            HypothesisConfig(low_cutoff=160, mid_cutoff=3000),
            HypothesisConfig(low_cutoff=200, mid_cutoff=4000),
            # Window
            HypothesisConfig(segment_width=60),
            HypothesisConfig(segment_width=100),
            HypothesisConfig(segment_width=160),
            HypothesisConfig(overlap=0.5),
            HypothesisConfig(target_sr=22000, segment_width=147),
            # Weighting (FFT mode only)
            HypothesisConfig(filter_type="fft", weighting="a_weight"),
            HypothesisConfig(filter_type="fft", weighting="c_weight"),
            # Peak hold variants (this is what production does!)
            HypothesisConfig(peak_hold=2),
            HypothesisConfig(peak_hold=3),
            HypothesisConfig(peak_hold=5),
            HypothesisConfig(peak_hold=3, power=1.0),
            HypothesisConfig(peak_hold=3, smoothing="envelope", envelope_lookahead=2),
            # Combined best guesses
            HypothesisConfig(power=1.0, normalize="track_peak"),
            HypothesisConfig(power=1.0, smoothing="envelope", envelope_lookahead=2),
        ]
    else:
        configs = None  # Full grid

    def _progress(msg: str) -> None:
        print(f"  {msg}", file=sys.stderr)

    results = run_hypothesis_test(audio_paths, configs=configs, progress_fn=_progress)

    print_results(results, top_n=args.top_n)
    print_band_breakdown(results, top_n=args.top_n)

    if args.json_out:
        export_results_json(results, Path(args.json_out))
        print(f"\nResults exported to {args.json_out}")


def main() -> None:
    """Entry point for python -m fourfour_analysis [analyze|benchmark|waveform-compare|pwv7-hypotheses] ..."""
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
    elif len(sys.argv) > 1 and sys.argv[1] == "pwv7-hypotheses":
        sys.argv = [sys.argv[0]] + sys.argv[2:]
        pwv7_hypotheses_main()
    else:
        print(f"fourfour-analysis v{__version__}")
        print("Use: fourfour-analyze <file>  or  fourfour-benchmark <command>  or  fourfour-waveform-compare <file>  or  fourfour-pwv7-hypotheses <path>")

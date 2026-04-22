#!/usr/bin/env python3
"""Batch-test `fourfour-analyze` against a folder of audio files.

This script intentionally calls the public CLI in chunks. It is for black-box
validation of the analysis contract that Rust will later consume.

Examples:
  # Run in the current terminal.
  analysis/.venv/bin/python benchmark/scripts/cli_batch_analyze.py \
    benchmark/datasets/beatport-edm-key/audio \
    --limit 50

  # Open a detached tmux pane, stream a log, and close the pane when done.
  analysis/.venv/bin/python benchmark/scripts/cli_batch_analyze.py \
    benchmark/datasets/beatport-edm-key/audio \
    --tmux
"""

from __future__ import annotations

import argparse
import json
import shlex
import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
ANALYSIS_DIR = ROOT / "analysis"
DEFAULT_CLI = ANALYSIS_DIR / ".venv" / "bin" / "fourfour-analyze"
DEFAULT_RESULTS_ROOT = ROOT / "benchmark" / "results"
DEFAULT_LOG_ROOT = ROOT / "benchmark" / "logs"
DEFAULT_AUDIO_EXTENSIONS = (".mp3", ".wav", ".flac", ".aiff", ".aif", ".m4a", ".ogg")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Run fourfour-analyze over a folder and save raw analysis output.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument("audio_dir", help="Folder containing audio files to analyze recursively.")
    parser.add_argument("--limit", type=int, help="Maximum number of matching tracks to analyze.")
    parser.add_argument("--offset", type=int, default=0, help="Skip the first N sorted matching tracks.")
    parser.add_argument("--workers", type=int, default=1, help="Workers passed to fourfour-analyze for each chunk. Default: 1.")
    parser.add_argument("--chunk-size", type=int, default=25, help="Files per fourfour-analyze invocation. Default: 25.")
    parser.add_argument("--run-id", help="Run id / output directory name. Default: generated timestamp.")
    parser.add_argument("--output-root", default=str(DEFAULT_RESULTS_ROOT), help="Directory for run artifacts.")
    parser.add_argument("--log-root", default=str(DEFAULT_LOG_ROOT), help="Directory for tmux logs.")
    parser.add_argument("--cli", default=str(DEFAULT_CLI), help="Path to fourfour-analyze.")
    parser.add_argument("--extensions", default=",".join(DEFAULT_AUDIO_EXTENSIONS), help="Comma-separated audio extensions.")
    parser.add_argument("--exclude", action="append", default=[], help="Filename to exclude. Can be repeated.")
    parser.add_argument("--timeout", type=int, default=240, help="Per-track CLI timeout in seconds.")
    parser.add_argument("--tmux", action="store_true", help="Launch this run in a new tmux pane and return immediately.")
    parser.add_argument("--tmux-session", default="fourfour", help="tmux session target for --tmux.")
    parser.add_argument("--tmux-pane-title", default="fourfour-cli-benchmark", help="tmux pane title for --tmux.")
    return parser


def generated_run_id(limit: int | None) -> str:
    size = "full" if limit is None else str(limit)
    return f"cli-batch-{size}-{time.strftime('%Y%m%dT%H%M%S')}"


def spawn_tmux(args: argparse.Namespace, argv: list[str]) -> int:
    run_id = args.run_id or generated_run_id(args.limit)
    log_root = Path(args.log_root).resolve()
    log_root.mkdir(parents=True, exist_ok=True)
    log_path = log_root / f"{run_id}.log"

    child_argv = [a for a in argv if a != "--tmux"]
    if "--run-id" not in child_argv:
        child_argv.extend(["--run-id", run_id])

    script = Path(__file__).resolve()
    python = sys.executable
    command_parts = [shlex.quote(python), shlex.quote(str(script)), *map(shlex.quote, child_argv)]
    command = " ".join(command_parts)
    shell_command = f"printf '\\033]2;{args.tmux_pane_title}\\033\\\\'; {command} 2>&1 | tee {shlex.quote(str(log_path))}"

    proc = subprocess.run(
        [
            "tmux",
            "split-window",
            "-t",
            args.tmux_session,
            "-d",
            "-P",
            "-F",
            "#{pane_id}",
            "-c",
            str(ROOT),
            "zsh",
            "-lc",
            shell_command,
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        print(proc.stderr.strip() or proc.stdout.strip(), file=sys.stderr)
        return proc.returncode

    pane_id = proc.stdout.strip()
    print(f"tmux_pane={pane_id}")
    print(f"run_id={run_id}")
    print(f"log={log_path}")
    print(f"results={Path(args.output_root).resolve() / run_id}")
    return 0


def load_tracks(args: argparse.Namespace) -> list[Path]:
    audio_dir = Path(args.audio_dir).resolve()
    if not audio_dir.is_dir():
        raise FileNotFoundError(f"audio_dir not found: {audio_dir}")

    extensions = tuple(
        ext.strip().lower() if ext.strip().startswith(".") else f".{ext.strip().lower()}"
        for ext in args.extensions.split(",")
        if ext.strip()
    )
    excluded = set(args.exclude)
    tracks = [
        p
        for p in sorted(audio_dir.rglob("*"))
        if p.is_file() and p.suffix.lower() in extensions and p.name not in excluded
    ]
    if args.offset:
        tracks = tracks[args.offset :]
    if args.limit is not None:
        tracks = tracks[: args.limit]
    return tracks


def run_cli_chunk(paths: list[Path], args: argparse.Namespace) -> tuple[list[dict[str, object] | None], float, str | None]:
    started = time.monotonic()
    proc = subprocess.run(
        [args.cli, *[str(p) for p in paths], "--json", "--workers", str(args.workers)],
        cwd=ANALYSIS_DIR,
        capture_output=True,
        text=True,
        timeout=max(args.timeout, args.timeout * len(paths)),
        check=False,
    )
    elapsed = time.monotonic() - started
    if proc.returncode != 0:
        error = proc.stderr.strip() or proc.stdout.strip() or f"returncode {proc.returncode}"
        return [None for _ in paths], elapsed, error

    try:
        payload = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        return [None for _ in paths], elapsed, f"json decode failed: {exc}"

    if isinstance(payload, dict):
        payload = [payload]
    if not isinstance(payload, list):
        return [None for _ in paths], elapsed, "CLI JSON was not an object or list"
    if len(payload) != len(paths):
        return [None for _ in paths], elapsed, f"CLI returned {len(payload)} results for {len(paths)} inputs"

    return payload, elapsed, None


def chunks(items: list[Path], size: int) -> list[list[Path]]:
    return [items[i : i + size] for i in range(0, len(items), size)]


def summarize(run_id: str, rows: list[dict[str, object]], elapsed: float) -> dict[str, object]:
    successful = [r for r in rows if r.get("returncode") == 0 and "result" in r]
    waveform_ok = sum(
        1
        for r in successful
        if r.get("waveform_preview_len") == 400
        and r.get("waveform_color_len") == 2000
        and r.get("waveform_peaks_len") == 2000
        and r.get("pioneer_3band_detail_len", 0) > 0
        and r.get("pioneer_3band_overview_len") == 1200
    )
    return {
        "run_id": run_id,
        "elapsed_seconds": elapsed,
        "tracks_run": len(rows),
        "successful": len(successful),
        "failures": len(rows) - len(successful),
        "waveform_shape_ok": waveform_ok,
        "waveform_shape_ok_pct": round(waveform_ok / len(successful) * 100, 1) if successful else None,
    }


def run(args: argparse.Namespace) -> int:
    run_id = args.run_id or generated_run_id(args.limit)
    out_dir = Path(args.output_root).resolve() / run_id
    out_dir.mkdir(parents=True, exist_ok=True)
    tracks = load_tracks(args)
    total = len(tracks)

    print(f"run_id={run_id}")
    print(f"audio_dir={Path(args.audio_dir).resolve()}")
    print(f"tracks={total}")
    print(f"workers={args.workers}")
    print(f"chunk_size={args.chunk_size}")
    print(f"output_dir={out_dir}")
    print("")
    sys.stdout.flush()

    rows: list[dict[str, object]] = []
    started = time.monotonic()
    track_chunks = chunks(tracks, max(1, args.chunk_size))
    for chunk_index, track_chunk in enumerate(track_chunks, start=1):
        detected_items, chunk_elapsed, chunk_error = run_cli_chunk(track_chunk, args)
        per_track_elapsed = chunk_elapsed / len(track_chunk) if track_chunk else 0.0
        if chunk_error:
            print(f"chunk {chunk_index}/{len(track_chunks)} failed: {chunk_error}")
        for offset, (audio_path, detected) in enumerate(zip(track_chunk, detected_items), start=0):
            index = (chunk_index - 1) * max(1, args.chunk_size) + offset + 1
            row: dict[str, object] = {
                "index": index,
                "total": total,
                "filename": audio_path.name,
                "path": str(audio_path),
                "returncode": 0 if detected is not None else 1,
                "cli_elapsed_seconds": per_track_elapsed,
            }
            if detected is None:
                row["error"] = chunk_error or "missing CLI result"
                rows.append(row)
                print(
                    f"{index:04d}/{total} fail "
                    f"path={audio_path.name} "
                    f"t={row.get('cli_elapsed_seconds', 0):.2f}s"
                )
                sys.stdout.flush()
                continue

            row["result"] = detected
            row["bpm"] = detected.get("bpm")
            row["key"] = detected.get("key")
            row["energy"] = detected.get("energy")
            row["errors"] = detected.get("errors", [])
            row["waveform_preview_len"] = len(detected.get("waveform_preview", []))
            row["waveform_color_len"] = len(detected.get("waveform_color", []))
            row["waveform_peaks_len"] = len(detected.get("waveform_peaks", []))
            row["pioneer_3band_detail_len"] = len(detected.get("pioneer_3band_detail", []))
            row["pioneer_3band_overview_len"] = len(detected.get("pioneer_3band_overview", []))
            row["beats_len"] = len(detected.get("beats", []))
            row["cue_points_len"] = len(detected.get("cue_points", []))
            rows.append(row)
            print(
                f"{index:04d}/{total} ok "
                f"bpm={row.get('bpm')} "
                f"key={row.get('key')} "
                f"t={row.get('cli_elapsed_seconds', 0):.2f}s"
            )
            sys.stdout.flush()

    rows.sort(key=lambda r: r["index"])
    elapsed = time.monotonic() - started
    summary = summarize(run_id, rows, elapsed)
    (out_dir / "results.json").write_text(json.dumps(rows, indent=2))
    (out_dir / "summary.json").write_text(json.dumps(summary, indent=2))

    print("")
    print(json.dumps(summary, indent=2))
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    raw_argv = list(sys.argv[1:] if argv is None else argv)
    args = parser.parse_args(raw_argv)
    if args.tmux:
        return spawn_tmux(args, raw_argv)
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())

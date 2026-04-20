"""CLI entry point for fourfour-analyze."""

import json
import sys
from pathlib import Path

import click

from fourfour_analysis.analyze import analyze_track, analyze_batch


@click.group()
def main():
    """fourfour-analyze — Audio analysis CLI."""
    pass


@main.command()
@click.argument("paths", nargs=-1, type=click.Path(exists=True))
@click.option("--dir", "directory", type=click.Path(exists=True), help="Analyze all audio files in a directory")
@click.option("--json", "output_json", is_flag=True, default=False, help="Output JSON to stdout")
@click.option("--output", "-o", type=click.Path(), help="Write JSON output to file")
@click.option("--workers", "-w", type=int, default=4, help="Number of parallel workers")
def analyze(paths, directory, output_json, output, workers):
    """Analyze audio files for BPM, key, energy, and waveform data."""
    file_list = list(paths)

    if directory:
        dir_path = Path(directory)
        extensions = {".mp3", ".wav", ".flac", ".aiff", ".aif", ".m4a", ".ogg"}
        file_list.extend(
            str(p) for p in dir_path.rglob("*") if p.suffix.lower() in extensions
        )

    if not file_list:
        click.echo("No audio files specified.", err=True)
        sys.exit(1)

    if len(file_list) == 1:
        results = [analyze_track(file_list[0])]
    else:
        results = analyze_batch(file_list, workers=workers)

    json_str = json.dumps(results, indent=2)

    if output:
        Path(output).write_text(json_str)
        click.echo(f"Results written to {output}", err=True)
    elif output_json:
        click.echo(json_str)
    else:
        # Human-readable summary
        for r in results:
            name = Path(r["path"]).name
            bpm = r.get("bpm", "?")
            key = r.get("key", "?")
            energy = r.get("energy", {})
            e_score = energy.get("score", "?") if energy else "?"
            errors = len(r.get("errors", []))
            click.echo(f"{name}: BPM={bpm} Key={key} Energy={e_score}/10 Errors={errors}")


@main.command()
@click.option("--masterdb", type=click.Path(exists=True), help="Path to Rekordbox master.db")
@click.option("--playlist", type=str, help="Playlist name to benchmark from master.db")
@click.option("--workers", "-w", type=int, default=4, help="Number of parallel workers")
@click.option("--json", "output_json", is_flag=True, default=False, help="Output JSON report")
def benchmark(masterdb, playlist, workers, output_json):
    """Benchmark analysis against Rekordbox master.db ground truth."""
    import platform
    from fourfour_analysis.benchmark import run_benchmark

    # Default master.db path
    if masterdb is None:
        if platform.system() == "Darwin":
            masterdb = str(Path.home() / "Library/Pioneer/rekordbox/master.db")
        else:
            click.echo("Please specify --masterdb path", err=True)
            sys.exit(1)

    if not Path(masterdb).exists():
        click.echo(f"master.db not found at {masterdb}", err=True)
        sys.exit(1)

    # Read ground truth via the Rust masterdb reader CLI
    proc = subprocess.run(
        ["cargo", "run", "-p", "pioneer-test-ui", "--", "read-masterdb", masterdb, "--json"],
        capture_output=True, text=True,
        cwd=str(Path(__file__).parents[3]),
    )
    if proc.returncode != 0:
        click.echo(f"Failed to read master.db: {proc.stderr}", err=True)
        sys.exit(1)

    ground_truth = json.loads(proc.stdout)

    # Filter to playlist if specified
    if playlist:
        playlist_tracks = None
        for pl in ground_truth.get("playlists", []):
            if pl["name"] == playlist:
                playlist_tracks = set(pl["track_ids"])
                break
        if playlist_tracks is None:
            click.echo(f"Playlist '{playlist}' not found in master.db", err=True)
            sys.exit(1)
        gt_tracks = [t for t in ground_truth["tracks"] if t["id"] in playlist_tracks]
    else:
        gt_tracks = ground_truth["tracks"]

    # Analyze the same tracks
    paths = [t["source_path"] for t in gt_tracks if Path(t["source_path"]).exists()]
    click.echo(f"Analyzing {len(paths)} tracks...", err=True)
    results = analyze_batch(paths, workers=workers)

    # Compare
    report = run_benchmark(results, gt_tracks)

    if output_json:
        click.echo(json.dumps({
            "total": report.total_tracks,
            "bpm_accuracy": report.bpm_accuracy,
            "key_accuracy": report.key_accuracy,
            "bpm_octave_errors": report.bpm_octave_errors,
            "key_relative_errors": report.key_relative_errors,
        }, indent=2))
    else:
        click.echo(report.summary())

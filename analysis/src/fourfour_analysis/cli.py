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

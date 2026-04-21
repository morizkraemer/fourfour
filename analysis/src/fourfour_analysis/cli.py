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


MASTERDB_KEY = "402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497"


def read_masterdb_tracks(db_path: str, playlist_name: str | None = None) -> list[dict]:
    """Read tracks from Rekordbox master.db (SQLCipher encrypted).

    Returns list of dicts with: source_path, tempo, key
    """
    try:
        from pysqlcipher3 import dbapi2 as sqlite
    except ImportError:
        raise ImportError(
            "pysqlcipher3 required for reading master.db.\n"
            "Install: pip install pysqlcipher3\n"
            "Requires sqlcipher library: brew install sqlcipher"
        )

    conn = sqlite.connect(db_path)
    conn.execute(f"PRAGMA key = '{MASTERDB_KEY}'")
    conn.execute("PRAGMA cipher_compatibility = 4")

    # Read key lookup table
    keys = {}
    for row in conn.execute("SELECT ID, ScaleName FROM djmdKey"):
        keys[row[0]] = row[1]

    # Read tracks
    query = """
        SELECT ID, FolderPath, BPM, KeyID
        FROM djmdContent
        WHERE (rb_local_deleted = 0 OR rb_local_deleted IS NULL)
    """
    tracks = []
    for row in conn.execute(query):
        content_id, folder_path, bpm_x100, key_id = row
        key = keys.get(key_id, "")
        tracks.append({
            "id": content_id,
            "source_path": folder_path,
            "tempo": bpm_x100 or 0,
            "key": key,
        })

    # Filter by playlist if specified
    if playlist_name:
        playlist_row = conn.execute(
            "SELECT ID FROM djmdPlaylist WHERE Name = ?", (playlist_name,)
        ).fetchone()
        if playlist_row is None:
            conn.close()
            raise click.ClickException(f"Playlist '{playlist_name}' not found")

        playlist_id = playlist_row[0]
        playlist_track_ids = set()
        for row in conn.execute(
            "SELECT TrackID FROM djmdSongPlaylist WHERE PlaylistID = ?", (playlist_id,)
        ):
            playlist_track_ids.add(row[0])

        tracks = [t for t in tracks if t["id"] in playlist_track_ids]

    conn.close()
    return tracks


@main.command()
@click.option("--masterdb", type=click.Path(exists=True), help="Path to Rekordbox master.db")
@click.option("--playlist", type=str, help="Playlist name to benchmark")
@click.option("--workers", "-w", type=int, default=4, help="Number of parallel workers")
@click.option("--json", "output_json", is_flag=True, default=False, help="Output JSON report")
def benchmark(masterdb, playlist, workers, output_json):
    """Benchmark analysis against Rekordbox master.db ground truth."""
    import platform
    from fourfour_analysis.benchmark import run_benchmark
    from fourfour_analysis.analyze import analyze_batch

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

    # Read ground truth
    try:
        ground_truth = read_masterdb_tracks(masterdb, playlist)
    except ImportError as e:
        click.echo(str(e), err=True)
        sys.exit(1)

    click.echo(f"Read {len(ground_truth)} tracks from master.db", err=True)

    # Filter to tracks that exist on disk
    paths = [t["source_path"] for t in ground_truth if Path(t["source_path"]).exists()]
    click.echo(f"Analyzing {len(paths)} tracks (found on disk)...", err=True)

    # Analyze
    results = analyze_batch(paths, workers=workers)

    # Compare
    report = run_benchmark(results, ground_truth)

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

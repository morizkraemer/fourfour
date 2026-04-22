"""Corpus builder — scan tagged audio files into benchmark manifest.

CLI: fourfour-benchmark init ~/Music/corpus --name accuracy-v1
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
from typing import Optional

from fourfour_analysis.types import TrackEntry, GroundTruth


AUDIO_EXTENSIONS = {".mp3", ".wav", ".flac", ".aiff", ".ogg", ".m4a", ".aac", ".wma"}


def build_corpus(
    directory: str | Path,
    name: str,
    output_dir: Path,
) -> Path:
    """Scan a directory of tagged audio files and produce a corpus JSON.

    Args:
        directory: Directory to scan recursively for audio files.
        name: Corpus name (used for output filename).
        output_dir: Directory to write the corpus JSON.

    Returns:
        Path to the written corpus JSON.
    """
    directory = Path(directory)
    if not directory.is_dir():
        raise FileNotFoundError(f"Directory not found: {directory}")

    entries = []
    track_id = 0

    for root, _dirs, files in os.walk(directory):
        for fname in sorted(files):
            path = Path(root) / fname
            if path.suffix.lower() not in AUDIO_EXTENSIONS:
                continue

            track_id += 1
            entry = _make_entry(path, track_id, directory)
            entries.append(entry)

    # Write corpus JSON
    output_dir.mkdir(parents=True, exist_ok=True)
    corpus_path = output_dir / f"{name}.corpus.json"

    corpus_data = {
        "name": name,
        "source_directory": str(directory),
        "num_entries": len(entries),
        "entries": [_entry_to_dict(e) for e in entries],
    }

    corpus_path.write_text(json.dumps(corpus_data, indent=2))
    return corpus_path


def load_corpus(path: str | Path) -> list[TrackEntry]:
    """Load a corpus JSON file.

    Args:
        path: Path to the .corpus.json file.

    Returns:
        List of TrackEntry with embedded GroundTruth.
    """
    path = Path(path)
    if not path.is_file():
        raise FileNotFoundError(f"Corpus not found: {path}")

    data = json.loads(path.read_text())
    entries = []

    for item in data.get("entries", []):
        gt_data = item.get("ground_truth")
        gt = None
        if gt_data:
            gt = GroundTruth(
                track_id=item["id"],
                bpm=gt_data.get("bpm"),
                key=gt_data.get("key"),
                energy=gt_data.get("energy"),
                bpm_source=gt_data.get("bpm_source"),
                key_source=gt_data.get("key_source"),
            )

        entry = TrackEntry(
            id=item["id"],
            path=item["path"],
            content_fingerprint=item["content_fingerprint"],
            artist=item.get("artist", ""),
            title=item.get("title", ""),
            genre=item.get("genre", ""),
            duration_seconds=item.get("duration_seconds"),
            ground_truth=gt,
        )
        entries.append(entry)

    return entries


def _make_entry(path: Path, track_id: int, base_dir: Path) -> TrackEntry:
    """Create a TrackEntry from an audio file, extracting tags as ground truth."""
    fingerprint = _compute_fingerprint(path)
    tags = _extract_tags(path)
    duration = _estimate_duration(path)

    return TrackEntry(
        id=str(track_id),
        path=str(path.resolve()),
        content_fingerprint=fingerprint,
        artist=tags.get("artist", ""),
        title=tags.get("title", path.stem),
        genre=tags.get("genre", ""),
        duration_seconds=duration,
        ground_truth=GroundTruth(
            track_id=str(track_id),
            bpm=tags.get("bpm"),
            key=tags.get("key"),
            energy=tags.get("energy"),
            bpm_source="tag" if tags.get("bpm") is not None else None,
            key_source="tag" if tags.get("key") is not None else None,
        ) if any(tags.get(k) is not None for k in ("bpm", "key", "energy")) else None,
    )


def _compute_fingerprint(path: Path) -> str:
    """SHA256 of first 64KB + last 64KB + file size."""
    file_size = path.stat().st_size
    chunk_size = 64 * 1024

    h = hashlib.sha256()
    with open(path, "rb") as f:
        # First 64KB
        h.update(f.read(chunk_size))
        # Last 64KB
        if file_size > chunk_size:
            f.seek(max(0, file_size - chunk_size))
            h.update(f.read(chunk_size))
    h.update(str(file_size).encode())

    return h.hexdigest()


def _extract_tags(path: Path) -> dict:
    """Extract BPM, key, energy, artist, title, genre from file tags.

    Uses mutagen if available, falls back to minimal extraction.
    """
    tags: dict = {}

    try:
        import mutagen

        try:
            f = mutagen.File(str(path))
        except Exception:
            return tags

        if f is None:
            return tags

        # Get tags depending on format
        if hasattr(f, 'tags') and f.tags is not None:
            tag_dict = f.tags

            # BPM
            for key in ("TBPM", "bpm", "BPM", "fBPM"):
                if key in tag_dict:
                    try:
                        tags["bpm"] = float(str(tag_dict[key]))
                        break
                    except (ValueError, TypeError):
                        pass

            # Key
            for key in ("TKEY", "initialkey", "INITIALKEY", "key", "KEY"):
                if key in tag_dict:
                    raw = str(tag_dict[key]).strip()
                    normalized = _normalize_key(raw)
                    if normalized:
                        tags["key"] = normalized
                    break

            # Energy
            for key in ("TXXX:Energy", "energy", "ENERGY"):
                if key in tag_dict:
                    try:
                        tags["energy"] = int(str(tag_dict[key]))
                        break
                    except (ValueError, TypeError):
                        pass

            # Artist, title, genre
            for key in ("TPE1", "artist", "ARTIST"):
                if key in tag_dict:
                    tags["artist"] = str(tag_dict[key])
                    break
            for key in ("TIT2", "title", "TITLE"):
                if key in tag_dict:
                    tags["title"] = str(tag_dict[key])
                    break
            for key in ("TCON", "genre", "GENRE"):
                if key in tag_dict:
                    tags["genre"] = str(tag_dict[key])
                    break

    except ImportError:
        pass  # mutagen not available

    return tags


def _normalize_key(raw: str) -> Optional[str]:
    """Normalize key notation to Camelot.

    Handles: "8A", "8B", "Abm", "G# major", "C minor", etc.
    """
    from fourfour_analysis.groundtruth import normalize_key

    return normalize_key(raw)


def _estimate_duration(path: Path) -> Optional[float]:
    """Estimate duration from file metadata or skip."""
    try:
        import mutagen
        f = mutagen.File(str(path))
        if f is not None and hasattr(f, 'info') and f.info is not None:
            return f.info.length
    except Exception:
        pass
    return None


def _entry_to_dict(entry: TrackEntry) -> dict:
    """Serialize TrackEntry to JSON dict."""
    d = {
        "id": entry.id,
        "path": entry.path,
        "content_fingerprint": entry.content_fingerprint,
        "artist": entry.artist,
        "title": entry.title,
        "genre": entry.genre,
    }
    if entry.duration_seconds is not None:
        d["duration_seconds"] = entry.duration_seconds
    if entry.ground_truth is not None:
        gt = entry.ground_truth
        d["ground_truth"] = {}
        if gt.bpm is not None:
            d["ground_truth"]["bpm"] = gt.bpm
        if gt.key is not None:
            d["ground_truth"]["key"] = gt.key
        if gt.energy is not None:
            d["ground_truth"]["energy"] = gt.energy
        if gt.bpm_source is not None:
            d["ground_truth"]["bpm_source"] = gt.bpm_source
        if gt.key_source is not None:
            d["ground_truth"]["key_source"] = gt.key_source
    return d

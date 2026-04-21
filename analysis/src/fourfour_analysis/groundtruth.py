"""Ground truth extraction — read BPM/key/energy from file tags.

Separated from manifest.py for reuse (e.g. reading tags on single files,
or importing ground truth from Rekordbox libraries).

Uses mutagen for ID3/Vorbis/MP4 tag reading. Falls back gracefully
if mutagen is not installed.
"""

from __future__ import annotations

from pathlib import Path
from typing import Optional

from fourfour_analysis.types import GroundTruth


# ── Tag key mappings per format ───────────────────────────

_ID3_MAP = {
    "bpm": ["TBPM", "bpm", "BPM", "fBPM"],
    "key": ["TKEY", "initialkey", "INITIALKEY", "key", "KEY"],
    "energy": ["TXXX:Energy", "energy", "ENERGY"],
    "artist": ["TPE1", "artist", "ARTIST"],
    "title": ["TIT2", "title", "TITLE"],
    "genre": ["TCON", "genre", "GENRE"],
}

_VORBIS_MAP = {
    "bpm": ["bpm", "BPM", "TBPM"],
    "key": ["initialkey", "INITIALKEY", "key", "KEY", "TKEY"],
    "energy": ["energy", "ENERGY"],
    "artist": ["artist", "ARTIST"],
    "title": ["title", "TITLE"],
    "genre": ["genre", "GENRE"],
}

_MP4_MAP = {
    "bpm": ["tmpo", "----:com.apple.iTunes:BPM"],
    "key": ["----:com.apple.iTunes:initialkey", "----:com.apple.iTunes:KEY"],
    "energy": ["----:com.apple.iTunes:ENERGY"],
    "artist": ["\xa9ART"],
    "title": ["\xa9nam"],
    "genre": ["\xa9gen"],
}


def extract_tags(path: str | Path) -> dict:
    """Extract BPM, key, energy, and metadata from audio file tags.

    Args:
        path: Path to audio file.

    Returns:
        Dict with keys: bpm (float|None), key (str|None), energy (int|None),
        artist, title, genre (all str, default "").
    """
    path = Path(path)
    result: dict = {"artist": "", "title": "", "genre": ""}

    try:
        import mutagen
    except ImportError:
        return result

    try:
        f = mutagen.File(str(path))
    except Exception:
        return result

    if f is None:
        return result

    # Pick tag mapping based on format
    tags = f.tags
    if tags is None:
        return result

    # Detect format
    from mutagen.id3 import ID3
    from mutagen.vorbis import VorbisComment
    from mutagen.mp4 import MP4Tags

    if isinstance(tags, ID3):
        tag_map = _ID3_MAP
    elif isinstance(tags, MP4Tags):
        tag_map = _MP4_MAP
    else:
        tag_map = _VORBIS_MAP

    # Extract each field
    for field, keys in tag_map.items():
        for key in keys:
            if key in tags:
                val = tags[key]
                if isinstance(val, list):
                    val = val[0] if val else ""
                raw = str(val).strip()

                if field == "bpm" and raw:
                    try:
                        result["bpm"] = float(raw)
                    except (ValueError, TypeError):
                        pass
                    break
                elif field == "key" and raw:
                    normalized = normalize_key(raw)
                    if normalized:
                        result["key"] = normalized
                    break
                elif field == "energy" and raw:
                    try:
                        result["energy"] = int(float(raw))
                    except (ValueError, TypeError):
                        pass
                    break
                elif field in ("artist", "title", "genre") and raw:
                    result[field] = raw
                    break

    return result


def normalize_key(raw: str) -> Optional[str]:
    """Normalize key notation to Camelot (e.g. "8A", "3B").

    Handles:
      - Already Camelot: "8A", "8B"
      - Note names: "Abm", "G# major", "C minor", "F#", "Bbm"
      - Camelot numbers: "8", "12" (defaults to minor/A)
    """
    if not raw:
        return None

    raw = raw.strip()
    if not raw:
        return None

    # Camelot: number + A/B
    if len(raw) >= 2:
        num_part = raw[:-1]
        letter = raw[-1].upper()
        if letter in ("A", "B") and num_part.isdigit():
            n = int(num_part)
            if 1 <= n <= 12:
                return f"{n}{letter}"

    # Note name → Camelot
    NOTE_TO_CAMELOT_MAJOR = {
        "C": "8B", "C#": "3B", "DB": "3B", "D": "10B", "D#": "5B", "EB": "5B",
        "E": "12B", "F": "7B", "F#": "2B", "GB": "2B", "G": "9B", "G#": "4B", "AB": "4B",
        "A": "11B", "A#": "6B", "BB": "6B", "B": "1B",
    }
    NOTE_TO_CAMELOT_MINOR = {
        "C": "8A", "C#": "3A", "DB": "3A", "D": "10A", "D#": "5A", "EB": "5A",
        "E": "12A", "F": "7A", "F#": "2A", "GB": "2A", "G": "9A", "G#": "4A", "AB": "4A",
        "A": "11A", "A#": "6A", "BB": "6A", "B": "1A",
    }

    lower = raw.lower().replace("♯", "#").replace("♭", "b")
    parts = lower.split()
    if not parts:
        return None

    # Detect mode
    is_minor = "min" in lower or "minor" in lower
    if "major" in lower:
        is_minor = False
    # Single note with trailing 'm' (but not a longer word like 'major')
    if len(parts) == 1 and parts[0].endswith("m") and not parts[0].endswith("maj"):
        is_minor = True

    # Extract note
    note = parts[0]
    # Strip mode suffixes (use removesuffix, not rstrip which is character-based)
    for suffix in ("minor", "major", "min", "maj"):
        if note.endswith(suffix):
            note = note[:-len(suffix)]
            break
    note = note.upper()
    # Strip trailing M for minor shorthand (Am → A, F#m → F#)
    if note.endswith("M") and len(note) >= 2:
        candidate = note[:-1]
        # Only strip if what's left is a valid note (not just "#" or empty)
        if candidate and candidate not in ("#",):
            note = candidate

    # Flats → sharps
    FLAT_TO_SHARP = {
        "DB": "C#", "EB": "D#", "FB": "E", "GB": "F#",
        "AB": "G#", "BB": "A#", "CB": "B",
    }
    if note in FLAT_TO_SHARP:
        note = FLAT_TO_SHARP[note]

    mapping = NOTE_TO_CAMELOT_MINOR if is_minor else NOTE_TO_CAMELOT_MAJOR
    return mapping.get(note)


def extract_ground_truth(path: str | Path, track_id: str = "") -> Optional[GroundTruth]:
    """Extract ground truth from file tags.

    Args:
        path: Path to audio file.
        track_id: Track ID to embed in GroundTruth.

    Returns:
        GroundTruth if any tags found, None otherwise.
    """
    tags = extract_tags(path)
    has_data = any(tags.get(k) is not None for k in ("bpm", "key", "energy"))

    if not has_data:
        return None

    return GroundTruth(
        track_id=track_id,
        bpm=tags.get("bpm"),
        key=tags.get("key"),
        energy=tags.get("energy"),
        bpm_source="tag" if tags.get("bpm") is not None else None,
        key_source="tag" if tags.get("key") is not None else None,
    )

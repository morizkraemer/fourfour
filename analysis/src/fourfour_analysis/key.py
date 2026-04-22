"""Musical key detection via Essentia KeyExtractor bgate."""

from __future__ import annotations

from fourfour_analysis.backends.essentia_key import EssentiaKeyBackend

# Camelot wheel mapping
CAMELOT_MAP = {
    "C major": "12B", "G major": "7B", "D major": "2B", "A major": "9B",
    "E major": "4B", "B major": "11B", "F# major": "6B", "C# major": "1B",
    "G# major": "8B", "D# major": "3B", "A# major": "10B", "F major": "5B",
    "A minor": "8A", "E minor": "3A", "B minor": "10A", "F# minor": "5A",
    "C# minor": "12A", "G# minor": "7A", "D# minor": "2A", "A# minor": "9A",
    "F minor": "4A", "C minor": "11A", "G minor": "6A", "D minor": "1A",
}

def to_camelot(key_name: str) -> str | None:
    """Convert 'A minor' or 'C major' to Camelot notation ('8A', '12B')."""
    return CAMELOT_MAP.get(key_name)


def detect_key(path: str, duration: float = 30.0) -> str | None:
    """Detect musical key, returned in Camelot notation (e.g. '8A').

    The duration argument is accepted for compatibility with the older API.
    Returns None on failure.
    """
    try:
        return EssentiaKeyBackend(profile_type="bgate").analyze_track(path).key
    except Exception:
        return None

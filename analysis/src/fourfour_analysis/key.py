"""Musical key detection via librosa chroma + Krumhansl-Schmuckler."""

from __future__ import annotations

import librosa
import numpy as np

# Camelot wheel mapping
CAMELOT_MAP = {
    "C major": "12B", "G major": "7B", "D major": "2B", "A major": "9B",
    "E major": "4B", "B major": "11B", "F# major": "6B", "C# major": "1B",
    "G# major": "8B", "D# major": "3B", "A# major": "10B", "F major": "5B",
    "A minor": "8A", "E minor": "3A", "B minor": "10A", "F# minor": "5A",
    "C# minor": "12A", "G# minor": "7A", "D# minor": "2A", "A# minor": "9A",
    "F minor": "4A", "C minor": "11A", "G minor": "6A", "D minor": "1A",
}

# Krumhansl-Schmuckler key profiles
_MAJOR_PROFILE = np.array([6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88])
_MINOR_PROFILE = np.array([6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17])

_NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]


def to_camelot(key_name: str) -> str | None:
    """Convert 'A minor' or 'C major' to Camelot notation ('8A', '12B')."""
    return CAMELOT_MAP.get(key_name)


def detect_key(path: str, duration: float = 30.0) -> str | None:
    """Detect musical key, returned in Camelot notation (e.g. '8A').

    Analyzes the first `duration` seconds of the track.
    Returns None on failure.
    """
    try:
        y, sr = librosa.load(path, sr=22050, duration=duration)
        chroma = librosa.feature.chroma_cqt(y=y, sr=sr)
        chroma_avg = chroma.mean(axis=1)

        best_corr = -1.0
        best_key = "C major"

        for i in range(12):
            rotated = np.roll(chroma_avg, -i)
            corr_major = np.corrcoef(rotated, _MAJOR_PROFILE)[0, 1]
            corr_minor = np.corrcoef(rotated, _MINOR_PROFILE)[0, 1]

            if corr_major > best_corr:
                best_corr = corr_major
                best_key = f"{_NOTE_NAMES[i]} major"
            if corr_minor > best_corr:
                best_corr = corr_minor
                best_key = f"{_NOTE_NAMES[i]} minor"

        return to_camelot(best_key)
    except Exception:
        return None

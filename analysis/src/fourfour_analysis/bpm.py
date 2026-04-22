"""BPM detection via the final analysis stack."""

from __future__ import annotations

from fourfour_analysis.backends.final_stack import FinalStackBackend


def detect_bpm(path: str) -> float | None:
    """Detect BPM for an audio file. Returns None on failure.

    Applies octave correction: doubles BPM < 70, halves BPM > 200.
    """
    try:
        bpm = FinalStackBackend(features={"bpm"}).analyze_track(path).bpm
        if bpm is None:
            return None

        # Octave correction
        if bpm < 70:
            bpm *= 2
        elif bpm > 200:
            bpm /= 2

        return float(bpm)
    except Exception:
        return None

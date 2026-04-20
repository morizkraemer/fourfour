"""BPM detection via DeepRhythm with octave correction."""

from __future__ import annotations

from deeprhythm import DeepRhythmAnalyzer

# Lazy singleton — DeepRhythm loads a model, expensive to reinit
_analyzer: DeepRhythmAnalyzer | None = None


def _get_analyzer() -> DeepRhythmAnalyzer:
    global _analyzer
    if _analyzer is None:
        _analyzer = DeepRhythmAnalyzer()
    return _analyzer


def detect_bpm(path: str) -> float | None:
    """Detect BPM for an audio file. Returns None on failure.

    Applies octave correction: doubles BPM < 70, halves BPM > 200.
    """
    try:
        analyzer = _get_analyzer()
        bpm = analyzer.analyze(path)

        # Octave correction
        if bpm < 70:
            bpm *= 2
        elif bpm > 200:
            bpm /= 2

        return float(bpm)
    except Exception:
        return None

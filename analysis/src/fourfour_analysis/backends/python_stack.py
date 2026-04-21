"""PythonStackBackend — DeepRhythm BPM + librosa key.

Requires optional [ml] dependencies: torch, librosa, DeepRhythm.
"""

from __future__ import annotations

from pathlib import Path
from typing import Optional

from fourfour_analysis.backends.base import AnalysisBackend
from fourfour_analysis.types import AnalysisResult, BackendMetadata


_VERSION = "0.1.0"
DEFAULT_FEATURES = frozenset({"bpm", "key", "energy"})


class PythonStackBackend(AnalysisBackend):
    """DeepRhythm BPM + librosa key + feature-based energy."""

    def __init__(self, cache_dir: Optional[Path] = None, features: Optional[set[str]] = None):
        super().__init__(cache_dir=cache_dir)
        self._features = set(features) if features is not None else set(DEFAULT_FEATURES)
        # Verify optional deps
        try:
            if "bpm" in self._features:
                import deeprhythm  # noqa: F401
            import librosa  # noqa: F401
        except ImportError as e:
            raise ImportError(
                f"PythonStackBackend requires [ml] dependencies. "
                f"Install with: pip install -e '.[ml]'. Error: {e}"
            )

    def metadata(self) -> BackendMetadata:
        feature_hash = ",".join(sorted(self._features))
        return BackendMetadata(
            id="python_deeprhythm",
            label="DeepRhythm + librosa KS",
            version=_VERSION,
            config_hash=f"v2-features:{feature_hash}",
            heavy_deps=["torch", "librosa", "DeepRhythm"],
            network_required=False,
        )

    def analyze_track(self, track_path: str) -> AnalysisResult:
        """Full analysis using Python ML stack."""
        from fourfour_analysis.audio_io import load_audio

        audio, sr = load_audio(track_path)

        # BPM via DeepRhythm
        bpm = self._detect_bpm(track_path) if "bpm" in self._features else None

        # Key via librosa chroma + Krumhansl-Schmuckler
        key = self._detect_key(audio, sr) if "key" in self._features else None

        # Energy via librosa features
        energy = None
        if "energy" in self._features:
            energy = self._compute_energy(audio, sr, bpm or 120.0)

        return AnalysisResult(
            bpm=bpm,
            key=key,
            energy=energy,
        )

    def _detect_bpm(self, track_path: str) -> Optional[float]:
        """BPM detection via DeepRhythm."""
        import warnings
        import io
        import sys

        with warnings.catch_warnings():
            warnings.simplefilter("ignore", UserWarning)
            from deeprhythm import DeepRhythmPredictor

        # DeepRhythm prints to stdout/stderr on every call
        quiet_std = io.StringIO()
        old_stdout, old_stderr = sys.stdout, sys.stderr
        sys.stdout, sys.stderr = quiet_std, quiet_std
        try:
            predictor = DeepRhythmPredictor()
            bpm = predictor.predict(track_path)
            return float(bpm) if bpm else None
        finally:
            sys.stdout, sys.stderr = old_stdout, old_stderr

    def _detect_key(self, audio, sr: int) -> Optional[str]:
        """Key detection via librosa chroma_cqt + Krumhansl-Schmuckler."""
        try:
            import librosa
            import numpy as np
            from fourfour_analysis.backends.lexicon_key import (
                MAJOR_PROFILE, MINOR_PROFILE, MAJOR_CAMELOT, MINOR_CAMELOT, NOTE_NAMES
            )

            # Compute chroma
            chroma = librosa.feature.chroma_cqt(y=audio, sr=sr)
            avg_chroma = np.mean(chroma, axis=1)

            # Krumhansl-Schmuckler
            best_key = 0
            best_is_major = True
            best_corr = -999.0

            for key_idx in range(12):
                for is_major, profile in [(True, MAJOR_PROFILE), (False, MINOR_PROFILE)]:
                    rotated = np.roll(profile, key_idx)
                    corr = np.corrcoef(avg_chroma, rotated)[0, 1]
                    if not np.isnan(corr) and corr > best_corr:
                        best_corr = corr
                        best_key = key_idx
                        best_is_major = is_major

            camelot_map = MAJOR_CAMELOT if best_is_major else MINOR_CAMELOT
            return camelot_map[best_key]
        except Exception:
            return None

    def _compute_energy(self, audio, sr: int, bpm: float) -> Optional[int]:
        """Energy rating via librosa feature fusion."""
        try:
            import librosa
            import numpy as np

            # Spectral features
            rms = float(np.mean(librosa.feature.rms(y=audio)))
            spectral_flux = float(np.mean(librosa.onset.onset_strength(y=audio, sr=sr)))
            centroid = float(np.mean(librosa.feature.spectral_centroid(y=audio, sr=sr)))

            # Normalize to 0-1
            rms_score = min(rms / 0.2, 1.0)
            flux_score = min(spectral_flux / 50.0, 1.0)
            tempo_score = max(0.0, min(1.0, (bpm - 120) / 120 + 0.5))

            score = 0.3 * rms_score + 0.4 * flux_score + 0.3 * tempo_score
            return max(1, min(10, int(round(9 * score)) + 1))
        except Exception:
            return None

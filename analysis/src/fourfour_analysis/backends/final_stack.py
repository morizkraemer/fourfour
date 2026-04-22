"""Final production analysis stack.

Combines DeepRhythm/librosa analysis with Essentia KeyExtractor bgate.
"""

from __future__ import annotations

from pathlib import Path
from typing import Optional

from fourfour_analysis.backends.base import AnalysisBackend
from fourfour_analysis.backends.essentia_key import EssentiaKeyBackend
from fourfour_analysis.backends.python_stack import PythonStackBackend
from fourfour_analysis.types import AnalysisResult, BackendMetadata


_VERSION = "0.2.0"
_PYTHON_FEATURES = {"bpm", "energy"}


class FinalStackBackend(AnalysisBackend):
    """Production analysis stack for `fourfour-analyze`."""

    def __init__(self, cache_dir: Optional[Path] = None, features: Optional[set[str]] = None):
        super().__init__(cache_dir=cache_dir)
        self._features = set(features) if features is not None else None
        python_features = _PYTHON_FEATURES if self._features is None else self._features & _PYTHON_FEATURES
        self._python = (
            PythonStackBackend(cache_dir=None, features=python_features)
            if python_features
            else None
        )
        self._essentia_key: EssentiaKeyBackend | None = None

    def metadata(self) -> BackendMetadata:
        feature_hash = "all" if self._features is None else ",".join(sorted(self._features))
        return BackendMetadata(
            id="final_stack",
            label="Final analysis stack (DeepRhythm + librosa energy + Essentia bgate key)",
            version=_VERSION,
            config_hash=f"v2-features:{feature_hash}-bpm:deeprhythm-energy:librosa-key:bgate",
            heavy_deps=["torch", "librosa", "DeepRhythm", "essentia"],
            network_required=False,
        )

    def analyze_track(self, track_path: str) -> AnalysisResult:
        """Run the production stack once for a single track."""
        python_result = self._python.analyze_track(track_path) if self._python is not None else AnalysisResult()
        key_result = None
        if self._features is None or "key" in self._features:
            if self._essentia_key is None:
                self._essentia_key = EssentiaKeyBackend(cache_dir=None, profile_type="bgate")
            key_result = self._essentia_key.analyze_track(track_path)

        return AnalysisResult(
            bpm=python_result.bpm,
            key=key_result.key if key_result is not None else None,
            energy=python_result.energy,
        )

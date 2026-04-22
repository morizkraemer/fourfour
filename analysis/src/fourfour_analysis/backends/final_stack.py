"""Final production analysis stack.

Combines Lexicon-style full analysis with Essentia KeyExtractor bgate.
"""

from __future__ import annotations

from pathlib import Path
from typing import Optional

from fourfour_analysis.backends.base import AnalysisBackend
from fourfour_analysis.backends.essentia_key import EssentiaKeyBackend
from fourfour_analysis.backends.lexicon_port import LexiconPortBackend
from fourfour_analysis.types import AnalysisResult, BackendMetadata


_VERSION = "0.1.0"


class FinalStackBackend(AnalysisBackend):
    """Production analysis stack for `fourfour-analyze`."""

    def __init__(self, cache_dir: Optional[Path] = None, features: Optional[set[str]] = None):
        super().__init__(cache_dir=cache_dir)
        self._features = set(features) if features is not None else None
        self._lexicon = LexiconPortBackend(cache_dir=None, features=features)
        self._essentia_key = EssentiaKeyBackend(cache_dir=None, profile_type="bgate")

    def metadata(self) -> BackendMetadata:
        feature_hash = "all" if self._features is None else ",".join(sorted(self._features))
        return BackendMetadata(
            id="final_stack",
            label="Final analysis stack (Lexicon + Essentia bgate key)",
            version=_VERSION,
            config_hash=f"v1-features:{feature_hash}-key:bgate",
            heavy_deps=["numpy", "scipy", "essentia"],
            network_required=False,
        )

    def analyze_track(self, track_path: str) -> AnalysisResult:
        """Run the production stack once for a single track."""
        lexicon_result = self._lexicon.analyze_track(track_path)
        key_result = self._essentia_key.analyze_track(track_path) if (
            self._features is None or "key" in self._features
        ) else None

        return AnalysisResult(
            bpm=lexicon_result.bpm,
            key=key_result.key if key_result is not None else None,
            energy=lexicon_result.energy,
            beats=lexicon_result.beats,
            waveform_peaks=lexicon_result.waveform_peaks,
            waveform_colors=lexicon_result.waveform_colors,
            cue_points=lexicon_result.cue_points,
        )

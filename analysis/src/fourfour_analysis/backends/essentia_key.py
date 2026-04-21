"""Essentia key detection backend."""

from __future__ import annotations

from pathlib import Path
from typing import Optional

from fourfour_analysis.backends.base import AnalysisBackend
from fourfour_analysis.groundtruth import normalize_key
from fourfour_analysis.types import AnalysisResult, BackendMetadata


_VERSION = "0.1.0"
_SAMPLE_RATE = 44_100


class EssentiaKeyBackend(AnalysisBackend):
    """Key detection via Essentia's HPCP/profile pipeline."""

    def __init__(self, cache_dir: Optional[Path] = None, profile_type: str = "bgate"):
        super().__init__(cache_dir=cache_dir)
        self._profile_type = profile_type
        try:
            import essentia.standard as es  # noqa: F401
        except ImportError as e:
            raise ImportError(
                "EssentiaKeyBackend requires essentia. "
                "Install with: uv pip install --python .venv/bin/python essentia"
            ) from e

    def metadata(self) -> BackendMetadata:
        return BackendMetadata(
            id=f"essentia_key_{self._profile_type}",
            label=f"Essentia KeyExtractor ({self._profile_type})",
            version=_VERSION,
            config_hash=f"v1-profile:{self._profile_type}-sr:{_SAMPLE_RATE}",
            heavy_deps=["essentia"],
            network_required=False,
        )

    def analyze_track(self, track_path: str) -> AnalysisResult:
        import essentia.standard as es

        audio = es.MonoLoader(filename=track_path, sampleRate=_SAMPLE_RATE)()
        key, scale, _strength = es.KeyExtractor(
            profileType=self._profile_type,
            sampleRate=_SAMPLE_RATE,
        )(audio)

        camelot = normalize_key(f"{key} {scale}")
        if camelot is None:
            raise RuntimeError(f"Essentia returned unparseable key: {key} {scale}")

        return AnalysisResult(key=camelot)

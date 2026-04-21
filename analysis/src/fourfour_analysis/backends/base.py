"""Abstract base class for analysis backends."""

from __future__ import annotations

import time
from abc import ABC, abstractmethod
from pathlib import Path
from typing import Optional

from fourfour_analysis.types import AnalysisResult, AnalysisRecord, BackendMetadata, TrackEntry
from fourfour_analysis.cache import cache_key, load_cache, save_cache


class AnalysisBackend(ABC):
    """Base class for all analysis backends."""

    def __init__(self, cache_dir: Optional[Path] = None):
        self._cache_dir = cache_dir

    @abstractmethod
    def metadata(self) -> BackendMetadata:
        """Return backend metadata (id, version, etc.)."""
        ...

    @abstractmethod
    def analyze_track(self, track_path: str) -> AnalysisResult:
        """Analyze a single audio file.

        Args:
            track_path: Path to the audio file.

        Returns:
            AnalysisResult with detected features.
        """
        ...

    def analyze_track_cached(self, track: TrackEntry) -> AnalysisRecord:
        """Analyze a track, using cache if available.

        Args:
            track: TrackEntry with path and fingerprint.

        Returns:
            AnalysisRecord with status and result/error.
        """
        meta = self.metadata()
        key = cache_key(track.content_fingerprint, meta.id, meta.config_hash)

        # Check cache
        if self._cache_dir is not None:
            cached = load_cache(self._cache_dir, key)
            if cached is not None:
                return cached

        # Analyze
        start = time.monotonic()
        try:
            result = self.analyze_track(track.path)
            elapsed = time.monotonic() - start
            # Attach timing and metadata
            from fourfour_analysis.types import AnalysisResult as AR
            result = AR(
                bpm=result.bpm,
                key=result.key,
                energy=result.energy,
                beats=result.beats,
                waveform_peaks=result.waveform_peaks,
                waveform_colors=result.waveform_colors,
                cue_points=result.cue_points,
                elapsed_seconds=elapsed,
                backend_metadata=meta,
            )
            record = AnalysisRecord(
                track_id=track.id,
                backend_id=meta.id,
                status="ok",
                result=result,
            )
        except Exception as e:
            record = AnalysisRecord(
                track_id=track.id,
                backend_id=meta.id,
                status="failed",
                error=str(e),
            )

        # Save to cache
        if self._cache_dir is not None:
            save_cache(self._cache_dir, key, record)

        return record

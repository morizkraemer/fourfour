"""Shared type definitions — frozen dataclasses, no logic."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Optional


@dataclass(frozen=True)
class BackendMetadata:
    """Describes an analysis backend."""
    id: str
    label: str
    version: str
    config_hash: str
    heavy_deps: list[str] = field(default_factory=list)
    network_required: bool = False


@dataclass(frozen=True)
class WaveformPeak:
    """Single column of a waveform display: min/max deviation from center."""
    min_val: float
    max_val: float


@dataclass(frozen=True)
class WaveformColor:
    """RGB color for a waveform column (0-255 per channel)."""
    r: int
    g: int
    b: int


@dataclass(frozen=True)
class BeatPosition:
    """A single beat in the beat grid."""
    time_seconds: float
    bar_position: int  # 1-4, where 1 = downbeat


@dataclass(frozen=True)
class CuePoint:
    """A detected cue or loop point."""
    label: str
    time_seconds: float
    loop_end_seconds: Optional[float] = None


@dataclass(frozen=True)
class AnalysisResult:
    """Full analysis output for a single track."""
    bpm: Optional[float] = None
    key: Optional[str] = None  # Camelot notation, e.g. "8A"
    energy: Optional[int] = None  # 1-10
    beats: list[BeatPosition] = field(default_factory=list)
    waveform_peaks: list[WaveformPeak] = field(default_factory=list)
    waveform_colors: list[WaveformColor] = field(default_factory=list)
    cue_points: list[CuePoint] = field(default_factory=list)
    elapsed_seconds: float = 0.0
    backend_metadata: Optional[BackendMetadata] = None


@dataclass(frozen=True)
class AnalysisRecord:
    """Analysis result for a specific track + backend, including error info."""
    track_id: str
    backend_id: str
    status: str  # "ok" | "failed" | "timeout"
    result: Optional[AnalysisResult] = None
    error: Optional[str] = None


@dataclass(frozen=True)
class GroundTruth:
    """Known-good values extracted from file tags or manual annotation."""
    track_id: str
    bpm: Optional[float] = None
    key: Optional[str] = None  # Camelot notation
    energy: Optional[int] = None
    bpm_source: Optional[str] = None  # "tag" | "manual"
    key_source: Optional[str] = None


@dataclass(frozen=True)
class TrackEntry:
    """A track in the benchmark corpus."""
    id: str
    path: str
    content_fingerprint: str  # SHA256 of first+last 64KB + file size
    artist: str = ""
    title: str = ""
    genre: str = ""
    duration_seconds: Optional[float] = None
    ground_truth: Optional[GroundTruth] = None


@dataclass(frozen=True)
class TempoComparison:
    bpm_delta: float
    within_1pct: bool
    within_4pct: bool
    octave_error: bool


@dataclass(frozen=True)
class KeyComparison:
    exact: bool
    error_type: str  # "exact" | "relative" | "parallel" | "fifth" | "other"


@dataclass(frozen=True)
class TrackComparison:
    """Comparison of one backend's output against ground truth for one track."""
    track_id: str
    backend_id: str
    tempo: Optional[TempoComparison] = None
    key: Optional[KeyComparison] = None
    energy_delta: Optional[int] = None

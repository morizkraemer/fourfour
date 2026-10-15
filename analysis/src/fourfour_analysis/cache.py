"""Content-addressed JSON cache for analysis results."""

from __future__ import annotations

import base64
import hashlib
import json
from pathlib import Path
from typing import Optional

from fourfour_analysis.types import AnalysisRecord


def cache_key(content_fingerprint: str, backend_id: str, config_hash: str) -> str:
    """Compute cache key from content fingerprint + backend + config.

    Args:
        content_fingerprint: SHA256 of file content.
        backend_id: Backend identifier, e.g. "lexicon_port".
        config_hash: Hash of backend configuration.

    Returns:
        24-char hex string.
    """
    raw = f"{content_fingerprint}:{backend_id}:{config_hash}"
    return hashlib.sha1(raw.encode()).hexdigest()[:24]


def _record_to_dict(record: AnalysisRecord) -> dict:
    """Serialize AnalysisRecord to a JSON-compatible dict."""
    from dataclasses import asdict

    d = asdict(record)
    if d.get("result"):
        d["result"]["waveform_preview"] = base64.b64encode(record.result.waveform_preview).decode("ascii")
    return d


def _dict_to_record(d: dict) -> AnalysisRecord:
    """Deserialize dict back to AnalysisRecord."""
    from fourfour_analysis.types import (
        AnalysisResult,
        AnalysisRecord,
        BeatPosition,
        CuePoint,
        WaveformPeak,
        WaveformColor,
        BackendMetadata,
    )

    # Reconstruct nested types
    result = None
    if d.get("result"):
        r = d["result"]
        beats = [BeatPosition(**b) for b in r.get("beats", [])]
        peaks = [WaveformPeak(**p) for p in r.get("waveform_peaks", [])]
        colors = [WaveformColor(**c) for c in r.get("waveform_colors", [])]
        overview = [WaveformColor(**c) for c in r.get("waveform_overview", [])]
        preview = base64.b64decode(r.get("waveform_preview", ""), validate=True)
        cues = [CuePoint(**c) for c in r.get("cue_points", [])]
        meta = BackendMetadata(**r["backend_metadata"]) if r.get("backend_metadata") else None
        result = AnalysisResult(
            bpm=r.get("bpm"),
            key=r.get("key"),
            energy=r.get("energy"),
            beats=beats,
            waveform_peaks=peaks,
            waveform_colors=colors,
            waveform_overview=overview,
            waveform_preview=preview,
            waveform_fft_bands=r.get("waveform_fft_bands", []),
            cue_points=cues,
            elapsed_seconds=r.get("elapsed_seconds", 0.0),
            backend_metadata=meta,
        )

    return AnalysisRecord(
        track_id=d["track_id"],
        backend_id=d["backend_id"],
        status=d["status"],
        result=result,
        error=d.get("error"),
    )


def load_cache(cache_dir: Path, key: str) -> Optional[AnalysisRecord]:
    """Load cached analysis result.

    Args:
        cache_dir: Directory containing cache files.
        key: 24-char hex cache key.

    Returns:
        AnalysisRecord if cache hit, None if miss.
    """
    cache_file = cache_dir / f"{key}.json"
    if not cache_file.is_file():
        return None

    try:
        data = json.loads(cache_file.read_text())
        return _dict_to_record(data)
    except (json.JSONDecodeError, KeyError, TypeError, ValueError):
        return None


def save_cache(cache_dir: Path, key: str, record: AnalysisRecord) -> None:
    """Save analysis result to cache.

    Args:
        cache_dir: Directory containing cache files.
        key: 24-char hex cache key.
        record: The analysis record to cache.
    """
    cache_dir.mkdir(parents=True, exist_ok=True)
    cache_file = cache_dir / f"{key}.json"
    cache_file.write_text(json.dumps(_record_to_dict(record), indent=2))

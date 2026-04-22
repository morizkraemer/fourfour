"""StratumDspBackend — wraps the Rust stratum-dsp subprocess.

Requires building the stratum-cli binary in the Rust workspace.
"""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Optional

from fourfour_analysis.backends.base import AnalysisBackend
from fourfour_analysis.types import (
    AnalysisResult,
    BackendMetadata,
    BeatPosition,
    CuePoint,
)


_VERSION = "0.1.0"


class StratumDspBackend(AnalysisBackend):
    """stratum-dsp analysis via Rust subprocess."""

    def __init__(self, cache_dir: Optional[Path] = None, settings=None):
        super().__init__(cache_dir=cache_dir)
        self._settings = settings
        self._binary_path = self._find_binary()

    def _find_binary(self) -> Optional[str]:
        """Find the stratum-cli binary."""
        if self._settings is not None:
            # Look in target/debug/ relative to project root
            candidate = self._settings.root_dir / "target" / "debug" / "stratum-cli"
            if candidate.is_file():
                return str(candidate)
            candidate = self._settings.root_dir / "target" / "release" / "stratum-cli"
            if candidate.is_file():
                return str(candidate)
        return None

    def metadata(self) -> BackendMetadata:
        return BackendMetadata(
            id="stratum_dsp",
            label="stratum-dsp (Rust subprocess)",
            version=_VERSION,
            config_hash="v1",
            heavy_deps=[],
            network_required=False,
        )

    def analyze_track(self, track_path: str) -> AnalysisResult:
        """Analyze via stratum-cli subprocess."""
        if self._binary_path is None:
            raise RuntimeError(
                "stratum-cli binary not found. Build it with: "
                "cd fourfour && cargo build -p stratum-cli"
            )

        result = subprocess.run(
            [self._binary_path, track_path],
            capture_output=True,
            text=True,
            timeout=120,
        )

        if result.returncode != 0:
            raise RuntimeError(f"stratum-cli failed: {result.stderr}")

        data = json.loads(result.stdout)

        beats = []
        for b in data.get("beats", []):
            beats.append(BeatPosition(
                time_seconds=b.get("time", 0.0),
                bar_position=b.get("bar_position", 1),
            ))

        return AnalysisResult(
            bpm=data.get("bpm"),
            key=data.get("key"),
            beats=beats,
        )

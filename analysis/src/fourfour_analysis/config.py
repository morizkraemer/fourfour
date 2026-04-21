"""Project configuration — path resolution."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path


def _find_project_root(start: Path | None = None) -> Path:
    """Walk up from start (or cwd) to find the directory containing Cargo.toml."""
    current = start or Path.cwd()
    for parent in [current] + list(current.parents):
        if (parent / "Cargo.toml").is_file():
            return parent
    raise FileNotFoundError(
        f"Could not find project root (Cargo.toml) starting from {current}"
    )


@dataclass(frozen=True)
class Settings:
    """Resolved paths for the fourfour project."""
    root_dir: Path

    @classmethod
    def from_cwd(cls, start: Path | None = None) -> Settings:
        return cls(root_dir=_find_project_root(start))

    @property
    def benchmark_dir(self) -> Path:
        return self.root_dir / "benchmark"

    @property
    def manifests_dir(self) -> Path:
        return self.benchmark_dir / "manifests"

    @property
    def results_dir(self) -> Path:
        return self.benchmark_dir / "results"

    @property
    def cache_dir(self) -> Path:
        return self.benchmark_dir / "cache"

    def ensure_dirs(self) -> None:
        """Create all benchmark directories if they don't exist."""
        for d in [self.benchmark_dir, self.manifests_dir, self.results_dir, self.cache_dir]:
            d.mkdir(parents=True, exist_ok=True)

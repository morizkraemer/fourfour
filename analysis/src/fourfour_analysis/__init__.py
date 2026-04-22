"""fourfour-analysis: Audio analysis CLI for Pioneer DJ USB drives."""

__version__ = "0.1.0"


def analyze_track(path: str) -> dict:
    """Analyze one track with the compatibility orchestrator."""
    from fourfour_analysis.analyze import analyze_track as _analyze_track

    return _analyze_track(path)


def analyze_batch(paths: list[str], workers: int = 4) -> list[dict]:
    """Analyze many tracks with the compatibility orchestrator."""
    from fourfour_analysis.analyze import analyze_batch as _analyze_batch

    return _analyze_batch(paths, workers=workers)


__all__ = ["__version__", "analyze_track", "analyze_batch"]

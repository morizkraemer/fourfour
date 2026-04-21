"""Backend registry — variant definitions and factory."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from fourfour_analysis.backends.base import AnalysisBackend
    from fourfour_analysis.config import Settings


# Variant definitions
ANALYSIS_VARIANTS = {
    "lexicon_port": {
        "backend": "lexicon_port",
        "label": "Lexicon algorithms (Python port)",
        "heavy_deps": ["numpy", "scipy"],
    },
    "python_deeprhythm": {
        "backend": "python_stack",
        "label": "DeepRhythm + librosa KS",
        "heavy_deps": ["torch", "librosa", "DeepRhythm"],
    },
    "stratum_dsp": {
        "backend": "stratum_dsp",
        "label": "stratum-dsp (Rust subprocess)",
        "heavy_deps": [],
    },
}


def load_backend(
    variant_id: str,
    settings: "Settings",
    features: set[str] | None = None,
) -> "AnalysisBackend":
    """Instantiate a backend by variant ID.

    Args:
        variant_id: One of the keys in ANALYSIS_VARIANTS.
        settings: Project settings (for cache dir, etc.).

    Returns:
        AnalysisBackend instance.

    Raises:
        ValueError: Unknown variant.
        ImportError: Missing optional dependencies.
    """
    if variant_id not in ANALYSIS_VARIANTS:
        raise ValueError(
            f"Unknown variant '{variant_id}'. "
            f"Available: {list(ANALYSIS_VARIANTS.keys())}"
        )

    cache_dir = settings.cache_dir
    backend_key = ANALYSIS_VARIANTS[variant_id]["backend"]

    if backend_key == "lexicon_port":
        from fourfour_analysis.backends.lexicon_port import LexiconPortBackend
        return LexiconPortBackend(cache_dir=cache_dir, features=features)

    elif backend_key == "python_stack":
        from fourfour_analysis.backends.python_stack import PythonStackBackend
        return PythonStackBackend(cache_dir=cache_dir, features=features)

    elif backend_key == "stratum_dsp":
        from fourfour_analysis.backends.stratum_dsp import StratumDspBackend
        return StratumDspBackend(cache_dir=cache_dir, settings=settings)

    else:
        raise ValueError(f"Unknown backend key: {backend_key}")

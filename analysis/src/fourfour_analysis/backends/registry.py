"""Backend registry — variant definitions and factory."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from fourfour_analysis.backends.base import AnalysisBackend
    from fourfour_analysis.config import Settings


# Variant definitions
ANALYSIS_VARIANTS = {
    "deeprhythm_essentia": {
        "backend": "deeprhythm_essentia",
        "label": "DeepRhythm BPM + Essentia key (bgate) + Lexicon waveform/beats/cues",
        "heavy_deps": ["torch", "DeepRhythm", "essentia", "numpy", "scipy"],
    },
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
    "essentia_key_bgate": {
        "backend": "essentia_key",
        "label": "Essentia KeyExtractor bgate",
        "profile_type": "bgate",
        "heavy_deps": ["essentia"],
    },
    "final_stack": {
        "backend": "final_stack",
        "label": "Final analysis stack (DeepRhythm + librosa energy + Essentia bgate key)",
        "heavy_deps": ["torch", "librosa", "DeepRhythm", "essentia"],
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

    if backend_key == "deeprhythm_essentia":
        from fourfour_analysis.backends.deeprhythm_essentia import DeepRhythmEssentiaBackend
        return DeepRhythmEssentiaBackend(cache_dir=cache_dir, features=features)

    elif backend_key == "lexicon_port":
        from fourfour_analysis.backends.lexicon_port import LexiconPortBackend
        return LexiconPortBackend(cache_dir=cache_dir, features=features)

    elif backend_key == "python_stack":
        from fourfour_analysis.backends.python_stack import PythonStackBackend
        return PythonStackBackend(cache_dir=cache_dir, features=features)

    elif backend_key == "stratum_dsp":
        from fourfour_analysis.backends.stratum_dsp import StratumDspBackend
        return StratumDspBackend(cache_dir=cache_dir, settings=settings)

    elif backend_key == "essentia_key":
        from fourfour_analysis.backends.essentia_key import EssentiaKeyBackend
        profile_type = ANALYSIS_VARIANTS[variant_id]["profile_type"]
        return EssentiaKeyBackend(cache_dir=cache_dir, profile_type=profile_type)

    elif backend_key == "final_stack":
        from fourfour_analysis.backends.final_stack import FinalStackBackend
        return FinalStackBackend(cache_dir=cache_dir, features=features)

    else:
        raise ValueError(f"Unknown backend key: {backend_key}")

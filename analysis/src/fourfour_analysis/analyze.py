"""Orchestrator: runs all extractors on a track and returns unified JSON."""

from dataclasses import asdict
import multiprocessing
import time

from fourfour_analysis.backends.final_stack import FinalStackBackend
from fourfour_analysis.waveform import extract_peaks, extract_color_bands, generate_pwav_preview


def _energy_label(score: int | None) -> str | None:
    if score is None:
        return None
    if score <= 3:
        return "low"
    if score <= 6:
        return "medium"
    return "high"


def analyze_track(path: str) -> dict:
    """Run all analysis on a single track.

    Returns a dict with:
        path, bpm, key, energy, waveform_preview, waveform_color,
        waveform_peaks, errors, elapsed_seconds
    """
    result = {"path": path, "errors": []}
    start = time.time()

    try:
        analysis = FinalStackBackend(features={"bpm", "key", "energy", "cues"}).analyze_track(path)
        result["bpm"] = analysis.bpm
        result["key"] = analysis.key
        result["energy"] = (
            {"score": analysis.energy, "label": _energy_label(analysis.energy)}
            if analysis.energy is not None
            else None
        )
        result["beats"] = [asdict(beat) for beat in analysis.beats]
        result["cue_points"] = [asdict(cue) for cue in analysis.cue_points]
        if analysis.bpm is None:
            result["errors"].append("bpm: detection failed")
        if analysis.key is None:
            result["errors"].append("key: detection failed")
        if analysis.energy is None:
            result["errors"].append("energy: detection failed or track too short")
    except Exception as e:
        result["bpm"] = None
        result["key"] = None
        result["energy"] = None
        result["beats"] = []
        result["cue_points"] = []
        result["errors"].append(f"analysis: {e}")

    # Waveform preview (400 bytes, Pioneer PWAV format)
    try:
        preview = generate_pwav_preview(path)
        result["waveform_preview"] = list(preview)
    except Exception as e:
        result["waveform_preview"] = [0] * 400
        result["errors"].append(f"waveform_preview: {e}")

    # Color waveform (2000 points, RGB)
    try:
        result["waveform_color"] = extract_color_bands(path, points=2000)
    except Exception as e:
        result["waveform_color"] = []
        result["errors"].append(f"waveform_color: {e}")

    # Waveform peaks (2000 points, min/max)
    try:
        result["waveform_peaks"] = extract_peaks(path, target_points=2000)
    except Exception as e:
        result["waveform_peaks"] = []
        result["errors"].append(f"waveform_peaks: {e}")

    # Pioneer 3-band waveform (native resolution for ANLZ files)
    try:
        from fourfour_analysis.waveform import generate_pioneer_3band
        pioneer_bands = generate_pioneer_3band(path)
        result["pioneer_3band_detail"] = pioneer_bands["detail"]
        result["pioneer_3band_overview"] = pioneer_bands["overview"]
    except Exception as e:
        result["pioneer_3band_detail"] = []
        result["pioneer_3band_overview"] = []
        result["errors"].append(f"pioneer_3band: {e}")

    result["elapsed_seconds"] = time.time() - start
    return result


def analyze_batch(paths: list[str], workers: int = 4) -> list[dict]:
    """Analyze multiple tracks in parallel.

    Uses multiprocessing to parallelize across CPU cores.
    Falls back to sequential processing when workers <= 1.
    """
    if workers <= 1:
        return [analyze_track(p) for p in paths]
    ctx = multiprocessing.get_context("spawn")
    with ctx.Pool(processes=workers) as pool:
        results = pool.map(analyze_track, paths)
    return results

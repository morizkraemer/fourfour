"""Orchestrator: runs all extractors on a track and returns unified JSON."""

import multiprocessing
import time
from pathlib import Path

from fourfour_analysis.bpm import detect_bpm
from fourfour_analysis.key import detect_key
from fourfour_analysis.energy import compute_energy
from fourfour_analysis.waveform import extract_peaks, extract_color_bands, generate_pwav_preview


def analyze_track(path: str) -> dict:
    """Run all analysis on a single track.

    Returns a dict with:
        path, bpm, key, energy, waveform_preview, waveform_color,
        waveform_peaks, errors, elapsed_seconds
    """
    result = {"path": path, "errors": []}
    start = time.time()

    # BPM
    bpm = detect_bpm(path)
    if bpm is not None:
        result["bpm"] = bpm
    else:
        result["bpm"] = None
        result["errors"].append("bpm: detection failed")

    # Key
    key = detect_key(path)
    if key is not None:
        result["key"] = key
    else:
        result["key"] = None
        result["errors"].append("key: detection failed")

    # Energy
    energy = compute_energy(path)
    result["energy"] = energy
    if energy is None:
        result["errors"].append("energy: detection failed or track too short")

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
    ctx = multiprocessing.get_context("fork")
    with ctx.Pool(processes=workers) as pool:
        results = pool.map(analyze_track, paths)
    return results

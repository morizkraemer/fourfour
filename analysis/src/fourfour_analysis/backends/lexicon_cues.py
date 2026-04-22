"""Lexicon cue point detection — energy phrase segmentation.

Port of Lexicon's Worker 182 section detection.
Reference: docs/lexicon-wiki.md §7

Algorithm:
  1. Per bar (4 beats at BPM): RMS energy, beat strength, ramp type
  2. Split at mean energy → high/low sections
  3. Filter: min section = 64 beats, round to 4-bar boundaries
  4. Assign labels: Start, Drop, Breakdown, SecondDrop, Lastbeat
  5. Emergency loop: find 16-beat stable section before last drop
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Optional

import numpy as np

from fourfour_analysis.types import CuePoint


# ── Constants ─────────────────────────────────────────────

BREAKDOWN_MIN_BEATS = 64   # minimum section length in beats
BEATS_PER_BAR = 4
BARS_PER_SECTION = 4       # round sections to 4-bar boundaries


@dataclass
class _BarAnalysis:
    """Analysis of a single bar (4 beats)."""
    start_sec: float
    duration_sec: float
    rms_energy: float
    beat_strength: float
    ramp_type: str  # "up", "down", "flat"


def detect_sections(
    beats: list[float],
    audio: np.ndarray,
    sr: int,
    bpm: float,
) -> list[CuePoint]:
    """Detect section cue points from beat positions and audio.

    Args:
        beats: Beat positions in seconds.
        audio: Mono f32 audio (lowpass-filtered preferred).
        sr: Sample rate.
        bpm: Detected BPM.

    Returns:
        List of CuePoint with section labels.
    """
    if len(beats) < BREAKDOWN_MIN_BEATS or bpm <= 0:
        return [CuePoint(label="Start", time_seconds=0.0)]

    # Step 1: Analyze each bar
    bars = _analyze_bars(beats, audio, sr, bpm)
    if len(bars) == 0:
        return [CuePoint(label="Start", time_seconds=0.0)]

    # Step 2: Segment into high/low energy sections
    sections = _segment_bars(bars)

    # Step 3: Filter short sections
    sections = _filter_sections(sections)

    # Step 4: Assign markers
    cues = _assign_markers(sections, bars)

    # Step 5: Emergency loop (optional, skip for now)
    # TODO: implement emergency loop detection

    return cues


def _analyze_bars(
    beats: list[float],
    audio: np.ndarray,
    sr: int,
    bpm: float,
) -> list[_BarAnalysis]:
    """Analyze each bar: RMS energy, beat strength, ramp type."""
    bars: list[_BarAnalysis] = []

    # Group beats into bars (every 4 beats)
    num_bars = len(beats) // BEATS_PER_BAR

    for bar_idx in range(num_bars):
        beat_start_idx = bar_idx * BEATS_PER_BAR
        beat_end_idx = beat_start_idx + BEATS_PER_BAR

        if beat_end_idx >= len(beats):
            break

        start_sec = beats[beat_start_idx]
        end_sec = beats[min(beat_end_idx, len(beats) - 1)]
        duration = end_sec - start_sec

        if duration <= 0:
            continue

        # RMS energy over the bar
        s_start = int(start_sec * sr)
        s_end = int(end_sec * sr)
        s_start = max(0, min(s_start, len(audio)))
        s_end = max(0, min(s_end, len(audio)))

        if s_end <= s_start:
            continue

        bar_audio = audio[s_start:s_end]
        rms_energy = float(np.sqrt(np.mean(bar_audio ** 2)))

        # Beat strength: average RMS at each beat position (±50ms window)
        beat_strengths = []
        for b in range(beat_start_idx, min(beat_end_idx, len(beats))):
            beat_sec = beats[b]
            w_start = int((beat_sec - 0.05) * sr)
            w_end = int((beat_sec + 0.05) * sr)
            w_start = max(0, min(w_start, len(audio)))
            w_end = max(0, min(w_end, len(audio)))
            if w_end > w_start:
                window = audio[w_start:w_end]
                beat_strengths.append(float(np.sqrt(np.mean(window ** 2))))

        avg_beat_strength = float(np.mean(beat_strengths)) if beat_strengths else 0.0

        # Ramp type: linear regression slope of beat strengths
        if len(beat_strengths) >= 2:
            x = np.arange(len(beat_strengths), dtype=float)
            y = np.array(beat_strengths)
            slope = float(np.polyfit(x, y, 1)[0])
            mean_val = float(np.mean(y)) if np.mean(y) > 0 else 1e-10
            threshold = mean_val * 0.3
            if slope > threshold:
                ramp_type = "up"
            elif slope < -threshold:
                ramp_type = "down"
            else:
                ramp_type = "flat"
        else:
            ramp_type = "flat"

        bars.append(_BarAnalysis(
            start_sec=start_sec,
            duration_sec=duration,
            rms_energy=rms_energy,
            beat_strength=avg_beat_strength,
            ramp_type=ramp_type,
        ))

    return bars


def _segment_bars(bars: list[_BarAnalysis]) -> list[dict]:
    """Split bars into high/low energy sections based on mean energy."""
    if len(bars) == 0:
        return []

    energies = np.array([b.rms_energy for b in bars])
    mean_energy = float(np.mean(energies))

    # Create sections: consecutive bars of same type (high/low)
    sections = []
    current_type = "high" if bars[0].rms_energy >= mean_energy else "low"
    section_start = 0

    for i in range(1, len(bars)):
        bar_type = "high" if bars[i].rms_energy >= mean_energy else "low"
        if bar_type != current_type:
            sections.append({
                "type": current_type,
                "start_bar": section_start,
                "end_bar": i,
                "num_beats": (i - section_start) * BEATS_PER_BAR,
                "start_sec": bars[section_start].start_sec,
                "end_sec": bars[i - 1].start_sec + bars[i - 1].duration_sec,
            })
            current_type = bar_type
            section_start = i

    # Final section
    sections.append({
        "type": current_type,
        "start_bar": section_start,
        "end_bar": len(bars),
        "num_beats": (len(bars) - section_start) * BEATS_PER_BAR,
        "start_sec": bars[section_start].start_sec,
        "end_sec": bars[-1].start_sec + bars[-1].duration_sec,
    })

    return sections


def _filter_sections(sections: list[dict]) -> list[dict]:
    """Remove sections shorter than BREAKDOWN_MIN_BEATS, round to 4-bar boundaries."""
    # Merge short sections into neighbors
    filtered = []
    for section in sections:
        if section["num_beats"] >= BREAKDOWN_MIN_BEATS:
            # Round to 4-bar boundaries
            num_bars = section["end_bar"] - section["start_bar"]
            rounded_bars = max(BARS_PER_SECTION, (num_bars // BARS_PER_SECTION) * BARS_PER_SECTION)
            section["end_bar"] = section["start_bar"] + rounded_bars
            section["num_beats"] = rounded_bars * BEATS_PER_BAR
            filtered.append(section)
        elif len(filtered) > 0:
            # Merge into previous section
            filtered[-1]["end_bar"] = section["end_bar"]
            filtered[-1]["num_beats"] = (filtered[-1]["end_bar"] - filtered[-1]["start_bar"]) * BEATS_PER_BAR

    return filtered


def _assign_markers(sections: list[dict], bars: list[_BarAnalysis]) -> list[CuePoint]:
    """Assign cue point labels to detected sections."""
    from fourfour_analysis.types import CuePoint

    cues = [CuePoint(label="Start", time_seconds=0.0)]

    if len(sections) == 0:
        return cues

    # Find high and low energy sections
    high_sections = [s for s in sections if s["type"] == "high"]
    low_sections = [s for s in sections if s["type"] == "low"]

    # Remove high-energy sections in intro region (first 64 beats = 16 bars)
    high_sections = [s for s in high_sections if s["start_bar"] >= 16]

    # Assign markers
    if len(high_sections) >= 1:
        cues.append(CuePoint(label="Drop", time_seconds=high_sections[0]["start_sec"]))

    if len(low_sections) >= 1:
        # First breakdown after first drop
        breakdowns_after_drop = [
            s for s in low_sections
            if s["start_sec"] > (high_sections[0]["start_sec"] if high_sections else 0)
        ]
        if breakdowns_after_drop:
            cues.append(CuePoint(label="Breakdown", time_seconds=breakdowns_after_drop[0]["start_sec"]))

    if len(high_sections) >= 2:
        cues.append(CuePoint(label="SecondDrop", time_seconds=high_sections[1]["start_sec"]))

    # Lastbeat: end of last high-energy section
    if high_sections:
        last = high_sections[-1]
        cues.append(CuePoint(label="Lastbeat", time_seconds=last["end_sec"]))

    return cues

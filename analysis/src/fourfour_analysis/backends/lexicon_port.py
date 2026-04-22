"""LexiconPortBackend — wires all Lexicon algorithm modules together."""

from __future__ import annotations

from pathlib import Path
from typing import Optional

from fourfour_analysis.backends.base import AnalysisBackend
from fourfour_analysis.backends.lexicon_bpm import analyze_tempo
from fourfour_analysis.backends.lexicon_key import detect_key
from fourfour_analysis.backends.lexicon_energy import compute_energy
from fourfour_analysis.backends.lexicon_waveform import generate_waveform
from fourfour_analysis.backends.lexicon_cues import detect_sections
from fourfour_analysis.audio_io import load_audio, preprocess_tempo, preprocess_key, preprocess_waveform
from fourfour_analysis.types import (
    AnalysisResult,
    BackendMetadata,
    BeatPosition,
    CuePoint,
    WaveformPeak,
    WaveformColor,
)


_VERSION = "0.1.0"
DEFAULT_FEATURES = frozenset({"bpm", "key", "energy", "waveform", "cues"})


class LexiconPortBackend(AnalysisBackend):
    """All Lexicon algorithms ported to Python."""

    def __init__(self, cache_dir: Optional[Path] = None, features: Optional[set[str]] = None):
        super().__init__(cache_dir=cache_dir)
        self._features = set(features) if features is not None else set(DEFAULT_FEATURES)

    def metadata(self) -> BackendMetadata:
        feature_hash = ",".join(sorted(self._features))
        return BackendMetadata(
            id="lexicon_port",
            label="Lexicon algorithms (Python port)",
            version=_VERSION,
            config_hash=f"v2-features:{feature_hash}",
            heavy_deps=["numpy", "scipy"],
            network_required=False,
        )

    def analyze_track(self, track_path: str) -> AnalysisResult:
        """Full analysis: BPM, key, energy, waveform, cue points."""
        # Load audio
        audio, sr = load_audio(track_path)

        needs_tempo = bool(self._features & {"bpm", "energy", "cues"})
        needs_key = "key" in self._features
        needs_waveform = "waveform" in self._features

        # BPM detection
        tempo_audio = None
        bpm = None
        beats = []
        if needs_tempo:
            tempo_audio, _ = preprocess_tempo(audio, sr)
            bpm_result = analyze_tempo(tempo_audio, sr)
            bpm = bpm_result.bpm if bpm_result else None
            beats = bpm_result.beats if bpm_result else []

        # Key detection
        key = None
        if needs_key:
            key_audio, key_sr = preprocess_key(audio, sr)
            key_result = detect_key(key_audio, key_sr)
            key = key_result.camelot if key_result else None

        # Energy rating
        energy = None
        if "energy" in self._features and bpm is not None and tempo_audio is not None:
            energy = compute_energy(tempo_audio, sr, bpm)

        # Waveform
        peaks = []
        colors = []
        if needs_waveform:
            waveform_audio, waveform_sr = preprocess_waveform(audio, sr)
            waveform_columns = generate_waveform(waveform_audio, waveform_sr)
            peaks = [WaveformPeak(min_val=c.min_val, max_val=c.max_val) for c in waveform_columns]
            colors = [WaveformColor(r=c.r, g=c.g, b=c.b) for c in waveform_columns]

        # Cue points
        cue_points = []
        if "cues" in self._features and bpm is not None and len(beats) > 0 and tempo_audio is not None:
            cue_points = detect_sections(beats, tempo_audio, sr, bpm)

        # Convert beats to BeatPosition
        beat_positions = []
        for i, t in enumerate(beats):
            beat_positions.append(BeatPosition(
                time_seconds=t,
                bar_position=(i % 4) + 1,
            ))

        return AnalysisResult(
            bpm=bpm,
            key=key,
            energy=energy,
            beats=beat_positions,
            waveform_peaks=peaks,
            waveform_colors=colors,
            cue_points=cue_points,
        )

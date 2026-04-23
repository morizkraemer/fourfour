"""DeepRhythmEssentiaBackend — DeepRhythm BPM + Essentia key + Lexicon everything else.

Pipeline:
  BPM:      DeepRhythm (torch) — best-in-class ML tempo detection
  Beats:    Lexicon beat generator seeded with DeepRhythm BPM
  Key:      Essentia KeyExtractor bgate — beats Rekordbox (54% vs 47% exact)
  Waveform: Lexicon FFT-based 3-band color waveform
  Cues:     Lexicon section detector
  Energy:   Lexicon RMS/onset feature fusion

Requires: torch, DeepRhythm, essentia (plus numpy/scipy for Lexicon modules)
"""

from __future__ import annotations

from pathlib import Path
from typing import Optional

from fourfour_analysis.backends.base import AnalysisBackend
from fourfour_analysis.types import (
    AnalysisResult,
    BackendMetadata,
    BeatPosition,
    WaveformPeak,
    WaveformColor,
)


_VERSION = "0.1.0"
DEFAULT_FEATURES = frozenset({"bpm", "key", "energy", "waveform", "cues"})


class DeepRhythmEssentiaBackend(AnalysisBackend):
    """DeepRhythm BPM + Essentia key + Lexicon beats/waveform/cues/energy."""

    def __init__(self, cache_dir: Optional[Path] = None, features: Optional[set[str]] = None):
        super().__init__(cache_dir=cache_dir)
        self._features = set(features) if features is not None else set(DEFAULT_FEATURES)

        # Verify Essentia is available at init time (lightweight check, no model loading)
        if "key" in self._features:
            try:
                import essentia.standard  # noqa: F401
            except ImportError as e:
                raise ImportError(
                    f"DeepRhythmEssentiaBackend requires essentia for key. "
                    f"Install with: pip install essentia. Error: {e}"
                )

    def metadata(self) -> BackendMetadata:
        return BackendMetadata(
            id="deeprhythm_essentia",
            label="DeepRhythm BPM + Essentia key (bgate)",
            version=_VERSION,
            config_hash="v1",
            heavy_deps=["torch", "DeepRhythm", "essentia", "numpy", "scipy"],
            network_required=False,
        )

    def analyze_track(self, track_path: str) -> AnalysisResult:
        from fourfour_analysis.audio_io import load_audio, preprocess_tempo, preprocess_waveform
        from fourfour_analysis.backends.lexicon_bpm import _generate_beats
        from fourfour_analysis.backends.lexicon_energy import compute_energy
        from fourfour_analysis.backends.lexicon_waveform import generate_waveform
        from fourfour_analysis.backends.lexicon_cues import detect_sections

        audio, sr = load_audio(track_path)

        needs_tempo = bool(self._features & {"bpm", "energy", "cues", "waveform"})

        # BPM via DeepRhythm
        bpm: Optional[float] = None
        beats: list[float] = []
        tempo_audio = None

        if "bpm" in self._features:
            bpm = self._detect_bpm(track_path)

        if needs_tempo:
            tempo_audio, _ = preprocess_tempo(audio, sr)
            duration = len(tempo_audio) / sr
            if bpm is not None and bpm > 0:
                beats, _anchor_idx = _generate_beats(tempo_audio, sr, bpm, duration)
            # If DeepRhythm failed, beats stays empty

        # Key via Essentia bgate
        key: Optional[str] = None
        if "key" in self._features:
            key = self._detect_key(track_path)

        # Energy via Lexicon
        energy: Optional[int] = None
        if "energy" in self._features and bpm is not None and tempo_audio is not None:
            energy = compute_energy(tempo_audio, sr, bpm)

        # Waveform via Lexicon
        peaks: list[WaveformPeak] = []
        colors: list[WaveformColor] = []
        fft_bands_list: list[list[int]] = []
        if "waveform" in self._features:
            waveform_audio, waveform_sr = preprocess_waveform(audio, sr)
            waveform_columns = generate_waveform(waveform_audio, waveform_sr)
            peaks = [WaveformPeak(min_val=c.min_val, max_val=c.max_val) for c in waveform_columns]
            colors = [WaveformColor(r=c.r, g=c.g, b=c.b) for c in waveform_columns]
            fft_bands_list = [list(c.fft_bands) for c in waveform_columns]

        # Cue points via Lexicon
        cue_points = []
        if "cues" in self._features and bpm is not None and len(beats) > 0 and tempo_audio is not None:
            cue_points = detect_sections(beats, tempo_audio, sr, bpm)

        # Convert beat times to BeatPosition
        beat_positions = [
            BeatPosition(time_seconds=t, bar_position=(i % 4) + 1)
            for i, t in enumerate(beats)
        ]

        return AnalysisResult(
            bpm=bpm,
            key=key,
            energy=energy,
            beats=beat_positions,
            waveform_peaks=peaks,
            waveform_colors=colors,
            waveform_fft_bands=fft_bands_list,
            cue_points=cue_points,
        )

    def _detect_bpm(self, track_path: str) -> Optional[float]:
        """BPM via DeepRhythm with full stdout/stderr suppression.

        DeepRhythm's Python code calls print() which buffers at the Python level,
        so we must redirect both sys.stdout (Python buffer) and FD 1 (C level).
        Replacing sys.stdout with StringIO means the buffered print never reaches
        FD 1 even after it is restored.
        """
        import io
        import os
        import sys
        import warnings

        sink = io.StringIO()
        old_sys_stdout, old_sys_stderr = sys.stdout, sys.stderr

        with open(os.devnull, "w") as devnull:
            devnull_fd = devnull.fileno()
            saved_stdout = os.dup(1)
            saved_stderr = os.dup(2)
            try:
                sys.stdout = sink
                sys.stderr = sink
                os.dup2(devnull_fd, 1)
                os.dup2(devnull_fd, 2)
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    from deeprhythm import DeepRhythmPredictor
                predictor = DeepRhythmPredictor()
                result = predictor.predict(track_path)
            finally:
                sys.stdout = old_sys_stdout
                sys.stderr = old_sys_stderr
                os.dup2(saved_stdout, 1)
                os.dup2(saved_stderr, 2)
                os.close(saved_stdout)
                os.close(saved_stderr)

        return float(result) if result else None

    def _detect_key(self, track_path: str) -> Optional[str]:
        """Key via Essentia KeyExtractor bgate with C-level stdout suppression."""
        import os
        import essentia.standard as es
        from fourfour_analysis.groundtruth import normalize_key

        with open(os.devnull, "w") as devnull:
            devnull_fd = devnull.fileno()
            saved_stdout = os.dup(1)
            saved_stderr = os.dup(2)
            try:
                os.dup2(devnull_fd, 1)
                os.dup2(devnull_fd, 2)
                audio = es.MonoLoader(filename=track_path, sampleRate=44100)()
                key, scale, _ = es.KeyExtractor(profileType="bgate", sampleRate=44100)(audio)
            finally:
                os.dup2(saved_stdout, 1)
                os.dup2(saved_stderr, 2)
                os.close(saved_stdout)
                os.close(saved_stderr)

        return normalize_key(f"{key} {scale}")

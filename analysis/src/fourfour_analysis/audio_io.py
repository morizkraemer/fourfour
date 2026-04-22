"""Shared audio loading and preprocessing for all backends."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path
from typing import Optional

import numpy as np
from scipy.signal import butter, sosfilt, resample_poly


def load_audio(path: str | Path, sr: Optional[int] = None) -> tuple[np.ndarray, int]:
    """Load audio file to mono f32 numpy array.

    Uses soundfile for WAV/FLAC, falls back to ffmpeg pipe for MP3/AAC/etc.

    Args:
        path: Path to audio file.
        sr: Target sample rate. None = native.

    Returns:
        (mono_f32_samples, sample_rate)
    """
    path = Path(path)
    if not path.is_file():
        raise FileNotFoundError(f"Audio file not found: {path}")

    suffix = path.suffix.lower()

    if suffix in (".wav", ".flac", ".aiff", ".ogg"):
        samples, native_sr = _load_soundfile(path)
    else:
        samples, native_sr = _load_ffmpeg(path)

    # Mono downmix
    if samples.ndim == 2:
        samples = samples.mean(axis=1)

    if sr is not None and sr != native_sr:
        samples = resample_poly(samples, sr, native_sr)
        return samples.astype(np.float32), sr

    return samples.astype(np.float32), native_sr


def _load_soundfile(path: Path) -> tuple[np.ndarray, int]:
    """Load via soundfile (WAV, FLAC, etc.)."""
    import soundfile as sf

    data, sr = sf.read(path, dtype="float32")
    return data, sr


def _load_ffmpeg(path: Path, sr: int = 44100) -> tuple[np.ndarray, int]:
    """Decode via ffmpeg pipe to raw PCM f32le."""
    cmd = [
        "ffmpeg", "-i", str(path),
        "-ac", "1",           # mono
        "-ar", str(sr),       # target sample rate
        "-f", "f32le",        # raw PCM float32 little-endian
        "-v", "error",
        "pipe:1",
    ]
    try:
        result = subprocess.run(cmd, capture_output=True, check=True)
    except FileNotFoundError:
        raise RuntimeError(
            "ffmpeg not found on PATH. Install ffmpeg or use WAV/FLAC files."
        )
    except subprocess.CalledProcessError as e:
        raise RuntimeError(f"ffmpeg failed: {e.stderr.decode()}")

    samples = np.frombuffer(result.stdout, dtype=np.float32)
    return samples, sr


def resample_audio(audio: np.ndarray, sr_from: int, sr_to: int) -> tuple[np.ndarray, int]:
    """Resample audio to a new sample rate."""
    if sr_from == sr_to:
        return audio, sr_to
    # Use GCD to keep integers small
    from math import gcd
    g = gcd(sr_to, sr_from)
    up = sr_to // g
    down = sr_from // g
    resampled = resample_poly(audio, up, down)
    return resampled.astype(np.float32), sr_to


def lowpass_cascade(
    audio: np.ndarray,
    sr: int,
    cutoff_freqs: list[float],
    order: int = 2,
) -> np.ndarray:
    """Apply N-stage cascading Butterworth lowpass filter.

    Args:
        audio: Mono f32 samples.
        sr: Sample rate.
        cutoff_freqs: List of cutoff frequencies for each stage.
        order: Filter order per stage.

    Returns:
        Filtered audio.
    """
    out = audio.copy()
    for freq in cutoff_freqs:
        # Nyquist check
        if freq >= sr / 2:
            continue
        sos = butter(order, freq, btype="low", fs=sr, output="sos")
        out = sosfilt(sos, out)
    return out


def preprocess_tempo(audio: np.ndarray, sr: int) -> tuple[np.ndarray, int]:
    """Preprocess for tempo/BPM analysis.

    7-stage cascading lowpass: [800, 400, 400, 200, 200, 200, 200] Hz.
    Matches Lexicon's OfflineAudioContext filter chain.
    Result contains only kick drums and bass — ideal for tempo detection.

    Returns:
        (filtered_audio, sr) — same sample rate, filtered content.
    """
    cascade_freqs = [800, 400, 400, 200, 200, 200, 200]
    filtered = lowpass_cascade(audio, sr, cascade_freqs, order=2)
    return filtered, sr


def preprocess_key(audio: np.ndarray, sr: int) -> tuple[np.ndarray, int]:
    """Preprocess for key detection.

    FIR lowpass at ~1999 Hz, then decimate by ~10:1.
    Result: ~4400 Hz sample rate containing harmonic content only.

    Returns:
        (decimated_audio, effective_sr)
    """
    from scipy.signal import firwin, lfilter

    # Design FIR lowpass — 110 taps, cutoff ~1999 Hz (matches Lexicon)
    # The original uses a specific FIR kernel; we approximate with windowed sinc
    numtaps = 110
    cutoff_norm = 1999.0 / (sr / 2.0)
    if cutoff_norm >= 1.0:
        cutoff_norm = 0.95
    taps = firwin(numtaps, cutoff_norm, window="blackman")
    filtered = lfilter(taps, 1.0, audio)

    # Decimate by ~10:1 (find a clean integer ratio)
    decimate_factor = max(1, round(sr / 4400))
    decimated = filtered[::decimate_factor]
    effective_sr = sr // decimate_factor

    return decimated.astype(np.float32), effective_sr


def preprocess_waveform(audio: np.ndarray, sr: int) -> tuple[np.ndarray, int]:
    """Preprocess for waveform display.

    Resample to 12kHz mono (matches Lexicon waveform worker).

    Returns:
        (resampled_audio, 12000)
    """
    target_sr = 12000
    resampled, _ = resample_audio(audio, sr, target_sr)
    return resampled, target_sr

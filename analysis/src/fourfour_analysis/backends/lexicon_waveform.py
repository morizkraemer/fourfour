"""Lexicon waveform generation — 128-pt FFT, 3-band color.

Port of Lexicon's Worker 160 waveform renderer.
Reference: docs/lexicon-wiki.md §9

Per 256-sample segment at 12kHz:
  1. Min/max → waveform shape
  2. 128-pt FFT → 3-band RMS → RGB color
  3. Smooth with previous segment (50% blend)
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np


# ── Constants (from Lexicon) ──────────────────────────────

TARGET_SR = 12_000
FFT_SIZE = 128
# 80 samples at 12 kHz = 6.67 ms/column = 150 col/sec, matching Rekordbox PWV5/PWV3.
# The FFT window is zero-padded from SEGMENT_WIDTH to FFT_SIZE to preserve band resolution.
SEGMENT_WIDTH = 80

# Frequency bands (Hz) — Nyquist at 12kHz = 6kHz.
#
# At 12kHz / 128-pt FFT, freq_per_bin = 93.75 Hz.  The original (0, 150) low band
# covered only bin 0 (DC, always ~0 in music), making the low channel dead and the
# high band win every column → all-white display.
#
# New bands capture musically meaningful regions:
#   LOW:  60–250 Hz  → kick drum fundamental + sub-bass (bins 1–2, ≈ 2 bins)
#   MID:  250–2500 Hz → bass guitar, synths, snare body  (bins 2–26, ≈ 24 bins)
#   HIGH: 2500–6000 Hz → cymbals, hi-hats, presence     (bins 26–64, ≈ 38 bins)
#
# LOW_WEIGHT is aggressive to match Rekordbox's bass-first visual hierarchy.
LOW_BAND = (60, 250)
MID_BAND = (250, 2500)
HIGH_BAND = (2500, 6000)

# Band weights — low heavily boosted to make kick drum visually dominant,
# matching Rekordbox's perceptual tuning (bass fills the waveform).
LOW_WEIGHT = 5.0
MID_WEIGHT = 1.0
HIGH_WEIGHT = 0.6

# Smoothing: blend with previous segment (0 = no smoothing, 1 = freeze).
# Reduced from 0.5 → sharper transients to match Rekordbox appearance.
MIX_FACTOR = 0.1


@dataclass(frozen=True)
class WaveformParams:
    """Parameter set for a single waveform generation run."""
    target_sr: int = TARGET_SR
    fft_size: int = FFT_SIZE
    segment_width: int = SEGMENT_WIDTH
    low_band: tuple[float, float] = LOW_BAND
    mid_band: tuple[float, float] = MID_BAND
    high_band: tuple[float, float] = HIGH_BAND
    low_weight: float = LOW_WEIGHT
    mid_weight: float = MID_WEIGHT
    high_weight: float = HIGH_WEIGHT
    mix_factor: float = MIX_FACTOR


@dataclass(frozen=True)
class WaveformColumn:
    """One column of waveform data: shape + color + raw FFT sub-bands."""
    min_val: float
    max_val: float
    r: int
    g: int
    b: int
    # Raw FFT magnitudes, bins 1-64 (93.75 Hz–6 kHz at 12 kHz/128-pt FFT).
    # Per-column normalised: dominant bin = 255.  Used for interactive crossover tuning.
    fft_bands: tuple[int, ...]


def generate_waveform_with_params(
    audio: np.ndarray,
    sr: int,
    params: WaveformParams,
) -> list[WaveformColumn]:
    """Generate waveform display data from mono audio with explicit parameters.

    Args:
        audio: Mono f32, already resampled to params.target_sr.
        sr: Sample rate (should match params.target_sr).
        params: Waveform generation parameters.

    Returns:
        List of WaveformColumn, one per segment.
    """
    seg_w = params.segment_width
    if len(audio) < seg_w:
        return []

    num_segments = len(audio) // seg_w
    columns: list[WaveformColumn] = []

    prev_r, prev_g, prev_b = 0, 0, 0

    # Precompute FFT frequency bins
    freq_per_bin = sr / params.fft_size
    low_bin_start = int(params.low_band[0] / freq_per_bin)
    low_bin_end = max(low_bin_start + 1, int(params.low_band[1] / freq_per_bin))
    mid_bin_start = int(params.mid_band[0] / freq_per_bin)
    mid_bin_end = max(mid_bin_start + 1, int(params.mid_band[1] / freq_per_bin))
    high_bin_start = int(params.high_band[0] / freq_per_bin)
    high_bin_end = max(high_bin_start + 1, int(params.high_band[1] / freq_per_bin))

    for i in range(num_segments):
        start = i * seg_w
        segment = audio[start:start + seg_w]

        # 1. Min/max for waveform shape
        min_val = float(np.min(segment))
        max_val = float(np.max(segment))

        # 2. FFT for color — zero-pad segment to FFT_SIZE to preserve frequency resolution
        fft_input = np.zeros(params.fft_size, dtype=np.float32)
        fft_input[:len(segment)] = segment
        spectrum = np.fft.rfft(fft_input)
        magnitudes = np.abs(spectrum)

        # Raw per-bin magnitudes (bins 1-64, skip DC at bin 0), per-column normalised 0-255.
        fft_raw = magnitudes[1:65]  # 64 values covering 93.75 Hz–6 kHz
        fft_max = float(np.max(fft_raw)) if fft_raw.size > 0 else 0.0
        if fft_max > 1e-10:
            fft_bands: tuple[int, ...] = tuple(int(round(float(v) / fft_max * 255)) for v in fft_raw)
        else:
            fft_bands = (0,) * 64

        # RMS per band
        low_rms = _band_rms(magnitudes, low_bin_start, low_bin_end) * params.low_weight
        mid_rms = _band_rms(magnitudes, mid_bin_start, mid_bin_end) * params.mid_weight
        high_rms = _band_rms(magnitudes, high_bin_start, high_bin_end) * params.high_weight

        # Normalize to strongest band
        max_band = max(low_rms, mid_rms, high_rms, 1e-10)

        r_raw = int(round(low_rms / max_band * 255))
        g_raw = int(round(mid_rms / max_band * 255))
        b_raw = int(round(high_rms / max_band * 255))

        # 3. Smooth with previous segment
        r = int(round(prev_r * params.mix_factor + r_raw * (1 - params.mix_factor)))
        g = int(round(prev_g * params.mix_factor + g_raw * (1 - params.mix_factor)))
        b = int(round(prev_b * params.mix_factor + b_raw * (1 - params.mix_factor)))

        prev_r, prev_g, prev_b = r, g, b

        columns.append(WaveformColumn(
            min_val=min_val,
            max_val=max_val,
            r=max(0, min(255, r)),
            g=max(0, min(255, g)),
            b=max(0, min(255, b)),
            fft_bands=fft_bands,
        ))

    return columns


def generate_waveform(
    audio: np.ndarray,
    sr: int,
) -> list[WaveformColumn]:
    """Generate waveform display data from 12kHz mono audio (default parameters).

    Audio should already be resampled to 12kHz
    (use audio_io.preprocess_waveform before calling this).
    """
    return generate_waveform_with_params(audio, sr, WaveformParams())


# ── Parameter sweep variants ──────────────────────────────

SWEEP_VARIANTS: list[tuple[str, WaveformParams]] = [
    ("baseline", WaveformParams()),
]


def _band_rms(magnitudes: np.ndarray, bin_start: int, bin_end: int) -> float:
    """Compute RMS energy of FFT magnitude bins in a frequency band."""
    if bin_end > len(magnitudes):
        bin_end = len(magnitudes)
    if bin_start >= bin_end:
        return 0.0

    band = magnitudes[bin_start:bin_end]
    if len(band) == 0:
        return 0.0

    return float(np.sqrt(np.mean(band ** 2)))


# ── Filter bank waveform generation ───────────────────────

@dataclass(frozen=True)
class FilterbankParams:
    """Parameters for filter-bank waveform generation (matched to Rekordbox PWV7).

    Calibrated against 5 Rekordbox-analyzed tracks. Key findings:
    - Crossover at 130/2500 Hz with 4th-order Butterworth matches Rekordbox band split
    - sqrt compression (power=0.5) matches Rekordbox's dynamic range
    - Fixed per-band gains (no per-track normalization) — Rekordbox uses absolute scaling
    - Values clamped to 0-127 (7-bit range used by PWV7 format)
    """
    target_sr: int = 12_000
    segment_width: int = 80
    low_cutoff: float = 130.0      # low-mid crossover (Hz)
    mid_cutoff: float = 2500.0     # mid-high crossover (Hz)
    filter_order: int = 4
    measure: str = "rms"           # "max" | "rms" | "mean"
    power: float = 0.5             # compression exponent (sqrt)
    gain_low: float = 140.0        # fixed gain per band (no per-track normalization)
    gain_mid: float = 120.0
    gain_high: float = 70.0
    mix_factor: float = 0.1
    peak_hold: int = 3             # hold peak for N segments (fattens transients)


def _bandpass_filter(data: np.ndarray, low: float, high: float, sr: int, order: int = 4) -> np.ndarray:
    """Apply Butterworth bandpass filter."""
    from scipy.signal import butter, sosfilt
    nyq = sr / 2.0
    low_norm = max(low / nyq, 0.001)
    high_norm = min(high / nyq, 0.999)
    sos = butter(order, [low_norm, high_norm], btype="band", output="sos")
    return sosfilt(sos, data)


def generate_waveform_filterbank(
    audio: np.ndarray,
    sr: int,
    params: FilterbankParams | None = None,
) -> list[WaveformColumn]:
    """Generate 3-band waveform matched to Rekordbox PWV7 format.

    Uses Butterworth crossover filters with sqrt compression and fixed
    per-band gains.  No per-track normalization — absolute scaling so
    quiet tracks look quiet and loud tracks look loud, matching Rekordbox.
    Output values are clamped to 0-127 (PWV7 native range).
    """
    if params is None:
        params = FilterbankParams()

    seg_w = params.segment_width
    if len(audio) < seg_w:
        return []

    num_segments = len(audio) // seg_w

    # Apply 3 bandpass filters to full audio
    low_sig = np.abs(_bandpass_filter(audio, 0.0, params.low_cutoff, sr, params.filter_order))
    mid_sig = np.abs(_bandpass_filter(audio, params.low_cutoff, params.mid_cutoff, sr, params.filter_order))
    high_sig = np.abs(_bandpass_filter(audio, params.mid_cutoff, sr / 2.0, sr, params.filter_order))

    # First pass: compute raw per-segment RMS values
    raw_values: list[tuple[float, float, float]] = []
    for i in range(num_segments):
        start = i * seg_w
        end = start + seg_w
        if params.measure == "rms":
            l = float(np.sqrt(np.mean(low_sig[start:end] ** 2)))
            m = float(np.sqrt(np.mean(mid_sig[start:end] ** 2)))
            h = float(np.sqrt(np.mean(high_sig[start:end] ** 2)))
        elif params.measure == "mean":
            l = float(np.mean(low_sig[start:end]))
            m = float(np.mean(mid_sig[start:end]))
            h = float(np.mean(high_sig[start:end]))
        else:  # max
            l = float(np.max(low_sig[start:end]))
            m = float(np.max(mid_sig[start:end]))
            h = float(np.max(high_sig[start:end]))
        raw_values.append((l, m, h))

    # Peak-hold: for each segment, use the max of itself and the next N segments.
    # This fattens transients (kick drums hold their peak for a few columns).
    hold = params.peak_hold
    held_values: list[tuple[float, float, float]] = []
    for i in range(num_segments):
        window = raw_values[i : min(i + hold + 1, num_segments)]
        held_values.append((
            max(v[0] for v in window),
            max(v[1] for v in window),
            max(v[2] for v in window),
        ))

    columns: list[WaveformColumn] = []
    prev_r, prev_g, prev_b = 0.0, 0.0, 0.0

    for i, (l, m, h) in enumerate(held_values):
        start = i * seg_w
        segment = audio[start:start + seg_w]
        min_val = float(np.min(segment))
        max_val = float(np.max(segment))

        # Power-law compression + fixed gain (no per-track normalization)
        r_raw = params.gain_low * (l ** params.power)
        g_raw = params.gain_mid * (m ** params.power)
        b_raw = params.gain_high * (h ** params.power)

        # Smooth with previous segment
        r = prev_r * params.mix_factor + r_raw * (1 - params.mix_factor)
        g = prev_g * params.mix_factor + g_raw * (1 - params.mix_factor)
        b = prev_b * params.mix_factor + b_raw * (1 - params.mix_factor)
        prev_r, prev_g, prev_b = r, g, b

        columns.append(WaveformColumn(
            min_val=min_val,
            max_val=max_val,
            r=max(0, min(127, int(round(r)))),
            g=max(0, min(127, int(round(g)))),
            b=max(0, min(127, int(round(b)))),
            fft_bands=(0,) * 64,
        ))

    return columns


# ── Preview waveform (400-byte Pioneer PWAV) ─────────────

def generate_preview(detail: list[WaveformColumn]) -> bytes:
    """Downsample detail waveform to 400-byte Pioneer PWAV format.

    Each byte encodes: height (5 low bits, 0-31) | whiteness (3 high bits, 0-7).
    Height = overall amplitude. Whiteness = how much high-frequency content
    (bright = hi-hats/cymbals, dark = bass-heavy).

    Uses max-per-bin downsampling to preserve transient peaks.
    """
    n = len(detail)
    if n == 0:
        return bytes(400)

    out = bytearray(400)
    for i in range(400):
        start = i * n // 400
        end = (i + 1) * n // 400
        bin_slice = detail[start:end] if end > start else [detail[start]]

        # Height from peak amplitude across the bin
        max_val = max(abs(c.max_val) for c in bin_slice)
        height = min(31, int(round(max_val * 31.0)))

        # Whiteness from high-band dominance: high / (low + mid + high)
        # High whiteness = treble-heavy (hi-hats), low = bass-heavy (kicks)
        max_r = max(c.r for c in bin_slice)
        max_g = max(c.g for c in bin_slice)
        max_b = max(c.b for c in bin_slice)
        total = max_r + max_g + max_b
        whiteness = min(7, int(round((max_b / total) * 7.0))) if total > 0 else 0

        out[i] = (whiteness << 5) | height

    return bytes(out)


# ── Overview waveform (1200-entry Pioneer PWV6) ──────────

def generate_overview(detail: list[WaveformColumn], num_entries: int = 1200) -> list[WaveformColumn]:
    """Downsample detail waveform to fixed-size overview using peak-hold.

    Uses max-per-bin (not point-sample) to preserve transient peaks.
    Matches Pioneer PWV6 format: 1200 entries covering the full track.
    """
    n = len(detail)
    if n == 0:
        return []
    if n <= num_entries:
        return list(detail)

    columns: list[WaveformColumn] = []
    for i in range(num_entries):
        start = i * n // num_entries
        end = (i + 1) * n // num_entries
        bin_slice = detail[start:end] if end > start else [detail[start]]

        max_r = max(c.r for c in bin_slice)
        max_g = max(c.g for c in bin_slice)
        max_b = max(c.b for c in bin_slice)
        max_max_val = max(c.max_val for c in bin_slice)
        min_min_val = min(c.min_val for c in bin_slice)

        columns.append(WaveformColumn(
            min_val=min_min_val,
            max_val=max_max_val,
            r=max_r,
            g=max_g,
            b=max_b,
            fft_bands=(0,) * 64,
        ))

    return columns


# ── Filter bank sweep variants ────────────────────────────

FILTERBANK_VARIANTS: list[tuple[str, FilterbankParams]] = [
    ("FB 130/2.5k sqrt", FilterbankParams()),
]

"""PWV7 hypothesis testing framework.

Systematically test waveform generation hypotheses against Rekordbox reference
data. Loads PWV7 from Rekordbox .2EX files (via master.db lookup), generates
candidate waveforms with varying parameters, and scores by MAE + correlation.

Usage:
    python -m fourfour_analysis pwv7-hypotheses <audio-file-or-dir>
"""

from __future__ import annotations

import json
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Callable, Optional

import numpy as np
from numpy.typing import NDArray

from fourfour_analysis.audio_io import load_audio, preprocess_waveform
from fourfour_analysis.backends.lexicon_waveform import FilterbankParams, generate_waveform_filterbank


# ── Rekordbox reference loading ────────────────────────────────────────────

def find_rekordbox_anlz(audio_path: Path) -> Optional[Path]:
    """Find Rekordbox ANLZ path for an audio file via master.db."""
    import sqlite3

    home = Path.home()
    master_db = home / "Library/Pioneer/rekordbox/master.db"
    if not master_db.exists():
        return None

    KEY = "402fd482c38817c35ffa8ffb8c7d93143b749e7d315df7a81732a1ff43608497"
    folder_path = str(audio_path.resolve())
    filename = audio_path.name
    like_pattern = f"%/{filename}"

    try:
        # sqlcipher3 if available, otherwise try pysqlcipher3
        try:
            import sqlcipher3
            conn = sqlcipher3.connect(str(master_db))
        except ImportError:
            conn = sqlite3.connect(str(master_db))
        conn.execute(f"PRAGMA key='{KEY}'")
        conn.execute("SELECT count(*) FROM sqlite_master")

        cursor = conn.execute(
            "SELECT AnalysisDataPath FROM djmdContent "
            "WHERE (FolderPath = ? OR FolderPath LIKE ?) AND AnalysisDataPath IS NOT NULL "
            "LIMIT 1",
            (folder_path, like_pattern),
        )
        row = cursor.fetchone()
        conn.close()

        if not row or not row[0]:
            return None

        anlz_rel = row[0].lstrip("/")
        share_base = home / "Library/Pioneer/rekordbox/share"
        dat_path = share_base / anlz_rel
        ex_path = dat_path.with_suffix(".2EX")
        return ex_path if ex_path.exists() else None
    except Exception as e:
        print(f"  master.db lookup failed: {e}", file=sys.stderr)
        return None


def read_pwv7(path: Path) -> tuple[NDArray[np.float64], Optional[NDArray[np.float64]]]:
    """Read PWV7 (detail) and PWV6 (overview) from a .2EX file.

    Returns (detail, overview) where each is a float64 array of shape (N, 3)
    with values normalized to 0.0–1.0 (divided by 127).
    """
    data = path.read_bytes()
    if len(data) < 28 or data[:4] != b"PMAI":
        raise ValueError("Not a valid .2EX file")

    file_header_len = int.from_bytes(data[4:8], "big")
    offset = file_header_len

    detail: Optional[NDArray[np.float64]] = None
    overview: Optional[NDArray[np.float64]] = None

    while offset + 12 <= len(data):
        tag = data[offset:offset + 4]
        header_len = int.from_bytes(data[offset + 4:offset + 8], "big")
        section_len = int.from_bytes(data[offset + 8:offset + 12], "big")

        if section_len == 0 or offset + section_len > len(data):
            break

        section = data[offset:offset + section_len]

        if tag == b"PWV7" and len(section) >= 24:
            entry_count = int.from_bytes(section[16:20], "big")
            payload = section[24:]
            n = min(entry_count, len(payload) // 3)
            arr = np.frombuffer(payload[:n * 3], dtype=np.uint8).reshape(n, 3).astype(np.float64) / 127.0
            detail = arr

        elif tag == b"PWV6" and len(section) >= 20:
            entry_count = int.from_bytes(section[16:20], "big")
            payload = section[20:]
            n = min(entry_count, len(payload) // 3)
            arr = np.frombuffer(payload[:n * 3], dtype=np.uint8).reshape(n, 3).astype(np.float64) / 127.0
            overview = arr

        offset += section_len

    if detail is None:
        raise ValueError("No PWV7 section found in .2EX file")

    return detail, overview


# ── Scoring ────────────────────────────────────────────────────────────────

@dataclass
class BandScores:
    mae: float
    correlation: float
    mae_low: float
    mae_mid: float
    mae_high: float
    corr_low: float
    corr_mid: float
    corr_high: float


@dataclass
class TrackScore:
    track_path: str
    num_columns: int
    bands: BandScores


@dataclass
class HypothesisResult:
    hypothesis_id: str
    description: str
    config: dict
    track_scores: list[TrackScore] = field(default_factory=list)
    aggregate_mae: float = 0.0
    aggregate_corr: float = 0.0

    @property
    def num_tracks(self) -> int:
        return len(self.track_scores)

    def compute_aggregate(self) -> None:
        if not self.track_scores:
            return
        # Weighted by track length (column count)
        total_cols = sum(s.num_columns for s in self.track_scores)
        self.aggregate_mae = sum(s.bands.mae * s.num_columns for s in self.track_scores) / total_cols
        self.aggregate_corr = sum(s.bands.correlation * s.num_columns for s in self.track_scores) / total_cols


def score_waveform(ours: NDArray[np.float64], ref: NDArray[np.float64]) -> BandScores:
    """Compare two (N, 3) normalized waveforms. Ours may differ in length from ref.

    If lengths differ, resample ours to match ref using simple linear interpolation.
    """
    if ours.shape[1] != 3 or ref.shape[1] != 3:
        raise ValueError("Waveforms must have 3 bands")

    # Resample to match ref length
    if len(ours) != len(ref):
        x_old = np.linspace(0, 1, len(ours))
        x_new = np.linspace(0, 1, len(ref))
        ours_resampled = np.zeros((len(ref), 3), dtype=np.float64)
        for b in range(3):
            ours_resampled[:, b] = np.interp(x_new, x_old, ours[:, b])
        ours = ours_resampled

    diff = np.abs(ours - ref)
    mae_low = float(np.mean(diff[:, 0]))
    mae_mid = float(np.mean(diff[:, 1]))
    mae_high = float(np.mean(diff[:, 2]))
    mae = float(np.mean(diff))

    # Pearson correlation per band
    def _corr(a: NDArray, b: NDArray) -> float:
        if np.std(a) < 1e-10 or np.std(b) < 1e-10:
            return 0.0
        return float(np.corrcoef(a, b)[0, 1])

    corr_low = _corr(ours[:, 0], ref[:, 0])
    corr_mid = _corr(ours[:, 1], ref[:, 1])
    corr_high = _corr(ours[:, 2], ref[:, 2])
    # Average correlation
    correlation = (corr_low + corr_mid + corr_high) / 3.0

    return BandScores(
        mae=mae,
        correlation=correlation,
        mae_low=mae_low,
        mae_mid=mae_mid,
        mae_high=mae_high,
        corr_low=corr_low,
        corr_mid=corr_mid,
        corr_high=corr_high,
    )


# ── Hypothesis implementations ─────────────────────────────────────────────

@dataclass(frozen=True)
class HypothesisConfig:
    """Full parameter set for one hypothesis test.

    Defaults match the current production FilterbankParams.
    """
    # Filter
    filter_type: str = "butterworth"  # butterworth | linkwitz_riley | shelving | fft
    filter_order: int = 4
    low_cutoff: float = 130.0
    mid_cutoff: float = 2500.0
    # Weighting
    weighting: str = "none"  # none | a_weight | c_weight | itu_r_468
    # Measurement
    measure: str = "rms"  # rms | peak | true_peak | mean
    # Multi-pass
    smoothing: str = "mix"  # none | mix | block_max | envelope | compressor
    mix_factor: float = 0.1
    block_max_width: int = 1
    envelope_lookahead: int = 0
    compressor_attack_ms: float = 0.0
    compressor_release_ms: float = 0.0
    peak_hold: int = 1  # 1 = no hold, >1 = forward-looking peak hold (matches production)
    # Normalization
    normalize: str = "fixed"  # fixed | track_peak | track_loudness | sigmoid
    gain_low: float = 140.0
    gain_mid: float = 120.0
    gain_high: float = 70.0
    power: float = 0.5
    # Window
    target_sr: int = 12_000
    segment_width: int = 80
    overlap: float = 0.0  # 0.0 = no overlap, 0.5 = 50%

    def id(self) -> str:
        parts = [
            f"filt={self.filter_type}",
            f"ord={self.filter_order}",
            f"cut={self.low_cutoff:.0f}/{self.mid_cutoff:.0f}",
            f"w={self.weighting}",
            f"m={self.measure}",
            f"sm={self.smoothing}",
            f"ph={self.peak_hold}",
            f"norm={self.normalize}",
            f"p={self.power:.2f}",
            f"sw={self.segment_width}",
            f"ov={self.overlap:.1f}",
        ]
        return "|".join(parts)


def _a_weighting(freqs: NDArray) -> NDArray:
    """A-weighting curve in frequency domain."""
    c1 = 12194.217**2
    c2 = 20.598997**2
    c3 = 107.65265**2
    c4 = 737.86223**2
    f2 = freqs**2
    f4 = f2**2
    num = c1 * f4
    den = (f2 + c2) * np.sqrt((f2 + c3) * (f2 + c4)) * (f2 + c1)
    ra = num / den
    # Normalize so 1kHz = 1.0
    f1k = 1000.0
    f1k2 = f1k**2
    f1k4 = f1k2**2
    num1k = c1 * f1k4
    den1k = (f1k2 + c2) * np.sqrt((f1k2 + c3) * (f1k2 + c4)) * (f1k2 + c1)
    ra1k = num1k / den1k
    return ra / ra1k


def _c_weighting(freqs: NDArray) -> NDArray:
    """C-weighting curve in frequency domain."""
    c1 = 12194.217**2
    c2 = 20.598997**2
    f2 = freqs**2
    f4 = f2**2
    num = c1 * f4
    den = (f2 + c2) * (f2 + c1)
    rc = num / den
    f1k = 1000.0
    f1k2 = f1k**2
    f1k4 = f1k2**2
    num1k = c1 * f1k4
    den1k = (f1k2 + c2) * (f1k2 + c1)
    rc1k = num1k / den1k
    return rc / rc1k


def _itu_r_468_weighting(freqs: NDArray) -> NDArray:
    """ITU-R 468 weighting approximation."""
    # Simplified approximation
    h1 = -4.737338981378384e-24 * freqs**6 + 2.043828333606125e-15 * freqs**4 - 1.363894795463638e-7 * freqs**2 + 1
    h1 = np.maximum(h1, 0.0)
    h2 = 1.306612257412824e-19 * freqs**5 - 2.118150887518656e-11 * freqs**3 + 5.559488023498642e-4 * freqs
    r = 1.246332637532143e-4 * freqs / np.sqrt(h1**2 + h2**2)
    # Normalize roughly
    return r / np.maximum(np.max(r), 1e-10)


def _apply_filters(audio: NDArray, sr: int, cfg: HypothesisConfig) -> tuple[NDArray, NDArray, NDArray]:
    """Apply band-split filters according to config. Returns (low, mid, high) signals."""
    nyq = sr / 2.0

    if cfg.filter_type == "butterworth":
        from scipy.signal import butter, sosfilt
        low_sig = np.abs(sosfilt(butter(cfg.filter_order, cfg.low_cutoff / nyq, btype="low", output="sos"), audio))
        mid_sig = np.abs(sosfilt(butter(cfg.filter_order, [cfg.low_cutoff / nyq, cfg.mid_cutoff / nyq], btype="band", output="sos"), audio))
        high_sig = np.abs(sosfilt(butter(cfg.filter_order, cfg.mid_cutoff / nyq, btype="high", output="sos"), audio))

    elif cfg.filter_type == "linkwitz_riley":
        # Linkwitz-Riley: cascade two Butterworth filters for flat summed response
        from scipy.signal import butter, sosfilt
        order = max(2, cfg.filter_order // 2)
        # Lowpass for low band
        sos_lp = butter(order, cfg.low_cutoff / nyq, btype="low", output="sos")
        low_sig = np.abs(sosfilt(sos_lp, sosfilt(sos_lp, audio)))
        # Bandpass for mid
        sos_bp = butter(order, [cfg.low_cutoff / nyq, cfg.mid_cutoff / nyq], btype="band", output="sos")
        mid_sig = np.abs(sosfilt(sos_bp, sosfilt(sos_bp, audio)))
        # Highpass for high
        sos_hp = butter(order, cfg.mid_cutoff / nyq, btype="high", output="sos")
        high_sig = np.abs(sosfilt(sos_hp, sosfilt(sos_hp, audio)))

    elif cfg.filter_type == "shelving":
        # 1st-order shelving approximation
        from scipy.signal import bilinear, lfilter
        def _shelving_lp(cutoff: float) -> NDArray:
            wc = 2 * np.pi * cutoff
            # Analog 1st-order lowpass: H(s) = wc / (s + wc)
            # Digital via bilinear transform
            b, a = bilinear([wc], [1, wc], fs=sr)
            return b, a
        def _shelving_hp(cutoff: float) -> NDArray:
            wc = 2 * np.pi * cutoff
            b, a = bilinear([1, 0], [1, wc], fs=sr)
            return b, a
        b_lp, a_lp = _shelving_lp(cfg.low_cutoff)
        low_sig = np.abs(lfilter(b_lp, a_lp, audio))
        # Mid = bandpass via lowpass @ mid - lowpass @ low
        b_mp, a_mp = _shelving_lp(cfg.mid_cutoff)
        mid_lp = np.abs(lfilter(b_mp, a_mp, audio))
        mid_sig = np.maximum(mid_lp - low_sig, 0.0)
        high_sig = np.maximum(np.abs(audio) - mid_lp, 0.0)

    elif cfg.filter_type == "fft":
        # STFT-based band energy (not time-domain filters)
        seg_w = cfg.segment_width
        hop = int(seg_w * (1 - cfg.overlap)) if cfg.overlap > 0 else seg_w
        n_fft = 256
        from scipy.signal import stft
        _, _, Z = stft(audio, fs=sr, nperseg=seg_w, noverlap=seg_w - hop, nfft=n_fft)
        freqs = np.fft.rfftfreq(n_fft, 1.0 / sr)
        mag = np.abs(Z)

        # Weighting
        if cfg.weighting == "a_weight":
            weights = _a_weighting(freqs)
        elif cfg.weighting == "c_weight":
            weights = _c_weighting(freqs)
        elif cfg.weighting == "itu_r_468":
            weights = _itu_r_468_weighting(freqs)
        else:
            weights = np.ones_like(freqs)

        mag_weighted = mag * weights[:, None]

        low_mask = (freqs >= 0) & (freqs < cfg.low_cutoff)
        mid_mask = (freqs >= cfg.low_cutoff) & (freqs < cfg.mid_cutoff)
        high_mask = (freqs >= cfg.mid_cutoff)

        low_energy = np.sqrt(np.mean(mag_weighted[low_mask]**2, axis=0))
        mid_energy = np.sqrt(np.mean(mag_weighted[mid_mask]**2, axis=0))
        high_energy = np.sqrt(np.mean(mag_weighted[high_mask]**2, axis=0))

        # Convert STFT frames back to per-sample resolution for consistent measurement
        # Just repeat each frame value across its hop samples
        num_segments = len(audio) // seg_w
        low_sig = np.repeat(low_energy, hop)[:len(audio)]
        mid_sig = np.repeat(mid_energy, hop)[:len(audio)]
        high_sig = np.repeat(high_energy, hop)[:len(audio)]

        # Pad or truncate to match audio length
        for sig in (low_sig, mid_sig, high_sig):
            if len(sig) < len(audio):
                sig = np.pad(sig, (0, len(audio) - len(sig)), mode="edge")
            else:
                sig = sig[:len(audio)]
        return low_sig, mid_sig, high_sig

    else:
        raise ValueError(f"Unknown filter_type: {cfg.filter_type}")

    # Apply frequency weighting to time-domain signals via FFT if requested
    if cfg.weighting != "none" and cfg.filter_type != "fft":
        # Approximate: weight the power spectrum of each band
        # This is a simplified approach — for true weighting we'd need to
        # apply the weighting before filtering.
        pass  # Time-domain weighting is tricky; we'll rely on FFT mode for weighting tests

    return low_sig, mid_sig, high_sig


def _measure_segment(sig: NDArray, start: int, end: int, measure: str, sr: int) -> float:
    """Measure energy in a segment using specified method."""
    seg = sig[start:end]
    if len(seg) == 0:
        return 0.0

    if measure == "rms":
        return float(np.sqrt(np.mean(seg**2)))
    elif measure == "mean":
        return float(np.mean(seg))
    elif measure == "peak":
        return float(np.max(seg))
    elif measure == "true_peak":
        # 4x oversampling for inter-sample peaks
        from scipy.signal import resample
        oversampled = resample(seg, len(seg) * 4)
        return float(np.max(oversampled))
    else:
        raise ValueError(f"Unknown measure: {measure}")


def _apply_smoothing(raw_values: list[tuple[float, float, float]], cfg: HypothesisConfig, sr: int) -> list[tuple[float, float, float]]:
    """Apply multi-pass smoothing to raw segment values."""
    if cfg.smoothing == "none" or (cfg.smoothing == "mix" and cfg.mix_factor <= 0):
        return raw_values

    num_segments = len(raw_values)

    if cfg.smoothing == "mix":
        result = []
        prev = (0.0, 0.0, 0.0)
        for val in raw_values:
            mf = cfg.mix_factor
            smoothed = (
                prev[0] * mf + val[0] * (1 - mf),
                prev[1] * mf + val[1] * (1 - mf),
                prev[2] * mf + val[2] * (1 - mf),
            )
            result.append(smoothed)
            prev = smoothed
        return result

    elif cfg.smoothing == "block_max":
        w = cfg.block_max_width
        result = []
        for i in range(num_segments):
            window = raw_values[i:min(i + w, num_segments)]
            result.append((
                max(v[0] for v in window),
                max(v[1] for v in window),
                max(v[2] for v in window),
            ))
        return result

    elif cfg.smoothing == "envelope":
        # Forward-backward: look-ahead + look-behind smoothing
        lookahead = cfg.envelope_lookahead
        result = []
        for i in range(num_segments):
            start = max(0, i - lookahead)
            end = min(num_segments, i + lookahead + 1)
            window = raw_values[start:end]
            result.append((
                max(v[0] for v in window),
                max(v[1] for v in window),
                max(v[2] for v in window),
            ))
        return result

    elif cfg.smoothing == "compressor":
        # Simple attack/release envelope follower
        attack_coeff = 1.0 - np.exp(-1.0 / (sr * cfg.compressor_attack_ms / 1000.0)) if cfg.compressor_attack_ms > 0 else 1.0
        release_coeff = 1.0 - np.exp(-1.0 / (sr * cfg.compressor_release_ms / 1000.0)) if cfg.compressor_release_ms > 0 else 1.0
        # Actually the coeff should be per-segment, not per-sample
        # Simplified: treat as per-segment
        seg_per_sec = sr / cfg.segment_width
        attack_coeff = 1.0 - np.exp(-seg_per_sec * cfg.compressor_attack_ms / 1000.0) if cfg.compressor_attack_ms > 0 else 1.0
        release_coeff = 1.0 - np.exp(-seg_per_sec * cfg.compressor_release_ms / 1000.0) if cfg.compressor_release_ms > 0 else 1.0

        result = []
        env = [0.0, 0.0, 0.0]
        for val in raw_values:
            for band in range(3):
                v = val[band]
                coeff = attack_coeff if v > env[band] else release_coeff
                env[band] = env[band] + coeff * (v - env[band])
            result.append(tuple(env))
        return result

    return raw_values


def _apply_normalization(
    values: list[tuple[float, float, float]],
    cfg: HypothesisConfig,
) -> NDArray[np.float64]:
    """Apply normalization and convert to output array.

    CRITICAL: Power compression is applied BEFORE gain, matching production:
        production:  gain * (value ** power)
        NOT:        (gain * value) ** power
    """
    arr = np.array(values, dtype=np.float64)

    if cfg.normalize == "sigmoid":
        arr[:, 0] *= cfg.gain_low
        arr[:, 1] *= cfg.gain_mid
        arr[:, 2] *= cfg.gain_high
        # Soft-clip: sigmoid-ish compression near top
        arr = 127.0 * (arr / (arr + 50.0)) * 2.0  # maps ~0-100 to ~0-127
    else:
        # Per-track normalization (if any) happens before power+gain
        if cfg.normalize == "track_peak":
            peak = np.max(arr)
            if peak > 1e-10:
                arr = arr / peak * 100.0
        elif cfg.normalize == "track_loudness":
            full_band = np.sum(arr, axis=1)
            loudness = np.sqrt(np.mean(full_band**2))
            if loudness > 1e-10:
                arr = arr / loudness * 50.0

        # Power-law compression FIRST (same as production)
        arr = arr ** cfg.power

        # Fixed gains SECOND (same as production)
        arr[:, 0] *= cfg.gain_low
        arr[:, 1] *= cfg.gain_mid
        arr[:, 2] *= cfg.gain_high

    # Clamp to 0-127
    arr = np.clip(arr, 0, 127)

    return arr / 127.0  # Normalize for comparison


def generate_pwv7_hypothesis(
    audio: NDArray,
    sr: int,
    cfg: HypothesisConfig,
) -> NDArray[np.float64]:
    """Generate PWV7-style waveform using a hypothesis configuration.

    Returns (N, 3) float64 array normalized to 0.0–1.0.
    """
    # Resample if needed
    if sr != cfg.target_sr:
        from fourfour_analysis.audio_io import resample_audio
        audio, sr = resample_audio(audio, sr, cfg.target_sr)

    seg_w = cfg.segment_width
    if len(audio) < seg_w:
        return np.zeros((0, 3), dtype=np.float64)

    # Apply filters
    low_sig, mid_sig, high_sig = _apply_filters(audio, sr, cfg)

    # Segment and measure
    hop = int(seg_w * (1 - cfg.overlap)) if cfg.overlap > 0 else seg_w
    num_segments = (len(audio) - seg_w) // hop + 1 if cfg.overlap > 0 else len(audio) // seg_w

    raw_values: list[tuple[float, float, float]] = []
    for i in range(num_segments):
        if cfg.overlap > 0:
            start = i * hop
            end = start + seg_w
        else:
            start = i * seg_w
            end = start + seg_w

        if end > len(audio):
            break

        l = _measure_segment(low_sig, start, end, cfg.measure, sr)
        m = _measure_segment(mid_sig, start, end, cfg.measure, sr)
        h = _measure_segment(high_sig, start, end, cfg.measure, sr)
        raw_values.append((l, m, h))

    # Peak-hold: forward-looking max (production uses hold=3 → window of 4)
    if cfg.peak_hold > 1:
        held = []
        n = len(raw_values)
        for i in range(n):
            window = raw_values[i:min(i + cfg.peak_hold + 1, n)]
            held.append((
                max(v[0] for v in window),
                max(v[1] for v in window),
                max(v[2] for v in window),
            ))
        raw_values = held

    # Multi-pass smoothing
    smoothed = _apply_smoothing(raw_values, cfg, sr)

    # Normalization + compression
    result = _apply_normalization(smoothed, cfg)

    return result


# ── Hypothesis grid ────────────────────────────────────────────────────────

def build_hypothesis_grid() -> list[HypothesisConfig]:
    """Build a focused grid of hypotheses to test."""
    configs: list[HypothesisConfig] = []

    # Base reference
    base = HypothesisConfig()

    # ── Hypothesis 1: Filter topology ──────────────────────────────
    for filt in ["butterworth", "linkwitz_riley", "shelving", "fft"]:
        if filt == "shelving":
            configs.append(HypothesisConfig(filter_type=filt, filter_order=1))
        elif filt == "fft":
            configs.append(HypothesisConfig(filter_type=filt, filter_order=0))
        else:
            configs.append(HypothesisConfig(filter_type=filt))

    # ── Hypothesis 2: Perceptual weighting ─────────────────────────
    for w in ["none", "a_weight", "c_weight", "itu_r_468"]:
        configs.append(HypothesisConfig(weighting=w, filter_type="fft"))

    # ── Hypothesis 3: Measurement method ───────────────────────────
    for m in ["rms", "peak", "true_peak", "mean"]:
        configs.append(HypothesisConfig(measure=m))

    # ── Hypothesis 4: Multi-pass processing ────────────────────────
    # No smoothing
    configs.append(HypothesisConfig(smoothing="none", mix_factor=0.0))
    # Simple mix (various factors)
    for mf in [0.05, 0.1, 0.2, 0.3, 0.5]:
        configs.append(HypothesisConfig(smoothing="mix", mix_factor=mf))
    # Block max
    for w in [2, 3, 4, 5]:
        configs.append(HypothesisConfig(smoothing="block_max", block_max_width=w))
    # Envelope
    for la in [1, 2, 3, 5, 8]:
        configs.append(HypothesisConfig(smoothing="envelope", envelope_lookahead=la))
    # Compressor
    configs.append(HypothesisConfig(
        smoothing="compressor",
        compressor_attack_ms=1.0,
        compressor_release_ms=100.0,
    ))
    configs.append(HypothesisConfig(
        smoothing="compressor",
        compressor_attack_ms=5.0,
        compressor_release_ms=200.0,
    ))

    # ── Hypothesis 5: Normalization ────────────────────────────────
    for norm in ["fixed", "track_peak", "track_loudness", "sigmoid"]:
        configs.append(HypothesisConfig(normalize=norm))

    # Power-law variants
    for p in [0.3, 0.4, 0.5, 0.6, 0.7, 1.0]:
        configs.append(HypothesisConfig(power=p))

    # Gain variants
    for gl, gm, gh in [
        (140, 120, 70),
        (180, 100, 60),
        (120, 140, 80),
        (160, 110, 65),
        (200, 90, 50),
    ]:
        configs.append(HypothesisConfig(gain_low=gl, gain_mid=gm, gain_high=gh))

    # ── Hypothesis 6: Sample rate / window ─────────────────────────
    # Different segment widths at 12kHz
    for sw in [40, 60, 80, 100, 120, 160]:
        configs.append(HypothesisConfig(segment_width=sw))
    # Overlap
    for ov in [0.0, 0.25, 0.5, 0.75]:
        configs.append(HypothesisConfig(overlap=ov))
    # 22kHz target
    for sw in [80, 147, 160]:
        configs.append(HypothesisConfig(target_sr=22000, segment_width=sw))

    # Crossover frequency variants
    for low_c in [100, 130, 160, 200]:
        for mid_c in [2000, 2500, 3000, 4000]:
            configs.append(HypothesisConfig(low_cutoff=low_c, mid_cutoff=mid_c))

    # Peak-hold variants (production uses peak_hold=3)
    for ph in [2, 3, 4, 5, 8]:
        configs.append(HypothesisConfig(peak_hold=ph))
    # Peak-hold + power combinations
    for ph in [2, 3, 5]:
        for p in [0.5, 0.7, 1.0]:
            configs.append(HypothesisConfig(peak_hold=ph, power=p))

    # Remove duplicates by converting to ID and back
    seen: set[str] = set()
    unique = []
    for c in configs:
        cid = c.id()
        if cid not in seen:
            seen.add(cid)
            unique.append(c)

    return unique


def _generate_production_waveform(audio: NDArray, sr: int) -> NDArray[np.float64]:
    """Generate waveform using the exact current production code."""
    from fourfour_analysis.audio_io import preprocess_waveform
    waveform_audio, waveform_sr = preprocess_waveform(audio, sr)
    columns = generate_waveform_filterbank(waveform_audio, waveform_sr)
    # Convert to normalized (N, 3) array
    arr = np.array([[c.r, c.g, c.b] for c in columns], dtype=np.float64) / 127.0
    return arr


def run_hypothesis_test(
    audio_paths: list[Path],
    configs: Optional[list[HypothesisConfig]] = None,
    progress_fn: Optional[Callable[[str], None]] = None,
    include_production: bool = True,
) -> list[HypothesisResult]:
    """Run all hypotheses against a list of audio files with Rekordbox references.

    Returns results sorted by aggregate MAE (best first).
    """
    if configs is None:
        configs = build_hypothesis_grid()

    # Load references
    references: list[tuple[Path, NDArray, Optional[NDArray]]] = []
    for ap in audio_paths:
        ex_path = find_rekordbox_anlz(ap)
        if ex_path is None:
            if progress_fn:
                progress_fn(f"Skipping {ap.name}: no Rekordbox reference found")
            continue
        try:
            detail, overview = read_pwv7(ex_path)
            references.append((ap, detail, overview))
            if progress_fn:
                progress_fn(f"Loaded reference: {ap.name} ({len(detail)} cols)")
        except Exception as e:
            if progress_fn:
                progress_fn(f"Error reading {ap.name}: {e}")
            continue

    if not references:
        raise ValueError("No valid reference tracks found")

    results: list[HypothesisResult] = []
    total_configs = len(configs)

    for ci, cfg in enumerate(configs):
        if progress_fn:
            progress_fn(f"[{ci+1}/{total_configs}] Testing {cfg.id()[:80]}...")

        result = HypothesisResult(
            hypothesis_id=cfg.id(),
            description=_describe_config(cfg),
            config=asdict(cfg),
        )

        for ap, ref_detail, _ref_overview in references:
            try:
                audio, sr = load_audio(str(ap))
                audio_mono = audio.mean(axis=0) if audio.ndim > 1 else audio

                ours = generate_pwv7_hypothesis(audio_mono, sr, cfg)

                if len(ours) == 0:
                    continue

                bands = score_waveform(ours, ref_detail)
                result.track_scores.append(TrackScore(
                    track_path=str(ap),
                    num_columns=len(ref_detail),
                    bands=bands,
                ))
            except Exception as e:
                if progress_fn:
                    progress_fn(f"  Error on {ap.name}: {e}")
                continue

        result.compute_aggregate()
        results.append(result)

    # Add production baseline comparison
    if include_production:
        if progress_fn:
            progress_fn("[production] Testing current production code...")
        prod_result = HypothesisResult(
            hypothesis_id="production",
            description="Current production FilterbankParams",
            config={"source": "lexicon_waveform.generate_waveform_filterbank"},
        )
        for ap, ref_detail, _ref_overview in references:
            try:
                audio, sr = load_audio(str(ap))
                audio_mono = audio.mean(axis=0) if audio.ndim > 1 else audio
                ours = _generate_production_waveform(audio_mono, sr)
                if len(ours) == 0:
                    continue
                bands = score_waveform(ours, ref_detail)
                prod_result.track_scores.append(TrackScore(
                    track_path=str(ap),
                    num_columns=len(ref_detail),
                    bands=bands,
                ))
            except Exception as e:
                if progress_fn:
                    progress_fn(f"  Error on {ap.name}: {e}")
                continue
        prod_result.compute_aggregate()
        results.append(prod_result)

    # Sort by aggregate MAE ascending, then correlation descending
    results.sort(key=lambda r: (r.aggregate_mae, -r.aggregate_corr))
    return results


def _describe_config(cfg: HypothesisConfig) -> str:
    """Human-readable description of a config."""
    parts = []
    parts.append(f"Filter: {cfg.filter_type} (order={cfg.filter_order})")
    parts.append(f"Crossover: {cfg.low_cutoff:.0f}/{cfg.mid_cutoff:.0f} Hz")
    if cfg.weighting != "none":
        parts.append(f"Weighting: {cfg.weighting}")
    parts.append(f"Measure: {cfg.measure}")
    if cfg.smoothing != "none":
        parts.append(f"Smoothing: {cfg.smoothing}")
    if cfg.peak_hold > 1:
        parts.append(f"Peak-hold: {cfg.peak_hold}")
    parts.append(f"Norm: {cfg.normalize}")
    parts.append(f"Power: {cfg.power}")
    parts.append(f"SR={cfg.target_sr}, seg={cfg.segment_width}, ov={cfg.overlap}")
    return "; ".join(parts)


def print_results(results: list[HypothesisResult], top_n: int = 20) -> None:
    """Print a formatted table of hypothesis results."""
    print(f"\n{'='*100}")
    print(f"PWV7 Hypothesis Test Results (top {top_n} of {len(results)})")
    print(f"{'='*100}")
    print(f"{'Rank':>5}  {'MAE':>8}  {'Corr':>8}  {'Tracks':>7}  {'Description'}")
    print(f"{'-'*100}")

    for i, r in enumerate(results[:top_n], 1):
        print(f"{i:>5}  {r.aggregate_mae:>8.4f}  {r.aggregate_corr:>8.4f}  {r.num_tracks:>7}  {r.description}")

    # Also print the baseline (current default) and production
    baseline_id = HypothesisConfig().id()
    baseline = next((r for r in results if r.hypothesis_id == baseline_id), None)
    if baseline:
        rank = next(i for i, r in enumerate(results, 1) if r.hypothesis_id == baseline_id)
        print(f"\n  Hypothesis baseline rank: #{rank}  MAE={baseline.aggregate_mae:.4f}  Corr={baseline.aggregate_corr:.4f}")

    prod = next((r for r in results if r.hypothesis_id == "production"), None)
    if prod:
        rank = next(i for i, r in enumerate(results, 1) if r.hypothesis_id == "production")
        print(f"  Production code rank:   #{rank}  MAE={prod.aggregate_mae:.4f}  Corr={prod.aggregate_corr:.4f}")

    print(f"{'='*100}")


def print_band_breakdown(results: list[HypothesisResult], top_n: int = 10) -> None:
    """Print per-band MAE breakdown for top results."""
    print(f"\n{'='*110}")
    print(f"Per-band breakdown (top {top_n})")
    print(f"{'='*110}")
    print(f"{'Rank':>5}  {'MAE':>8}  {'Low':>8}  {'Mid':>8}  {'High':>8}  {'cL':>7}  {'cM':>7}  {'cH':>7}  {'Description'}")
    print(f"{'-'*110}")

    for i, r in enumerate(results[:top_n], 1):
        if not r.track_scores:
            continue
        # Aggregate per-band
        total_cols = sum(s.num_columns for s in r.track_scores)
        mae_l = sum(s.bands.mae_low * s.num_columns for s in r.track_scores) / total_cols
        mae_m = sum(s.bands.mae_mid * s.num_columns for s in r.track_scores) / total_cols
        mae_h = sum(s.bands.mae_high * s.num_columns for s in r.track_scores) / total_cols
        corr_l = sum(s.bands.corr_low * s.num_columns for s in r.track_scores) / total_cols
        corr_m = sum(s.bands.corr_mid * s.num_columns for s in r.track_scores) / total_cols
        corr_h = sum(s.bands.corr_high * s.num_columns for s in r.track_scores) / total_cols
        print(f"{i:>5}  {r.aggregate_mae:>8.4f}  {mae_l:>8.4f}  {mae_m:>8.4f}  {mae_h:>8.4f}  "
              f"{corr_l:>7.3f}  {corr_m:>7.3f}  {corr_h:>7.3f}  {r.description[:50]}")

    print(f"{'='*110}")


def export_results_json(results: list[HypothesisResult], path: Path) -> None:
    """Export all results to JSON for further analysis."""
    data = []
    for r in results:
        data.append({
            "hypothesis_id": r.hypothesis_id,
            "description": r.description,
            "config": r.config,
            "aggregate_mae": r.aggregate_mae,
            "aggregate_corr": r.aggregate_corr,
            "num_tracks": r.num_tracks,
            "track_scores": [
                {
                    "track": s.track_path,
                    "columns": s.num_columns,
                    "mae": s.bands.mae,
                    "correlation": s.bands.correlation,
                    "mae_low": s.bands.mae_low,
                    "mae_mid": s.bands.mae_mid,
                    "mae_high": s.bands.mae_high,
                }
                for s in r.track_scores
            ],
        })
    path.write_text(json.dumps(data, indent=2))

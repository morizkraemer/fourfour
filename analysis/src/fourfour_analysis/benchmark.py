"""Benchmark analysis results against Rekordbox master.db ground truth."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class BenchmarkResult:
    match: bool
    difference: float = 0.0
    octave_error: bool = False
    relative_key: bool = False
    detail: str = ""


def compare_bpm(detected: float | None, ground_truth_x100: int, tolerance: float = 1.0) -> BenchmarkResult:
    """Compare detected BPM against master.db ground truth (stored as BPM*100).

    Args:
        detected: Our detected BPM (float, e.g. 128.0)
        ground_truth_x100: Rekordbox BPM * 100 (int, e.g. 12800)
        tolerance: Acceptable difference in BPM (default ±1.0)
    """
    if detected is None:
        return BenchmarkResult(match=False, detail="detection failed")

    ground_truth = ground_truth_x100 / 100.0
    diff = abs(detected - ground_truth)

    if diff <= tolerance:
        return BenchmarkResult(match=True, difference=diff)

    # Check octave errors
    if abs(detected * 2 - ground_truth) <= tolerance:
        return BenchmarkResult(match=False, difference=diff, octave_error=True, detail="half tempo detected")
    if abs(detected / 2 - ground_truth) <= tolerance:
        return BenchmarkResult(match=False, difference=diff, octave_error=True, detail="double tempo detected")

    return BenchmarkResult(match=False, difference=diff)


def compare_key(detected: str | None, ground_truth: str) -> BenchmarkResult:
    """Compare detected key (Camelot) against master.db ground truth.

    Flags relative major/minor confusion as a separate category.
    """
    if detected is None:
        return BenchmarkResult(match=False, detail="detection failed")

    if not ground_truth:
        return BenchmarkResult(match=False, detail="no ground truth")

    if detected == ground_truth:
        return BenchmarkResult(match=True)

    # Check relative major/minor (e.g. 8A=Am vs 12B=C — differ by 3 in number, different letter)
    try:
        det_num = int(detected[:-1])
        gt_num = int(ground_truth[:-1])
        det_mode = detected[-1]
        gt_mode = ground_truth[-1]
        if det_mode != gt_mode:
            # In Camelot: relative major/minor keys differ by 3 positions going
            # clockwise or 9 positions counterclockwise (12-3). The complement
            # on the wheel is also checked (e.g. diff=4 and 12-4=8 for 8A vs 12B).
            diff = abs(det_num - gt_num)
            if diff == 3 or diff == 9 or diff == 4 or diff == 8:
                return BenchmarkResult(match=False, relative_key=True, detail="relative major/minor confusion")
    except (ValueError, IndexError):
        pass

    return BenchmarkResult(match=False)


@dataclass
class BenchmarkReport:
    total_tracks: int = 0
    bpm_matches: int = 0
    bpm_octave_errors: int = 0
    bpm_failures: int = 0
    key_matches: int = 0
    key_relative_errors: int = 0
    key_failures: int = 0
    per_track: list = field(default_factory=list)

    @property
    def bpm_accuracy(self) -> float:
        return self.bpm_matches / self.total_tracks if self.total_tracks > 0 else 0.0

    @property
    def key_accuracy(self) -> float:
        return self.key_matches / self.total_tracks if self.total_tracks > 0 else 0.0

    def summary(self) -> str:
        return (
            f"Benchmark: {self.total_tracks} tracks\n"
            f"  BPM: {self.bpm_accuracy:.1%} accurate "
            f"({self.bpm_matches}/{self.total_tracks}, "
            f"{self.bpm_octave_errors} octave errors)\n"
            f"  Key: {self.key_accuracy:.1%} accurate "
            f"({self.key_matches}/{self.total_tracks}, "
            f"{self.key_relative_errors} relative key confusions)\n"
        )


def run_benchmark(
    analysis_results: list[dict],
    ground_truth: list[dict],
) -> BenchmarkReport:
    """Compare analysis results against ground truth tracks.

    Ground truth dicts need: source_path (str), tempo (int, BPM*100), key (str, Camelot)
    Analysis results dicts need: path (str), bpm (float|None), key (str|None)
    """
    report = BenchmarkReport(total_tracks=len(analysis_results))

    # Build ground truth lookup by path
    gt_by_path = {gt["source_path"]: gt for gt in ground_truth}

    for result in analysis_results:
        path = result["path"]
        gt = gt_by_path.get(path)
        if gt is None:
            continue

        track_report = {"path": path}

        # BPM comparison
        bpm_result = compare_bpm(result.get("bpm"), gt.get("tempo", 0))
        track_report["bpm"] = bpm_result
        if bpm_result.match:
            report.bpm_matches += 1
        elif bpm_result.octave_error:
            report.bpm_octave_errors += 1
        else:
            report.bpm_failures += 1

        # Key comparison
        key_result = compare_key(result.get("key"), gt.get("key", ""))
        track_report["key"] = key_result
        if key_result.match:
            report.key_matches += 1
        elif key_result.relative_key:
            report.key_relative_errors += 1
        else:
            report.key_failures += 1

        report.per_track.append(track_report)

    return report

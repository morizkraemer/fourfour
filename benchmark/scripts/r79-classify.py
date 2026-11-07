import json
import statistics
import sys

RATIOS = [(2.0, "octave x2"), (0.5, "octave x0.5"), (1.5, "x3/2"), (2 / 3, "x2/3"), (48000 / 44100, "time base 48k/44.1k"), (44100 / 48000, "time base 44.1k/48k")]


def constant_grid(beats_ms):
    if len(beats_ms) < 3:
        return False
    n = len(beats_ms) - 1
    p = (beats_ms[-1] - beats_ms[0]) / n
    return all(abs(t - (beats_ms[0] + p * i)) <= 5 for i, t in enumerate(beats_ms))


def quantized_10ms(bpm):
    period = 60000 / bpm
    return any(abs(round(60000 / (10 * j), 2) - bpm) < 0.006 for j in range(int(period / 10) - 1, int(period / 10) + 2))


def grid_tempo(e):
    return statistics.median(t / 100 for t in e["tempo_x100"])


def classify(bpm, e):
    ref = e["bpm_masterdb"]
    gt = grid_tempo(e)
    dynamic = not constant_grid(e["beats_ms"])
    for target in ([ref, gt] if dynamic else [ref]):
        for k, name in RATIOS:
            if abs(bpm / target - k) < 0.03 * k:
                return name + (" (dynamic ref)" if dynamic else ""), gt
    if dynamic:
        if abs(bpm - gt) <= 0.1:
            return "dynamic ref, ours = RB grid tempo, master.db BPM differs" + (" (10 ms quantized)" if quantized_10ms(ref) else ""), gt
        return "dynamic ref, ours != RB grid tempo either", gt
    if abs(bpm - ref) <= 1.0:
        return "near-miss (constant ref)", gt
    return "other (constant ref)", gt


def main():
    ref = json.load(open(sys.argv[1]))
    for path in sys.argv[2:]:
        rows = [json.loads(line) for line in open(path)]
        misses = [r for r in rows if abs(r["bpm"] - r["ref_bpm"]) > 0.1]
        alt = sum(abs(r["bpm"] - grid_tempo(ref[str(r["id"])])) <= 0.1 for r in rows)
        print(f"== {path}: {len(rows) - len(misses)}/{len(rows)} within 0.1 of master.db, {alt}/{len(rows)} within 0.1 of RB grid median tempo")
        for r in sorted(misses, key=lambda r: r["id"]):
            e = ref[str(r["id"])]
            cls, gt = classify(r["bpm"], e)
            print(f"  {r['id']:>4} ours {r['bpm']:8.3f} master.db {r['ref_bpm']:7.2f} grid median {gt:7.2f}  {cls}")


main()

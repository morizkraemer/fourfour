#!/usr/bin/env python3
"""Time one Beat This! chunk (1500 frames x 128 mels) in ONNX Runtime.

usage: q26-ort-chunk-bench.py <model.onnx> <threads> <batch> [runs] [graph_opt: all|basic|none]
Prints wall and CPU seconds per chunk (CPU = process user+sys, so it includes helper threads).
"""
import os
import sys
import time

import numpy as np
import onnxruntime as ort

model, threads, batch = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
runs = int(sys.argv[4]) if len(sys.argv) > 4 else 3
opt = sys.argv[5] if len(sys.argv) > 5 else "all"

so = ort.SessionOptions()
so.intra_op_num_threads = threads
so.inter_op_num_threads = 1
so.graph_optimization_level = {
    "all": ort.GraphOptimizationLevel.ORT_ENABLE_ALL,
    "basic": ort.GraphOptimizationLevel.ORT_ENABLE_BASIC,
    "none": ort.GraphOptimizationLevel.ORT_DISABLE_ALL,
}[opt]
so.log_severity_level = 3
sess = ort.InferenceSession(model, so, providers=["CPUExecutionProvider"])
rng = np.random.default_rng(0)
x = rng.normal(0.0, 1.0, size=(batch, 1500, 128)).astype(np.float32)
sess.run(None, {"spectrogram": x})
w0, c0 = time.perf_counter(), sum(os.times()[:2])
for _ in range(runs):
    sess.run(None, {"spectrogram": x})
w1, c1 = time.perf_counter(), sum(os.times()[:2])
n = runs * batch
print(f"{os.path.basename(model)} threads={threads} batch={batch} opt={opt}: wall {(w1 - w0) / n:.3f} s/chunk, cpu {(c1 - c0) / n:.3f} s/chunk")

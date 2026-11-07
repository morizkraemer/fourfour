#!/usr/bin/env python3
"""Offline tool: dynamic int8 weight quantization of beat_this.onnx (MatMul with constant weights).

usage: quantize_beat_int8.py <in.onnx> <out.onnx> [--weight-type s8|u8] [--per-channel] [--ops MatMul,Gemm,Conv]

Needs onnx and onnxruntime (MIT) in a throwaway venv; never shipped in the app.
"""
import argparse
import tempfile
from pathlib import Path

from onnxruntime.quantization import QuantType, quantize_dynamic
from onnxruntime.quantization.shape_inference import quant_pre_process


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("src")
    ap.add_argument("dst")
    ap.add_argument("--weight-type", choices=["s8", "u8"], default="s8")
    ap.add_argument("--per-channel", action="store_true")
    ap.add_argument("--ops", default="MatMul")
    args = ap.parse_args()
    weight_type = QuantType.QInt8 if args.weight_type == "s8" else QuantType.QUInt8
    with tempfile.TemporaryDirectory() as tmp:
        prepared = str(Path(tmp) / "pre.onnx")
        quant_pre_process(args.src, prepared, skip_symbolic_shape=True, skip_optimization=True)
        quantize_dynamic(
            prepared,
            args.dst,
            op_types_to_quantize=args.ops.split(","),
            weight_type=weight_type,
            per_channel=args.per_channel,
        )


if __name__ == "__main__":
    main()

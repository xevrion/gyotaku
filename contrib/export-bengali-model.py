#!/usr/bin/env python3
"""Builds bengali_easyocr_rec.onnx, the recognizer behind `scripts = ["bengali"]`.

PaddleOCR has no Bengali recognizer, so this takes EasyOCR's (JaidedAI,
Apache-2.0) and exports it in the shape crates/ocr/src/rec.rs already reads:

    input   x      batch x 3 x 48 x width, RGB scaled to -1..1
    output  probs  batch x steps x classes, softmax, class 0 the CTC blank
    metadata "character": the alphabet, one character per line, no blank

The model itself wants 64 px tall greyscale lines; that conversion is folded
into the graph, so nothing in the Rust code is specific to it. The weights are
then quantized to 8 bits, which takes the file from 215 MB to 54 MB.

    pip install easyocr onnx onnxruntime
    python contrib/export-bengali-model.py bengali_easyocr_rec.onnx

After a change here, update the sha256 in crates/ocr/src/models.rs and upload
the new file to the models release.
"""

import hashlib
import os
import sys
import tempfile

import easyocr
import onnx
import torch
import torch.nn as nn
import torch.nn.functional as F
from onnxruntime.quantization import QuantType, quantize_dynamic


class AsPaddle(nn.Module):
    def __init__(self, net):
        super().__init__()
        self.net = net

    def forward(self, x):
        grey = 0.299 * x[:, 0:1] + 0.587 * x[:, 1:2] + 0.114 * x[:, 2:3]
        grey = F.interpolate(grey, scale_factor=64 / 48, mode="bilinear", align_corners=False)
        # The model's own forward, with its pooling over the three rows that
        # are left written as a mean, so it exports at any width.
        seen = self.net.FeatureExtraction(grey).permute(0, 3, 1, 2).mean(3)
        return F.softmax(self.net.Prediction(self.net.SequenceModeling(seen).contiguous()), dim=2)


def main(out):
    reader = easyocr.Reader(["bn", "en"], gpu=False, quantize=False, verbose=False)
    net = getattr(reader.recognizer, "module", reader.recognizer)
    alphabet = list(reader.converter.character)
    assert alphabet[0] == "[blank]"
    alphabet = alphabet[1:]
    assert all(len(c) == 1 and c not in "\r\n" for c in alphabet)

    model = AsPaddle(net).eval()
    with tempfile.TemporaryDirectory() as tmp:
        full = os.path.join(tmp, "full.onnx")
        torch.onnx.export(
            model,
            (torch.zeros(2, 3, 48, 320),),
            full,
            input_names=["x"],
            output_names=["probs"],
            opset_version=17,
            dynamo=False,
            dynamic_axes={"x": {0: "batch", 3: "width"}, "probs": {0: "batch", 1: "steps"}},
        )
        quantize_dynamic(full, out, weight_type=QuantType.QUInt8)

    graph = onnx.load(out)
    for key, value in (
        ("character", "\n".join(alphabet)),
        ("source", "EasyOCR bengali.pth (JaidedAI), exported for gyotaku"),
        ("license", "Apache-2.0"),
    ):
        prop = graph.metadata_props.add()
        prop.key, prop.value = key, value
    onnx.save(graph, out)

    with open(out, "rb") as f:
        print(hashlib.sha256(f.read()).hexdigest(), out)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "bengali_easyocr_rec.onnx")

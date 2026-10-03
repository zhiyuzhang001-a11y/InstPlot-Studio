#!/usr/bin/env python3
"""Encode the existing product PNG as a multi-size ICO (macOS asset maintenance)."""

from pathlib import Path
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "apps/instplot-studio/assets"
SIZES = (16, 24, 32, 48, 64, 128, 256)


def main() -> None:
    images = []
    with tempfile.TemporaryDirectory() as temporary:
        for size in SIZES:
            output = Path(temporary) / f"{size}.png"
            subprocess.run([
                "sips", "-z", str(size), str(size), str(ASSETS / "InstPlotStudio.png"),
                "--out", str(output),
            ], check=True, capture_output=True)
            images.append(output.read_bytes())
    header = struct.pack("<HHH", 0, 1, len(images))
    entries = bytearray()
    offset = len(header) + 16 * len(images)
    for size, data in zip(SIZES, images):
        dimension = size if size < 256 else 0
        entries.extend(struct.pack("<BBBBHHII", dimension, dimension, 0, 0, 1, 32, len(data), offset))
        offset += len(data)
    (ASSETS / "InstPlotStudio.ico").write_bytes(header + entries + b"".join(images))


if __name__ == "__main__":
    main()

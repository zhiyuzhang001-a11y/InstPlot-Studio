#!/usr/bin/env python3
"""Create the strict multi-platform update asset specification from Release files."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def build_spec(asset_dir: Path, version: str) -> dict[str, object]:
    definitions = {
        "linux-x86_64": (
            "deb",
            [
                ("deb", "deb", f"InstPlot-Studio-{version}-linux-x86_64.deb", "Ubuntu 22.04 or compatible"),
                ("portable", "tar-gz", f"InstPlot-Studio-{version}-linux-x86_64.tar.gz", "glibc 2.35 or newer"),
            ],
        ),
        "macos-aarch64": (
            "dmg",
            [("dmg", "dmg", f"InstPlot-Studio-{version}-macos-aarch64.dmg", "macOS 12 arm64 or newer")],
        ),
        "windows-x86_64": (
            "inno-setup",
            [
                (
                    "inno-setup",
                    "exe-installer",
                    f"InstPlot-Studio-{version}-windows-x86_64-setup.exe",
                    "Windows 10 x64 or newer",
                )
            ],
        ),
    }
    expected = {filename for _, packages in definitions.values() for _, _, filename, _ in packages}
    actual = {
        path.name
        for path in asset_dir.iterdir()
        if path.is_file() and path.name != "SHA256SUMS.txt"
    }
    if actual != expected:
        missing = sorted(expected - actual)
        unexpected = sorted(actual - expected)
        raise ValueError(f"release asset mismatch; missing={missing}, unexpected={unexpected}")
    platforms: dict[str, object] = {}
    for platform, (preferred, packages) in definitions.items():
        platforms[platform] = {
            "preferred": preferred,
            "packages": [
                {
                    "id": package_id,
                    "minimum_system": minimum_system,
                    "package_type": package_type,
                    "path": str((asset_dir / filename).resolve()),
                }
                for package_id, package_type, filename, minimum_system in packages
            ],
        }
    return {"platforms": platforms}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--asset-dir", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        payload = build_spec(args.asset_dir.resolve(), args.version)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    args.output.write_text(
        json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

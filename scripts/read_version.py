#!/usr/bin/env python3
"""Print the InstPlot Studio version reported by Cargo metadata."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PACKAGE_NAME = "instplot-studio"


def workspace_version() -> str:
    completed = subprocess.run(
        [
            os.environ.get("CARGO", "cargo"),
            "metadata",
            "--locked",
            "--no-deps",
            "--format-version",
            "1",
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    payload = json.loads(completed.stdout)
    versions = {
        package.get("version")
        for package in payload.get("packages", [])
        if package.get("name") == PACKAGE_NAME
    }
    if len(versions) != 1:
        raise ValueError(f"expected exactly one {PACKAGE_NAME!r} package in Cargo metadata")
    version = versions.pop()
    if not isinstance(version, str) or not version.strip():
        raise ValueError(f"{PACKAGE_NAME!r} has no valid Cargo package version")
    return version.strip()


def main() -> int:
    try:
        print(workspace_version())
    except (OSError, subprocess.SubprocessError, json.JSONDecodeError, ValueError) as error:
        print(f"Unable to read product version: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

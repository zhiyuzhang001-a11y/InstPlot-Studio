#!/usr/bin/env python3
"""Print the InstPlot Studio workspace version from the root Cargo manifest."""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "Cargo.toml"


def workspace_version() -> str:
    payload = tomllib.loads(MANIFEST.read_text(encoding="utf-8"))
    version = payload.get("workspace", {}).get("package", {}).get("version")
    if not isinstance(version, str) or not version.strip():
        raise ValueError("workspace.package.version is missing from Cargo.toml")
    return version.strip()


def main() -> int:
    try:
        print(workspace_version())
    except (OSError, tomllib.TOMLDecodeError, ValueError) as error:
        print(f"Unable to read product version: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

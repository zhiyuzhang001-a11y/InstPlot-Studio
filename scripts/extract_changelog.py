#!/usr/bin/env python3
"""Extract one release section from CHANGELOG.md without guessing boundaries."""

from __future__ import annotations

import argparse
import re
from pathlib import Path


def extract_section(text: str, version: str) -> str:
    heading = re.compile(
        rf"^## \[{re.escape(version)}\](?:\s+-\s+\d{{4}}-\d{{2}}-\d{{2}})?\s*$",
        re.MULTILINE,
    )
    match = heading.search(text)
    if match is None:
        raise ValueError(f"CHANGELOG.md has no section for {version}")
    next_heading = re.search(r"^## \[", text[match.end() :], re.MULTILINE)
    end = match.end() + next_heading.start() if next_heading else len(text)
    body = text[match.end() : end].strip()
    if not body:
        raise ValueError(f"CHANGELOG.md section for {version} is empty")
    return body + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("version")
    parser.add_argument("--changelog", type=Path, default=Path("CHANGELOG.md"))
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        section = extract_section(args.changelog.read_text(encoding="utf-8"), args.version)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    if args.output:
        args.output.write_text(section, encoding="utf-8")
    else:
        print(section, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

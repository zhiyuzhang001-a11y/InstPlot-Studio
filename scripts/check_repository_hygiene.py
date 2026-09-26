#!/usr/bin/env python3
"""Reject machine-local, generated, secret-looking, or oversized tracked files."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MAX_TRACKED_FILE_BYTES = 5 * 1024 * 1024
FORBIDDEN_PATH_PARTS = {"target", ".agent-token-manager", "tmp", "__pycache__"}
FORBIDDEN_NAMES = {".DS_Store"}
FORBIDDEN_SUFFIXES = {".log", ".pyc"}
TEXT_SCAN_LIMIT = 2 * 1024 * 1024
PRIVATE_PATHS = (
    re.compile(rb"/Users/[A-Za-z0-9._-]+/"),
    re.compile(rb"/home/[A-Za-z0-9._-]+/"),
    re.compile(rb"[A-Za-z]:\\Users\\[A-Za-z0-9._-]+\\"),
)
SECRET_MARKERS = (
    b"BEGIN " + b"OPENSSH PRIVATE KEY",
    b"BEGIN " + b"RSA PRIVATE KEY",
    b"BEGIN " + b"EC PRIVATE KEY",
    b"github" + b"_pat_",
    b"gh" + b"p_",
)


def tracked_files() -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
    )
    return [Path(item.decode("utf-8")) for item in result.stdout.split(b"\0") if item]


def main() -> int:
    failures: list[str] = []
    for relative in tracked_files():
        if any(part in FORBIDDEN_PATH_PARTS for part in relative.parts):
            failures.append(f"generated path is tracked: {relative}")
            continue
        if relative.name in FORBIDDEN_NAMES or relative.suffix in FORBIDDEN_SUFFIXES:
            failures.append(f"generated file is tracked: {relative}")
            continue
        path = ROOT / relative
        if not path.is_file():
            continue
        size = path.stat().st_size
        if size > MAX_TRACKED_FILE_BYTES:
            failures.append(f"tracked file exceeds 5 MiB: {relative} ({size} bytes)")
        if size > TEXT_SCAN_LIMIT:
            continue
        data = path.read_bytes()
        for pattern in PRIVATE_PATHS:
            if pattern.search(data):
                failures.append(f"machine-local absolute path in: {relative}")
                break
        if any(marker in data for marker in SECRET_MARKERS):
            failures.append(f"secret-looking content in: {relative}")

    whitespace = subprocess.run(
        ["git", "diff", "--check", "HEAD"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if whitespace.returncode != 0:
        failures.append(whitespace.stdout.strip())

    if failures:
        print("Repository hygiene: FAIL")
        for failure in failures:
            print(f"- {failure}")
        return 1
    print("Repository hygiene: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

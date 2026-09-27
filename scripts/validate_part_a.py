#!/usr/bin/env python3
"""Run the repeatable local checks for every Part A prototype."""

from __future__ import annotations

import argparse
import json
import os
import platform
import subprocess
import sys
import time
from datetime import UTC, datetime
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / "target" / "part-a-validation"
PROTOTYPES = (
    "plotine-comparison",
    "layout-engine-spike",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=DEFAULT_OUTPUT,
        help="directory for summary.json and command logs",
    )
    parser.add_argument(
        "--prototype",
        action="append",
        choices=PROTOTYPES,
        help="validate only this prototype; repeat to select several",
    )
    return parser.parse_args()


def run(argv: list[str], cwd: Path) -> tuple[int, float, str]:
    started = time.monotonic()
    try:
        completed = subprocess.run(
            argv,
            cwd=cwd,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            errors="replace",
        )
        return completed.returncode, time.monotonic() - started, completed.stdout
    except OSError as error:
        return 127, time.monotonic() - started, f"{type(error).__name__}: {error}\n"


def probe(argv: list[str], cwd: Path = ROOT) -> str:
    exit_code, _, output = run(argv, cwd)
    return output.strip() if exit_code == 0 else f"unavailable (exit {exit_code}): {output.strip()}"


def write_summary(path: Path, summary: dict) -> None:
    path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    selected = tuple(args.prototype) if args.prototype else PROTOTYPES
    timestamp = datetime.now(UTC).isoformat()
    summary_path = output_dir / "summary.json"
    summary = {
        "schema_version": 1,
        "started_at_utc": timestamp,
        "repository": str(ROOT),
        "environment": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "python": platform.python_version(),
            "rust_toolchain": probe(["rustup", "show", "active-toolchain"]),
            "rustc": probe(["rustc", "--version"]),
            "cargo": probe(["cargo", "--version"]),
            "clippy": probe(["cargo", "clippy", "--version"]),
            "git_commit": probe(["git", "rev-parse", "HEAD"]),
            "git_status": probe(["git", "status", "--porcelain=v1"]),
        },
        "selected_prototypes": list(selected),
        "checks": [],
        "result": "running",
    }
    write_summary(summary_path, summary)

    checks: list[tuple[str, Path, list[str]]] = [
        ("a1-contract", ROOT, [sys.executable, "scripts/validate_a1.py"]),
    ]
    for prototype in selected:
        cwd = ROOT / "prototypes" / prototype
        feature_args: list[str] = []
        checks.extend(
            (
                (f"{prototype}:fmt", cwd, ["cargo", "fmt", "--check"]),
                (
                    f"{prototype}:test",
                    cwd,
                    ["cargo", "test", "--locked", *feature_args],
                ),
                (
                    f"{prototype}:clippy",
                    cwd,
                    [
                        "cargo",
                        "clippy",
                        "--locked",
                        "--all-targets",
                        *feature_args,
                        "--",
                        "-D",
                        "warnings",
                    ],
                ),
            )
        )
    checks.append(
        (
            "formal-workspace-tests",
            ROOT,
            ["cargo", "test", "--workspace", "--locked", "--all-targets"],
        )
    )

    for index, (name, cwd, argv) in enumerate(checks, start=1):
        relative_cwd = str(cwd.relative_to(ROOT)) if cwd != ROOT else "."
        print(f"[{index}/{len(checks)}] {name}", flush=True)
        exit_code, duration, output = run(argv, cwd)
        log_name = name.replace(":", "-") + ".log"
        log_path = output_dir / log_name
        log_path.write_text(output, encoding="utf-8")
        summary["checks"].append(
            {
                "name": name,
                "cwd": relative_cwd,
                "argv": argv,
                "exit_code": exit_code,
                "duration_seconds": round(duration, 3),
                "result": "pass" if exit_code == 0 else "fail",
                "log": os.path.relpath(log_path, ROOT),
            }
        )
        write_summary(summary_path, summary)
        print(f"  {'PASS' if exit_code == 0 else 'FAIL'} ({duration:.2f}s)", flush=True)

    failed = [check["name"] for check in summary["checks"] if check["exit_code"] != 0]
    summary["finished_at_utc"] = datetime.now(UTC).isoformat()
    summary["passed"] = len(summary["checks"]) - len(failed)
    summary["failed"] = len(failed)
    summary["failed_checks"] = failed
    summary["result"] = "pass" if not failed else "fail"
    write_summary(summary_path, summary)
    print(f"Part A local validation: {summary['result'].upper()}")
    print(f"summary={summary_path}")
    return 0 if not failed else 1


if __name__ == "__main__":
    raise SystemExit(main())

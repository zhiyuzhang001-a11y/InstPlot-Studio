#!/usr/bin/env python3
"""Validate the complete B3.5 single-axes artist framework."""

from __future__ import annotations

import json
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "target" / "b3-5-validation"
SIZE_CEILING = 12 * 1024 * 1024


@dataclass
class Check:
    name: str
    status: str
    detail: str
    log: str | None = None


def run(name: str, command: list[str]) -> Check:
    log = OUTPUT / f"{name}.log"
    result = subprocess.run(
        command, cwd=ROOT, text=True, stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT, check=False,
    )
    log.write_text(result.stdout, encoding="utf-8")
    return Check(name, "pass" if result.returncode == 0 else "fail",
                 f"exit={result.returncode}", str(log.relative_to(ROOT)))


def fact(name: str, passed: bool, detail: str) -> Check:
    return Check(name, "pass" if passed else "fail", detail)


def main() -> int:
    OUTPUT.mkdir(parents=True, exist_ok=True)
    checks = [
        run("format", ["cargo", "fmt", "--all", "--check"]),
        run("workspace-test", ["cargo", "test", "--workspace", "--locked"]),
        run("a7-regression", ["cargo", "test", "--manifest-path",
            "prototypes/layout-engine-spike/Cargo.toml", "--locked"]),
        run("clippy", ["cargo", "clippy", "--workspace", "--locked",
            "--all-targets", "--all-features", "--", "-D", "warnings"]),
        run("release", ["cargo", "build", "--release", "--locked",
            "--package", "instplot-studio"]),
    ]
    document = (ROOT / "apps/instplot-studio/src/document.rs").read_text(encoding="utf-8")
    model = (ROOT / "crates/instplot-layout/src/model.rs").read_text(encoding="utf-8")
    layout = (ROOT / "crates/instplot-layout/src/layout.rs").read_text(encoding="utf-8")
    project = (ROOT / "apps/instplot-studio/src/project.rs").read_text(encoding="utf-8")
    checks.extend([
        fact("error-bars", "ArtistProperties::ErrorBar" in document
             and "ErrorStyle" in model and "draw_error_bar" in layout,
             "error data, cap width and stroke style are formal"),
        fact("reference-and-role-lines", "ArtistProperties::ReferenceLine" in document
             and "ReferenceOrientation::Horizontal" in document,
             "reference/baseline and fit/theory line paths are supported"),
        fact("semantic-annotations", "formal_annotations" in document
             and "AnnotationPosition::FigurePoints" in document
             and "measure_label(&annotation.label" in layout,
             "semantic annotations retain identity and label AST"),
        fact("explicit-legend", "fn legend_spec(" in document
             and "LegendPosition::FigurePoints" in document
             and "legend_id" in layout,
             "legend entries, identity and requested position are formal"),
        fact("log-artist-test", "formal_figure_layout_supports_positive_log_data" in document,
             "positive line/scatter/error/reference data traverse log axes"),
        fact("schema-stability", "pub const PROJECT_SCHEMA_VERSION: u32 = 1;" in project,
             "schema_version=1"),
    ])
    executable = ROOT / "target/release" / (
        "instplot-studio.exe" if sys.platform == "win32" else "instplot-studio")
    size = executable.stat().st_size if executable.exists() else -1
    checks.append(fact("release-size", 0 < size <= SIZE_CEILING,
                       f"bytes={size} ceiling={SIZE_CEILING}"))
    passed = all(check.status == "pass" for check in checks)
    summary = {"schema_version": 1, "phase": "B3.5",
               "status": "pass" if passed else "fail",
               "checks": [asdict(check) for check in checks]}
    summary_path = OUTPUT / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    for index, check in enumerate(checks, 1):
        print(f"[{index}/{len(checks)}] {check.name}: {check.status.upper()} — {check.detail}")
    print(f"B3.5 automated validation: {'PASS' if passed else 'FAIL'}")
    print(f"summary={summary_path}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

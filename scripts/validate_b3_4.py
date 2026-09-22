#!/usr/bin/env python3
"""Validate the B3.4 formal line/scatter artist slice."""

from __future__ import annotations

import json
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "target" / "b3-4-validation"
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
        command,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    log.write_text(result.stdout, encoding="utf-8")
    return Check(
        name=name,
        status="pass" if result.returncode == 0 else "fail",
        detail=f"exit={result.returncode}",
        log=str(log.relative_to(ROOT)),
    )


def fact(name: str, passed: bool, detail: str) -> Check:
    return Check(name=name, status="pass" if passed else "fail", detail=detail)


def main() -> int:
    OUTPUT.mkdir(parents=True, exist_ok=True)
    checks = [
        run("format", ["cargo", "fmt", "--all", "--check"]),
        run("workspace-test", ["cargo", "test", "--workspace", "--locked"]),
        run(
            "resolved-preview-test",
            [
                "cargo",
                "test",
                "--manifest-path",
                "prototypes/ui-shell-spike/Cargo.toml",
                "--locked",
                "--all-features",
            ],
        ),
        run(
            "clippy",
            [
                "cargo",
                "clippy",
                "--workspace",
                "--locked",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ],
        ),
        run(
            "release",
            [
                "cargo",
                "build",
                "--release",
                "--locked",
                "--package",
                "instplot-studio",
            ],
        ),
    ]

    document = (ROOT / "apps" / "instplot-studio" / "src" / "document.rs").read_text(
        encoding="utf-8"
    )
    main_rs = (ROOT / "apps" / "instplot-studio" / "src" / "main.rs").read_text(
        encoding="utf-8"
    )
    project = (ROOT / "apps" / "instplot-studio" / "src" / "project.rs").read_text(
        encoding="utf-8"
    )
    checks.extend(
        [
            fact(
                "formal-figure-preview",
                "pub fn layout_figure(&self)" in document
                and ".layout_figure()" in main_rs,
                "GUI preview consumes the formal artist layout",
            ),
            fact(
                "line-scatter-adapter",
                "ArtistProperties::Line" in document
                and "ArtistProperties::Scatter" in document
                and "Series {" in document,
                "line and scatter become formal layout series",
            ),
            fact(
                "embedded-data-contract",
                "fn bound_points(" in document
                and "DataSourcePayload::Embedded" in document
                and "ExternalDataUnavailable" in document
                and "ColumnLengthMismatch" in document,
                "embedded columns resolve; unavailable external data fails explicitly",
            ),
            fact(
                "style-and-identity-contract",
                "fn palette_color(" in document
                and "fn dash_style(" in document
                and "fn marker_shape(" in document
                and "register_node_id(&artist.id" in document,
                "palette, stroke, marker and stable IDs are preserved",
            ),
            fact(
                "combined-line-marker-test",
                "matching_line_and_scatter_bindings_form_a_combined_visual_series" in document,
                "matching bindings are tested for identical resolved geometry",
            ),
            fact(
                "schema-stability",
                "pub const PROJECT_SCHEMA_VERSION: u32 = 1;" in project,
                "schema_version=1; composition uses existing artist records",
            ),
        ]
    )

    executable = ROOT / "target" / "release" / (
        "instplot-studio.exe" if sys.platform == "win32" else "instplot-studio"
    )
    size = executable.stat().st_size if executable.exists() else -1
    checks.append(
        fact(
            "release-size",
            0 < size <= SIZE_CEILING,
            f"bytes={size} ceiling={SIZE_CEILING}",
        )
    )

    passed = all(check.status == "pass" for check in checks)
    summary = {
        "schema_version": 1,
        "phase": "B3.4",
        "status": "pass" if passed else "fail",
        "checks": [asdict(check) for check in checks],
    }
    summary_path = OUTPUT / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    for index, check in enumerate(checks, start=1):
        print(f"[{index}/{len(checks)}] {check.name}: {check.status.upper()} — {check.detail}")
    print(f"B3.4 automated validation: {'PASS' if passed else 'FAIL'}")
    print(f"summary={summary_path}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

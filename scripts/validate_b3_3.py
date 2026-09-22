#!/usr/bin/env python3
"""Validate the B3.3 resolved-text preview path."""

from __future__ import annotations

import json
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "target" / "b3-3-validation"
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

    main_rs = (ROOT / "apps" / "instplot-studio" / "src" / "main.rs").read_text(
        encoding="utf-8"
    )
    preview = (ROOT / "apps" / "instplot-studio" / "src" / "preview.rs").read_text(
        encoding="utf-8"
    )
    shell = (ROOT / "prototypes" / "ui-shell-spike" / "src" / "lib.rs").read_text(
        encoding="utf-8"
    )
    resolver = (
        ROOT / "prototypes" / "export-backend-spike" / "src" / "resolve.rs"
    ).read_text(encoding="utf-8")
    checks.extend(
        [
            fact(
                "formal-layout-preview-source",
                "document.layout_axes()" in main_rs
                and "resolve(&layout.result.display_list)" in main_rs,
                "preview starts from the formal document layout",
            ),
            fact(
                "resolved-preview-boundary",
                "ResolvedDisplayList" in preview
                and "paint_resolved_display_list" in preview
                and "paint_display_list" not in preview,
                "preview consumes resolved items rather than flattened labels",
            ),
            fact(
                "bundled-face-registration",
                "install_publication_fonts" in shell
                and all(
                    face in resolver
                    for face in [
                        "TeXGyreHeros-Regular",
                        "TeXGyreHeros-Italic",
                        "TeXGyreHeros-Bold",
                        "TeXGyreHeros-BoldItalic",
                    ]
                ),
                "all four publication faces are registered for preview",
            ),
            fact(
                "run-positioning-and-rotation",
                "run.start_x" in shell
                and "run.baseline_shift" in shell
                and "rotation_degrees.to_radians()" in shell
                and "with_angle(" in shell,
                "resolved positions, baselines and rotation reach egui",
            ),
            fact(
                "actual-egui-shape-test",
                "resolved_preview_emits_a_rotated_egui_text_shape" in shell
                and "egui::Shape::Text" in shell,
                "test inspects the emitted -90 degree egui text shape",
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
        "phase": "B3.3",
        "status": "pass" if passed else "fail",
        "checks": [asdict(check) for check in checks],
    }
    summary_path = OUTPUT / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    for index, check in enumerate(checks, start=1):
        print(f"[{index}/{len(checks)}] {check.name}: {check.status.upper()} — {check.detail}")
    print(f"B3.3 automated validation: {'PASS' if passed else 'FAIL'}")
    print(f"summary={summary_path}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

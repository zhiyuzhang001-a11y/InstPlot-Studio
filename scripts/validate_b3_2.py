#!/usr/bin/env python3
"""Validate the B3.2 formal axes-decoration and clipping contract."""

from __future__ import annotations

import json
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "target" / "b3-2-validation"
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
            "a7-layout-regression",
            [
                "cargo",
                "test",
                "--manifest-path",
                "prototypes/layout-engine-spike/Cargo.toml",
                "--locked",
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
    layout_model = (ROOT / "crates" / "instplot-layout" / "src" / "model.rs").read_text(
        encoding="utf-8"
    )
    layout_draw = (ROOT / "crates" / "instplot-layout" / "src" / "layout.rs").read_text(
        encoding="utf-8"
    )
    project = (ROOT / "apps" / "instplot-studio" / "src" / "project.rs").read_text(
        encoding="utf-8"
    )
    checks.extend(
        [
            fact(
                "document-layout-adapter",
                "pub fn layout_axes(&self)" in document
                and "AxisScale::Log10 => Scale::Log10" in document
                and "LocatorSpec::Fixed" in document
                and "FormatterSpec::Scientific" in document,
                "project dimensions/scales/locators/formatters route to formal layout",
            ),
            fact(
                "semantic-label-adapter",
                "fn label_node(" in document
                and "LabelNode::GreekVariable" in document
                and "LabelNode::VariableSubscript" in document
                and "LabelNode::UnitSeparator" in document,
                "semantic label AST is retained",
            ),
            fact(
                "grid-and-clip-contract",
                "pub struct GridSpec" in layout_model
                and "draw_grid(chart" in layout_draw
                and "pub data_clip: Bounds" in document
                and "ClipPush" in layout_draw
                and "ClipPop" in layout_draw,
                "major/minor grid policy and axes clipping boundary are explicit",
            ),
            fact(
                "stable-id-reverse-map",
                "pub project_ids: BTreeMap<NodeId, String>" in document
                and "fn register_node_id(" in document
                and "DuplicateNodeId" in document,
                "layout identities remain reversible and collisions fail explicitly",
            ),
            fact(
                "schema-stability",
                "pub const PROJECT_SCHEMA_VERSION: u32 = 1;" in project,
                "schema_version=1 (no format churn for fixed grid policy)",
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
        "phase": "B3.2",
        "status": "pass" if passed else "fail",
        "checks": [asdict(check) for check in checks],
    }
    summary_path = OUTPUT / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    for index, check in enumerate(checks, start=1):
        print(f"[{index}/{len(checks)}] {check.name}: {check.status.upper()} — {check.detail}")
    print(f"B3.2 automated validation: {'PASS' if passed else 'FAIL'}")
    print(f"summary={summary_path}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

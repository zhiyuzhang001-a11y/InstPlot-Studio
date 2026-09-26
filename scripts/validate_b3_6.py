#!/usr/bin/env python3
"""Validate the B3.6 single-layout Preview/PDF/PNG pipeline."""

from __future__ import annotations

import json
import re
import struct
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "target" / "b3-6-validation"
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
        name,
        "pass" if result.returncode == 0 else "fail",
        f"exit={result.returncode}",
        str(log.relative_to(ROOT)),
    )


def fact(name: str, passed: bool, detail: str) -> Check:
    return Check(name, "pass" if passed else "fail", detail)


def main() -> int:
    OUTPUT.mkdir(parents=True, exist_ok=True)
    executable = ROOT / "target" / "release" / (
        "instplot-studio.exe" if sys.platform == "win32" else "instplot-studio"
    )
    pdf_path = OUTPUT / "formal.pdf"
    png_path = OUTPUT / "formal.png"

    checks = [
        run("format", ["cargo", "fmt", "--all", "--check"]),
        run("workspace-test", ["cargo", "test", "--workspace", "--locked"]),
        run(
            "ui-shell-test",
            [
                "cargo",
                "test",
                "--manifest-path",
                "crates/instplot-ui/Cargo.toml",
                "--locked",
                "--all-features",
            ],
        ),
        run(
            "a7-regression",
            [
                "cargo",
                "test",
                "--manifest-path",
                "prototypes/layout-engine-spike/Cargo.toml",
                "--locked",
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

    if executable.exists():
        checks.extend(
            [
                run(
                    "headless-pdf",
                    [str(executable), "--export-fixed-pdf", str(pdf_path)],
                ),
                run(
                    "headless-png",
                    [str(executable), "--export-fixed-png", str(png_path)],
                ),
            ]
        )
    else:
        checks.extend(
            [
                fact("headless-pdf", False, f"missing executable: {executable}"),
                fact("headless-png", False, f"missing executable: {executable}"),
            ]
        )

    render = (ROOT / "apps/instplot-studio/src/render.rs").read_text(encoding="utf-8")
    export = (ROOT / "apps/instplot-studio/src/export.rs").read_text(encoding="utf-8")
    main_rs = (ROOT / "apps/instplot-studio/src/main.rs").read_text(encoding="utf-8")
    workflow = (ROOT / ".github/workflows/instplot-studio.yml").read_text(
        encoding="utf-8"
    )

    checks.extend(
        [
            fact(
                "single-resolution-entry",
                "pub fn resolve_document(" in render
                and "document.layout_figure()?" in render
                and "resolve(&layout.result.display_list)" in render,
                "one function owns formal layout and Display List resolution",
            ),
            fact(
                "preview-consumer",
                "resolve_document(document)" in main_rs
                and ".compile()" not in main_rs,
                "GUI preview consumes the formal resolved figure",
            ),
            fact(
                "gui-current-document-export",
                "save_figure_pdf(&self.document" in main_rs
                and "save_figure_png(&self.document" in main_rs,
                "GUI PDF and PNG export the same current Figure Document as preview",
            ),
            fact(
                "pdf-png-consumers",
                export.count("resolve_document(document)") == 2
                and "to_pdf(&resolved.display)" in export
                and "rasterize_direct(" in export,
                "PDF and PNG consume the same resolved-figure entry point",
            ),
            fact(
                "cross-platform-workflow",
                workflow.count("--export-fixed-pdf") >= 2
                and workflow.count("--export-fixed-png") >= 2,
                "Unix and Windows CI exercise both headless backends",
            ),
        ]
    )

    pdf = pdf_path.read_bytes() if pdf_path.exists() else b""
    page_tree = re.search(rb"/Type\s*/Pages\s*/Count\s+(\d+)", pdf)
    page_count = int(page_tree.group(1)) if page_tree else 0
    embedded_fonts = sum(pdf.count(marker) for marker in (b"/FontFile ", b"/FontFile2 ", b"/FontFile3 "))
    checks.append(
        fact(
            "pdf-structure",
            pdf.startswith(b"%PDF-")
            and len(pdf) > 1_000
            and page_count == 1
            and embedded_fonts >= 2,
            f"bytes={len(pdf)} pages={page_count} embedded_font_streams={embedded_fonts}",
        )
    )

    png = png_path.read_bytes() if png_path.exists() else b""
    width = height = 0
    if len(png) >= 24 and png.startswith(b"\x89PNG\r\n\x1a\n"):
        width, height = struct.unpack(">II", png[16:24])
    checks.append(
        fact(
            "png-structure",
            len(png) > 1_000 and width >= 1_000 and height >= 700,
            f"bytes={len(png)} dimensions={width}x{height}",
        )
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
        "phase": "B3.6",
        "status": "pass" if passed else "fail",
        "checks": [asdict(check) for check in checks],
    }
    summary_path = OUTPUT / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    for index, check in enumerate(checks, 1):
        print(f"[{index}/{len(checks)}] {check.name}: {check.status.upper()} — {check.detail}")
    print(f"B3.6 automated validation: {'PASS' if passed else 'FAIL'}")
    print(f"summary={summary_path}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

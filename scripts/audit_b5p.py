#!/usr/bin/env python3
"""Run the complete, unattended B5P Studio product-polish audit."""

from __future__ import annotations

import hashlib
import json
import re
import shutil
import struct
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "target" / "b5p-audit"
P0_OUTPUT = ROOT / "target" / "b5p-p0-validation"
SIZE_CEILING = 12 * 1024 * 1024
EXPECTED_REPORTS = [
    f"reports/B5P_P{phase}_{name}.md"
    for phase, name in [
        (0, "WORKFLOW_BASELINE"),
        (1, "EDIT_TRANSACTIONS"),
        (2, "WORKSPACE_LIFECYCLE"),
        (3, "DATA_SERIES"),
        (4, "AXES_LABELS_SIZE"),
        (5, "ARTIST_EDITING"),
        (6, "CANVAS_INTERACTION"),
        (7, "PUBLICATION_EXPORT"),
    ]
]


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
    tail = result.stdout.strip().splitlines()[-1:] or ["no output"]
    return Check(
        name=name,
        status="pass" if result.returncode == 0 else "fail",
        detail=f"exit={result.returncode}; last={tail[0][:240]}",
        log=str(log.relative_to(ROOT)),
    )


def fact(name: str, passed: bool, detail: str) -> Check:
    return Check(name, "pass" if passed else "fail", detail)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def png_dimensions(path: Path) -> tuple[int, int]:
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR":
        raise ValueError(f"invalid PNG signature: {path}")
    return struct.unpack(">II", data[16:24])


def pdf_page_size_pt(data: bytes) -> tuple[float, float]:
    match = re.search(rb"/MediaBox\s*\[\s*0\s+0\s+([\d.]+)\s+([\d.]+)\s*\]", data)
    if match is None:
        raise ValueError("PDF page has no explicit MediaBox")
    return float(match[1]), float(match[2])


def pdf_fonts_are_approved(data: bytes) -> bool:
    approved = {
        b"TeXGyreHeros-Regular",
        b"TeXGyreHeros-Italic",
        b"STIXTwoMath-Regular",
    }
    fonts = {
        name.removesuffix(b"-Identity-H")
        for name in re.findall(rb"/BaseFont/(?:[A-Z]{6}\+)?([A-Za-z0-9-]+)", data)
    }
    return b"TeXGyreHeros-Regular" in fonts and fonts <= approved


def main() -> int:
    if OUTPUT.exists():
        shutil.rmtree(OUTPUT)
    OUTPUT.mkdir(parents=True)
    fixture_root = ROOT / "apps/instplot-studio/tests/fixtures"
    fixture_hashes = {
        path.name: sha256(path) for path in sorted(fixture_root.iterdir()) if path.is_file()
    }

    checks = [
        run("format", ["cargo", "fmt", "--all", "--", "--check"]),
        run(
            "workspace-all-targets",
            ["cargo", "test", "--workspace", "--locked", "--all-targets"],
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
            "release-build",
            [
                "cargo",
                "build",
                "--release",
                "--locked",
                "--package",
                "instplot-studio",
                "--bin",
                "instplot-studio",
            ],
        ),
        run("p0-workflows", [sys.executable, "scripts/validate_b5p_p0.py"]),
    ]

    # Formal InstPlot crates are workspace members. Only the remaining historical
    # comparison prototypes need separate test invocations.
    for prototype in (
        "layout-engine-spike",
        "plotine-comparison",
        "shared-core-consumer-spike",
    ):
        checks.append(
            run(
                f"prototype-{prototype}",
                [
                    "cargo",
                    "test",
                    "--manifest-path",
                    f"prototypes/{prototype}/Cargo.toml",
                    "--locked",
                    "--all-targets",
                    "--all-features",
                ],
            )
        )

    try:
        p0_summary = json.loads((P0_OUTPUT / "summary.json").read_text(encoding="utf-8"))
        failed_p0 = [
            check["name"]
            for check in p0_summary.get("checks", [])
            if check.get("status") != "pass"
        ]
        checks.append(
            fact(
                "p0-summary",
                p0_summary.get("status") == "pass" and not failed_p0,
                f"checks={len(p0_summary.get('checks', []))} failed={failed_p0}",
            )
        )
    except (OSError, json.JSONDecodeError, KeyError) as error:
        checks.append(fact("p0-summary", False, str(error)))

    artifact_root = P0_OUTPUT / "artifacts"
    try:
        projects = sorted(artifact_root.glob("*.instplot"))
        project_payloads = [json.loads(path.read_text(encoding="utf-8")) for path in projects]
        complete = all(
            payload.get("schema_version") == 6
            and payload.get("figure", {}).get("axes", [{}])[0]
            .get("x", {})
            .get("appearance", {})
            .get("tick_direction")
            == "in"
            and payload.get("export_preferences", {}).get("selected_raster_dpi") == 300
            and all(
                "visible" in entry
                for artist in payload.get("figure", {}).get("artists", [])
                if artist.get("properties", {}).get("kind") == "legend"
                for entry in artist.get("properties", {}).get("entries", [])
            )
            for payload in project_payloads
        )
        checks.append(
            fact(
                "document-contract",
                len(project_payloads) == 5 and complete,
                f"projects={len(project_payloads)} schema=6 axis/export/legend fields={complete}",
            )
        )
    except (OSError, json.JSONDecodeError, IndexError, TypeError) as error:
        checks.append(fact("document-contract", False, str(error)))

    try:
        pdfs = sorted(artifact_root.glob("*.pdf"))
        pngs = sorted(artifact_root.glob("*.png"))
        fonts_ok = all(pdf_fonts_are_approved(path.read_bytes()) for path in pdfs)
        dimensions = {path.stem: png_dimensions(path) for path in pngs}
        page_sizes = {path.stem: pdf_page_size_pt(path.read_bytes()) for path in pdfs}
        dimension_checks = {}
        for name, png_size in dimensions.items():
            project = json.loads((artifact_root / f"{name}.instplot").read_text(encoding="utf-8"))
            base = (
                float(project["figure"]["width_mm"]) * 72.0 / 25.4,
                float(project["figure"]["height_mm"]) * 72.0 / 25.4,
            )
            page = page_sizes[name]
            pixel_match = all(
                abs(actual - round(points * 300.0 / 72.0)) <= 1
                for actual, points in zip(png_size, page)
            )
            # A compact automatic legend may fit inside the fixed figure.  An
            # outside legend is allowed to extend the export canvas, but must
            # never crop or shrink the requested figure size.
            layout_match = all(actual + 0.02 >= expected for actual, expected in zip(page, base))
            dimension_checks[name] = pixel_match and layout_match
        checks.append(
            fact(
                "export-structure",
                len(pdfs) == 5
                and len(pngs) == 5
                and fonts_ok
                and len(dimension_checks) == 5
                and all(dimension_checks.values()),
                f"pdf={len(pdfs)} png={len(pngs)} fonts={fonts_ok} "
                f"dimensions={dimensions} page_sizes={page_sizes} checks={dimension_checks}",
            )
        )
    except (OSError, ValueError, KeyError, TypeError) as error:
        checks.append(fact("export-structure", False, str(error)))

    executable = ROOT / "target/release" / (
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

    report_state = {}
    for path in EXPECTED_REPORTS:
        report = ROOT / path
        content = report.read_text(encoding="utf-8") if report.exists() else ""
        report_state[path] = "DONE" in content or "Status: PASS" in content
    checks.append(
        fact(
            "phase-evidence",
            all(report_state.values()),
            f"reports={len(report_state)} incomplete={[p for p, ok in report_state.items() if not ok]}",
        )
    )

    unchanged = all((fixture_root / name).exists() and sha256(fixture_root / name) == digest
                    for name, digest in fixture_hashes.items())
    checks.append(
        fact("fixtures-unchanged", unchanged, f"fixtures={len(fixture_hashes)}")
    )

    tracked = subprocess.run(
        [
            "git",
            "grep",
            "-n",
            "-E",
            r"/Users/[A-Za-z0-9._-]+/|[A-Za-z]:\\Users\\[A-Za-z0-9._-]+\\",
        ],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    checks.append(
        fact(
            "no-machine-paths",
            tracked.returncode == 1,
            "no tracked machine-local paths" if tracked.returncode == 1 else tracked.stdout[:400],
        )
    )

    passed = all(check.status == "pass" for check in checks)
    summary = {
        "schema_version": 1,
        "phase": "B5P",
        "status": "pass" if passed else "fail",
        "checks": [asdict(check) for check in checks],
        "manual_checks": [
            "macOS GUI workflow and visual interaction",
            "Windows GUI minimum confirmation",
        ],
    }
    summary_path = OUTPUT / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    for index, check in enumerate(checks, 1):
        print(f"[{index}/{len(checks)}] {check.name}: {check.status.upper()} — {check.detail}")
        if check.status == "fail" and check.log:
            print(f"  log={check.log}")
    print(f"B5P audit: {'PASS' if passed else 'FAIL'}")
    print(f"summary={summary_path}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Run the complete, unattended B5P Studio product-polish audit."""

from __future__ import annotations

import hashlib
import json
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
            payload.get("schema_version") == 3
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
                len(project_payloads) == 4 and complete,
                f"projects={len(project_payloads)} schema=3 axis/export/legend fields={complete}",
            )
        )
    except (OSError, json.JSONDecodeError, IndexError, TypeError) as error:
        checks.append(fact("document-contract", False, str(error)))

    try:
        pdfs = sorted(artifact_root.glob("*.pdf"))
        pngs = sorted(artifact_root.glob("*.png"))
        fonts_ok = all(
            b"TeXGyreHeros-Regular" in path.read_bytes()
            and b"TeXGyreHeros-Italic" in path.read_bytes()
            for path in pdfs
        )
        dimensions = {path.name: png_dimensions(path) for path in pngs}
        checks.append(
            fact(
                "export-structure",
                len(pdfs) == 4
                and len(pngs) == 4
                and fonts_ok
                and all(size == (1004, 768) for size in dimensions.values()),
                f"pdf={len(pdfs)} png={len(pngs)} fonts={fonts_ok} dimensions={dimensions}",
            )
        )
    except (OSError, ValueError) as error:
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
        ["git", "grep", "-n", "/Users/" + "zhiyu"],
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

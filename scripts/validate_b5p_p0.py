#!/usr/bin/env python3
"""Validate the reproducible B5P P0 workflow baseline."""

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
OUTPUT = ROOT / "target" / "b5p-p0-validation"
FIXTURES = ROOT / "apps" / "instplot-studio" / "tests" / "fixtures"
EXPECTED_CASES = {
    "single-source": (1, 1, 2, 3, 3),
    "source-fit": (2, 2, 3, 6, 6),
    "multi-source": (2, 2, 3, 8, 8),
    "disabled-row": (1, 1, 2, 5, 4),
}
EXPECTED_PROJECT_SCHEMA = 3
FIXTURE_NAMES = [
    "smoke.csv",
    "lite-source-fit.txt",
    "p0-multi-source.txt",
    "p0-disabled-source.csv",
    "p0-missing-values.csv",
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
    return Check(
        name,
        "pass" if result.returncode == 0 else "fail",
        f"exit={result.returncode}",
        str(log.relative_to(ROOT)),
    )


def fact(name: str, passed: bool, detail: str) -> Check:
    return Check(name, "pass" if passed else "fail", detail)


def canonical_digest(payload: object) -> str:
    encoded = json.dumps(payload, separators=(",", ":"), ensure_ascii=False).encode()
    return hashlib.sha256(encoded).hexdigest()


def png_dimensions(path: Path) -> tuple[int, int]:
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR":
        raise ValueError(f"not a PNG: {path}")
    return struct.unpack(">II", data[16:24])


def main() -> int:
    if OUTPUT.exists():
        shutil.rmtree(OUTPUT)
    OUTPUT.mkdir(parents=True)
    artifacts = OUTPUT / "artifacts"
    fixture_hashes = {
        name: hashlib.sha256((FIXTURES / name).read_bytes()).hexdigest()
        for name in FIXTURE_NAMES
    }

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
            "build-binary",
            [
                "cargo",
                "build",
                "--locked",
                "--package",
                "instplot-studio",
                "--bin",
                "instplot-studio",
            ],
        ),
        run(
            "generate-baseline",
            [
                "cargo",
                "run",
                "--locked",
                "--package",
                "instplot-studio",
                "--example",
                "p0_baseline",
                "--",
                str(artifacts),
            ],
        ),
    ]

    try:
        manifest = json.loads((artifacts / "manifest.json").read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        manifest = {}
        checks.append(fact("manifest", False, str(error)))
    else:
        checks.append(
            fact(
                "manifest",
                manifest.get("schema_version") == 1
                and manifest.get("phase") == "B5P-P0",
                f"cases={len(manifest.get('valid_cases', []))}",
            )
        )

    cases = {
        case.get("id"): case
        for case in manifest.get("valid_cases", [])
        if isinstance(case, dict)
    }
    executable = ROOT / "target" / "debug" / (
        "instplot-studio.exe" if sys.platform == "win32" else "instplot-studio"
    )
    for case_id, expected in EXPECTED_CASES.items():
        case = cases.get(case_id, {})
        counts = (
            case.get("dataset_count"),
            case.get("data_source_count"),
            case.get("artist_count"),
            case.get("row_count"),
            case.get("alive_count"),
        )
        checks.append(fact(f"{case_id}-counts", counts == expected, f"counts={counts}"))
        project_path = artifacts / str(case.get("project", "missing"))
        handoff_path = artifacts / str(case.get("handoff", "missing"))
        pdf_path = artifacts / str(case.get("pdf", "missing"))
        png_path = artifacts / str(case.get("png", "missing"))
        try:
            project = json.loads(project_path.read_text(encoding="utf-8"))
            handoff = json.loads(handoff_path.read_text(encoding="utf-8"))
            pdf_bytes = pdf_path.read_bytes()
            dimensions = png_dimensions(png_path)
        except (OSError, ValueError, json.JSONDecodeError) as error:
            checks.append(fact(f"{case_id}-artifacts", False, str(error)))
            continue
        embedded = all(
            source.get("payload", {}).get("storage") == "embedded"
            for source in project.get("data_sources", [])
        )
        payload = handoff.get("payload", {})
        checksum_ok = handoff.get("payload_sha256") == canonical_digest(payload)
        fonts_ok = b"TeXGyreHeros-Regular" in pdf_bytes and b"TeXGyreHeros-Italic" in pdf_bytes
        checks.append(
            fact(
                f"{case_id}-artifacts",
                project.get("schema_version") == EXPECTED_PROJECT_SCHEMA
                and embedded
                and checksum_ok
                and pdf_bytes.startswith(b"%PDF-1.7")
                and fonts_ok
                and dimensions == (1004, 768),
                f"embedded={embedded} checksum={checksum_ok} png={dimensions} fonts={fonts_ok}",
            )
        )
        checks.append(
            run(
                f"{case_id}-check-project",
                [str(executable), "--check-project", str(project_path)],
            )
        )
        checks.append(
            run(
                f"{case_id}-publication-check",
                [str(executable), "--publication-check", str(project_path)],
            )
        )

    disabled = json.loads((artifacts / "disabled-row.instplot").read_text(encoding="utf-8"))
    disabled_payload = disabled["data_sources"][0]["payload"]
    disabled_axes = disabled["figure"]["axes"][0]
    checks.append(
        fact(
            "disabled-row-semantics",
            disabled_payload["alive"] == [True, True, False, True, True]
            and disabled_axes["y"]["maximum"] < 8.0,
            f"alive={disabled_payload['alive']} y_max={disabled_axes['y']['maximum']}",
        )
    )

    rejections = manifest.get("expected_rejections", [])
    rejection = rejections[0] if len(rejections) == 1 else {}
    checks.append(
        fact(
            "missing-value-is-explicitly-rejected",
            rejection.get("id") == "missing-value"
            and rejection.get("stage") == "figure_document_creation"
            and "inconsistent columns or alive state" in rejection.get("error", ""),
            rejection.get("error", f"rejection_count={len(rejections)}"),
        )
    )

    unchanged = all(
        hashlib.sha256((FIXTURES / name).read_bytes()).hexdigest() == digest
        for name, digest in fixture_hashes.items()
    )
    checks.append(fact("fixtures-unchanged", unchanged, f"fixtures={len(fixture_hashes)}"))

    passed = all(check.status == "pass" for check in checks)
    summary = {
        "schema_version": 1,
        "phase": "B5P-P0",
        "status": "pass" if passed else "fail",
        "checks": [asdict(check) for check in checks],
    }
    summary_path = OUTPUT / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    for index, check in enumerate(checks, 1):
        print(f"[{index}/{len(checks)}] {check.name}: {check.status.upper()} — {check.detail}")
    print(f"B5P P0 validation: {'PASS' if passed else 'FAIL'}")
    print(f"summary={summary_path}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

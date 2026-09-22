#!/usr/bin/env python3
"""Validate the Studio side of the B5 Lite handoff."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "target" / "b5-studio-validation"
FIXTURE = ROOT / "apps/instplot-studio/tests/fixtures/lite-source-fit.txt"
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
    package = OUTPUT / "lite-transfer.instplot-handoff"
    project = OUTPUT / "imported.instplot"
    fixture_before = hashlib.sha256(FIXTURE.read_bytes()).hexdigest()

    checks = [
        run("format", ["cargo", "fmt", "--all", "--check"]),
        run("workspace-test", ["cargo", "test", "--workspace", "--locked"]),
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
                    "create-handoff",
                    [str(executable), "--create-handoff", str(FIXTURE), str(package)],
                ),
                run(
                    "import-handoff",
                    [str(executable), "--import-handoff", str(package), str(project)],
                ),
                run("check-project", [str(executable), "--check-project", str(project)]),
                run(
                    "publication-check",
                    [str(executable), "--publication-check", str(project)],
                ),
            ]
        )
    else:
        checks.append(fact("release-entry-points", False, f"missing executable: {executable}"))

    try:
        envelope = json.loads(package.read_text(encoding="utf-8"))
        imported = json.loads(project.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        envelope = imported = {}
        parse_error = str(error)
    else:
        parse_error = "none"

    payload = envelope.get("payload", {})
    datasets = payload.get("datasets", [])
    dataset_by_id = {
        dataset.get("id"): dataset for dataset in datasets if isinstance(dataset, dict)
    }
    fit = dataset_by_id.get("fit-a", {})
    fit_link = fit.get("fit_link", {}) if isinstance(fit, dict) else {}
    source_dataset = dataset_by_id.get("source-a", {})
    source_columns = (
        source_dataset.get("columns", []) if isinstance(source_dataset, dict) else []
    )
    source_values = source_columns[1].get("values") if len(source_columns) > 1 else None
    data_sources = {
        source.get("id"): source
        for source in imported.get("data_sources", [])
        if isinstance(source, dict)
    }
    imported_fit = data_sources.get("fit-a", {}).get("fit", {})
    source_payload = data_sources.get("source-a", {}).get("payload", {})
    checks.extend(
        [
            fact(
                "handoff-envelope",
                envelope.get("schema_version") == 1
                and len(envelope.get("payload_sha256", "")) == 64
                and payload.get("producer_name") == "InstPlot Lite compatible producer",
                f"datasets={len(datasets)} parse_error={parse_error}",
            ),
            fact(
                "lossless-fit-identity",
                fit_link.get("parent_id") == "source-a"
                and fit_link.get("source_x") == "field"
                and fit_link.get("source_y") == "response"
                and fit_link.get("equation") == "y=1+x"
                and fit_link.get("display_equation") == "y = 1 + x",
                "Parent-ID, Source-X/Y and both equations preserved",
            ),
            fact(
                "full-data-and-alive",
                source_dataset.get("alive") == [True, True, True]
                and source_values == [1.0, 2.0, 3.0],
                "ordered columns, f64 values and alive bitmap preserved",
            ),
            fact(
                "embedded-independent-project",
                source_payload.get("storage") == "embedded"
                and imported_fit.get("parent_data_source_id") == "source-a"
                and imported_fit.get("display_equation") == "y = 1 + x",
                "formal project embeds data and fit identity",
            ),
            fact(
                "no-source-writeback",
                hashlib.sha256(FIXTURE.read_bytes()).hexdigest() == fixture_before,
                f"fixture_sha256={fixture_before}",
            ),
        ]
    )

    handoff_source = (ROOT / "apps/instplot-studio/src/handoff.rs").read_text(encoding="utf-8")
    main_source = (ROOT / "apps/instplot-studio/src/main.rs").read_text(encoding="utf-8")
    checks.extend(
        [
            fact(
                "safe-cleanup",
                "symlink_metadata" in handoff_source
                and "DeleteAfterImport" in handoff_source
                and "fs::remove_file(path)?" in handoff_source
                and "document.layout_figure()" in handoff_source,
                "only the validated regular package is removed after layout",
            ),
            fact(
                "studio-launch-contract",
                "--open-handoff" in main_source and "Open from Lite…" in main_source,
                "CLI launch argument and UI consumer are implemented",
            ),
            fact(
                "compatibility-fixtures",
                "lite_xlsx_round_trip_preserves_the_same_fit_link"
                in (ROOT / "apps/instplot-studio/src/session.rs").read_text(encoding="utf-8")
                and "checksum_and_explicit_parent_identity_are_enforced" in handoff_source,
                "TXT, XLSX, checksum and explicit-link cases are covered",
            ),
        ]
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
        "phase": "B5-studio",
        "status": "pass" if passed else "fail",
        "checks": [asdict(check) for check in checks],
    }
    summary_path = OUTPUT / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    for index, check in enumerate(checks, 1):
        print(f"[{index}/{len(checks)}] {check.name}: {check.status.upper()} — {check.detail}")
    print(f"B5 Studio-side validation: {'PASS' if passed else 'FAIL'}")
    print(f"summary={summary_path}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

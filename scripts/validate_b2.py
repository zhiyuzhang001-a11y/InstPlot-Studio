#!/usr/bin/env python3
"""Run the repeatable automated acceptance checks for Part B phase B2."""

from __future__ import annotations

import json
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "target" / "b2-validation"
PROJECT = OUTPUT / "roundtrip.instplot"
BACKUP = OUTPUT / "roundtrip.instplot.bak"
LEGACY = ROOT / "apps" / "instplot-studio" / "tests" / "fixtures" / "project-v0.instplot"
SIZE_CEILING = 12 * 1024 * 1024
REQUIRED_ROOT_FIELDS = {
    "schema_version",
    "producer_version",
    "figure",
    "data_sources",
    "semantic_registry",
    "palette",
    "typography",
    "overrides",
    "export_preferences",
    "provenance",
}


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


def invoke(binary: Path, *arguments: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(binary), *arguments],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )


def main() -> int:
    OUTPUT.mkdir(parents=True, exist_ok=True)
    for generated in (PROJECT, BACKUP):
        generated.unlink(missing_ok=True)

    checks = [
        run("format", ["cargo", "fmt", "--all", "--check"]),
        run("test", ["cargo", "test", "--workspace", "--locked"]),
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

    executable = ROOT / "target" / "release" / (
        "instplot-studio.exe" if sys.platform == "win32" else "instplot-studio"
    )
    if executable.exists():
        created = invoke(executable, "--create-project", str(PROJECT))
        checks.append(
            fact(
                "headless-create-project",
                created.returncode == 0 and PROJECT.exists(),
                created.stdout.strip() or created.stderr.strip(),
            )
        )

        try:
            payload = json.loads(PROJECT.read_text(encoding="utf-8"))
            schema_ok = (
                set(payload) == REQUIRED_ROOT_FIELDS
                and payload["schema_version"] == 1
                and payload["typography"]["family"] == "TeX Gyre Heros"
                and len(payload["typography"]["faces"]) == 4
                and payload["palette"]["id"] == "publication-default-v1"
                and payload["export_preferences"]["vector_format"] == "pdf"
            )
            schema_detail = (
                f"schema={payload.get('schema_version')} fields={sorted(payload)}"
            )
        except (OSError, KeyError, TypeError, json.JSONDecodeError) as error:
            schema_ok = False
            schema_detail = str(error)
        checks.append(fact("schema-contract", schema_ok, schema_detail))

        opened = invoke(executable, "--check-project", str(PROJECT))
        checks.append(
            fact(
                "headless-open-project",
                opened.returncode == 0
                and "schema=1" in opened.stdout
                and "source=Primary" in opened.stdout
                and "warnings=0" in opened.stdout,
                opened.stdout.strip() or opened.stderr.strip(),
            )
        )

        second_save = invoke(executable, "--create-project", str(PROJECT))
        backup_ok = (
            second_save.returncode == 0
            and BACKUP.exists()
            and json.loads(BACKUP.read_text(encoding="utf-8"))["schema_version"] == 1
        )
        checks.append(fact("atomic-backup", backup_ok, f"backup={BACKUP.exists()}"))

        PROJECT.write_text("corrupt primary\n", encoding="utf-8")
        recovered = invoke(executable, "--check-project", str(PROJECT))
        checks.append(
            fact(
                "backup-recovery",
                recovered.returncode == 0
                and "source=Backup" in recovered.stdout
                and "warnings=" in recovered.stdout,
                recovered.stdout.strip() or recovered.stderr.strip(),
            )
        )

        migrated = invoke(executable, "--check-project", str(LEGACY))
        checks.append(
            fact(
                "legacy-migration",
                migrated.returncode == 0 and "schema=1" in migrated.stdout,
                migrated.stdout.strip() or migrated.stderr.strip(),
            )
        )

        size = executable.stat().st_size
        checks.append(
            fact(
                "release-size",
                size <= SIZE_CEILING,
                f"bytes={size} ceiling={SIZE_CEILING}",
            )
        )
    else:
        for name in (
            "headless-create-project",
            "schema-contract",
            "headless-open-project",
            "atomic-backup",
            "backup-recovery",
            "legacy-migration",
            "release-size",
        ):
            checks.append(fact(name, False, "release executable missing"))

    metadata = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if metadata.returncode == 0:
        packages = json.loads(metadata.stdout)["packages"]
        missing = sorted(
            f"{package['name']} {package['version']}"
            for package in packages
            if package.get("license") is None
        )
        checks.append(
            fact(
                "dependency-license-metadata",
                not missing,
                "complete" if not missing else repr(missing),
            )
        )
    else:
        checks.append(fact("dependency-license-metadata", False, metadata.stderr.strip()))

    checks.append(
        fact(
            "adr-and-migration-fixture",
            (ROOT / "adr" / "022-versioned-project-container.md").exists()
            and LEGACY.exists(),
            "ADR-022 and schema-0 fixture present",
        )
    )

    passed = all(check.status == "pass" for check in checks)
    summary = {
        "schema_version": 1,
        "phase": "B2",
        "status": "pass" if passed else "fail",
        "checks": [asdict(check) for check in checks],
    }
    summary_path = OUTPUT / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    for index, check in enumerate(checks, start=1):
        print(f"[{index}/{len(checks)}] {check.name}: {check.status.upper()} — {check.detail}")
    print(f"B2 automated validation: {'PASS' if passed else 'FAIL'}")
    print(f"summary={summary_path}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

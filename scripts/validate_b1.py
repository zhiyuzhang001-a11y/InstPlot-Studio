#!/usr/bin/env python3
"""Run the repeatable automated acceptance checks for Part B phase B1."""

from __future__ import annotations

import json
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "target" / "b1-validation"
PDF = OUTPUT / "fixed-figure.pdf"
EXPECTED_REVISION = "80ad374044d91dd3c306a384fc7f07ba82cac429"
SIZE_CEILING = 12 * 1024 * 1024


def product_version() -> str:
    result = subprocess.run(
        [sys.executable, str(ROOT / "scripts" / "read_version.py")],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "unable to read product version")
    return result.stdout.strip()


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
    expected_product = f"InstPlot Studio\tinstplot-studio\t{product_version()}"
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
        identity = subprocess.run(
            [str(executable), "--product-info"],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        checks.append(
            fact(
                "product-identity",
                identity.returncode == 0 and identity.stdout.strip() == expected_product,
                identity.stdout.strip() or identity.stderr.strip(),
            )
        )
        export = subprocess.run(
            [str(executable), "--export-fixed-pdf", str(PDF)],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        checks.append(
            fact(
                "headless-pdf-export",
                export.returncode == 0
                and PDF.exists()
                and PDF.stat().st_size > 1_000
                and PDF.read_bytes().startswith(b"%PDF-"),
                export.stdout.strip() or export.stderr.strip(),
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
        checks.extend(
            [
                fact("product-identity", False, "release executable missing"),
                fact("headless-pdf-export", False, "release executable missing"),
                fact("release-size", False, "release executable missing"),
            ]
        )

    metadata = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--no-deps"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if metadata.returncode == 0:
        payload = json.loads(metadata.stdout)
        packages = {package["id"]: package for package in payload["packages"]}
        members = [packages[member]["name"] for member in payload["workspace_members"]]
        app = next(package for package in payload["packages"] if package["name"] == "instplot-studio")
        git_dependencies = {
            dependency["name"]: dependency.get("source", "") for dependency in app["dependencies"]
        }
        shared_are_pinned = all(
            f"rev={EXPECTED_REVISION}" in git_dependencies.get(name, "")
            for name in ("instplot-core", "instplot-io")
        )
        checks.append(
            fact(
                "workspace-members",
                members == ["instplot-studio", "instplot-layout"],
                repr(members),
            )
        )
        checks.append(
            fact(
                "shared-revision",
                shared_are_pinned,
                f"core={git_dependencies.get('instplot-core')} io={git_dependencies.get('instplot-io')}",
            )
        )
    else:
        checks.extend(
            [
                fact("workspace-members", False, metadata.stderr.strip()),
                fact("shared-revision", False, metadata.stderr.strip()),
            ]
        )

    full_metadata = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if full_metadata.returncode == 0:
        payload = json.loads(full_metadata.stdout)
        missing_licenses = sorted(
            f"{package['name']} {package['version']}"
            for package in payload["packages"]
            if package.get("license") is None
        )
        checks.append(
            fact(
                "dependency-license-metadata",
                not missing_licenses,
                "complete" if not missing_licenses else repr(missing_licenses),
            )
        )
    else:
        checks.append(
            fact("dependency-license-metadata", False, full_metadata.stderr.strip())
        )

    rustc = subprocess.run(
        ["rustc", "--version"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    checks.append(
        fact(
            "pinned-rust",
            rustc.returncode == 0 and rustc.stdout.startswith("rustc 1.98.0 "),
            rustc.stdout.strip() or rustc.stderr.strip(),
        )
    )

    passed = all(check.status == "pass" for check in checks)
    summary = {
        "schema_version": 1,
        "phase": "B1",
        "status": "pass" if passed else "fail",
        "checks": [asdict(check) for check in checks],
    }
    summary_path = OUTPUT / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    for index, check in enumerate(checks, start=1):
        print(f"[{index}/{len(checks)}] {check.name}: {check.status.upper()} — {check.detail}")
    print(f"B1 automated validation: {'PASS' if passed else 'FAIL'}")
    print(f"summary={summary_path}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

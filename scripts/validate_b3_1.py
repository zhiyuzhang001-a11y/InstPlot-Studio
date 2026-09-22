#!/usr/bin/env python3
"""Validate the B3.1 promotion of A7 into the formal layout crate."""

from __future__ import annotations

import json
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "target" / "b3-1-validation"
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
            "a7-compatibility",
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

    metadata = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if metadata.returncode == 0:
        payload = json.loads(metadata.stdout)
        packages = {package["id"]: package for package in payload["packages"]}
        members = [packages[item]["name"] for item in payload["workspace_members"]]
        app = next(package for package in payload["packages"] if package["name"] == "instplot-studio")
        app_dependencies = {dependency["name"] for dependency in app["dependencies"]}
        checks.append(
            fact(
                "workspace-topology",
                members == ["instplot-studio", "instplot-layout"]
                and "instplot-layout" in app_dependencies,
                f"members={members!r} app_has_layout={'instplot-layout' in app_dependencies}",
            )
        )
        missing = sorted(
            f"{package['name']} {package['version']}"
            for package in payload["packages"]
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
        checks.extend(
            [
                fact("workspace-topology", False, metadata.stderr.strip()),
                fact("dependency-license-metadata", False, metadata.stderr.strip()),
            ]
        )

    production_sources = sorted((ROOT / "crates" / "instplot-layout" / "src").glob("*.rs"))
    prototype_sources = sorted((ROOT / "prototypes" / "layout-engine-spike" / "src").glob("*.rs"))
    facade_text = prototype_sources[0].read_text(encoding="utf-8") if len(prototype_sources) == 1 else ""
    checks.append(
        fact(
            "single-layout-implementation",
            len(production_sources) == 6
            and [path.name for path in prototype_sources] == ["lib.rs"]
            and "pub use instplot_layout::*;" in facade_text,
            f"production={len(production_sources)} prototype={[path.name for path in prototype_sources]!r}",
        )
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
        "phase": "B3.1",
        "status": "pass" if passed else "fail",
        "checks": [asdict(check) for check in checks],
    }
    summary_path = OUTPUT / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    for index, check in enumerate(checks, start=1):
        print(f"[{index}/{len(checks)}] {check.name}: {check.status.upper()} — {check.detail}")
    print(f"B3.1 automated validation: {'PASS' if passed else 'FAIL'}")
    print(f"summary={summary_path}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

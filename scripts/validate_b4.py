#!/usr/bin/env python3
"""Validate B4 palette/semantic registries and Publication Check."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "target" / "b4-validation"
SIZE_CEILING = 12 * 1024 * 1024
EXPECTED_RULES = {
    "physical_size",
    "font_size",
    "stroke_width",
    "font_embedding",
    "clipping",
    "legend_overlap",
    "color_only_encoding",
    "palette_data_relationship",
    "grayscale_distinguishability",
    "cvd_risk",
    "raster_dpi_pixels",
    "transparency",
    "provenance_completeness",
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
    project = OUTPUT / "publication.instplot"
    report_path = OUTPUT / "publication-report.json"

    checks = [
        run("format", ["cargo", "fmt", "--all", "--check"]),
        run("workspace-test", ["cargo", "test", "--workspace", "--locked"]),
        run(
            "ui-shell-test",
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
        checks.append(run("create-project", [str(executable), "--create-project", str(project)]))
        publication = run(
            "publication-check",
            [str(executable), "--publication-check", str(project)],
        )
        checks.append(publication)
        log = OUTPUT / "publication-check.log"
        if log.exists():
            report_path.write_text(log.read_text(encoding="utf-8"), encoding="utf-8")
    else:
        checks.extend(
            [
                fact("create-project", False, f"missing executable: {executable}"),
                fact("publication-check", False, f"missing executable: {executable}"),
            ]
        )

    try:
        report = json.loads(report_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        report = {}
        report_error = str(error)
    else:
        report_error = "none"
    findings = report.get("findings", []) if isinstance(report, dict) else []
    rule_ids = {item.get("rule_id") for item in findings if isinstance(item, dict)}
    errors = [item for item in findings if item.get("severity") == "error"]
    unlocated_warnings = [
        item
        for item in findings
        if item.get("severity") == "warning" and not item.get("node_id")
    ]
    checks.extend(
        [
            fact(
                "rules-version",
                report.get("rules_version") == "instplot-publication-rules-v1",
                f"rules_version={report.get('rules_version')!r} parse_error={report_error}",
            ),
            fact(
                "complete-rule-set",
                rule_ids == EXPECTED_RULES,
                f"rules={len(rule_ids)} expected={len(EXPECTED_RULES)}",
            ),
            fact(
                "baseline-has-no-errors",
                not errors,
                f"errors={len(errors)}",
            ),
            fact(
                "warnings-have-node",
                not unlocated_warnings,
                f"unlocated_warnings={len(unlocated_warnings)}",
            ),
        ]
    )

    palette_fixture = ROOT / "fixtures/publication-v1/palettes.toml"
    digest = hashlib.sha256(palette_fixture.read_bytes()).hexdigest()
    palette_source = (ROOT / "apps/instplot-studio/src/palette.rs").read_text(encoding="utf-8")
    publication_source = (ROOT / "apps/instplot-studio/src/publication.rs").read_text(
        encoding="utf-8"
    )
    semantic_source = (ROOT / "apps/instplot-studio/src/semantic.rs").read_text(
        encoding="utf-8"
    )
    main_source = (ROOT / "apps/instplot-studio/src/main.rs").read_text(encoding="utf-8")
    checks.extend(
        [
            fact(
                "palette-provenance",
                digest in palette_source
                and "builtin_palettes" in palette_source
                and "PaletteOrdering::Diverging { center_index: 4 }" in palette_source,
                f"fixture_sha256={digest}",
            ),
            fact(
                "semantic-registry",
                "SEMANTIC_REGISTRY_VERSION" in semantic_source
                and "ColorPolicy::ObjectIdentity" in semantic_source
                and "RequiredNonColorChannel::Marker" in semantic_source
                and "RequiredNonColorChannel::DashedLine" in semantic_source,
                "versioned role-to-colour and redundant-style policy is present",
            ),
            fact(
                "versioned-cvd-model",
                "machado-2009-deuteranopia-severity-1-linear-srgb-v1"
                in publication_source
                and "0.367_322" in publication_source,
                "CVD simulation identity and severity-1 linear-sRGB matrix are fixed",
            ),
            fact(
                "rule-fixtures",
                "structural_rule_fixtures_detect_font_stroke_and_clip_failures"
                in publication_source
                and "semantic_and_palette_rule_fixtures_detect_risks" in publication_source
                and "export_parameters_and_document_changes_update_findings"
                in publication_source,
                "structural, semantic, document and export-parameter failures are exercised",
            ),
            fact(
                "ui-and-headless-entry",
                "Publication Check" in main_source
                and "--publication-check" in main_source
                and "check_publication(&self.document" in main_source,
                "live inspector and structured headless report use the same rule engine",
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
        "phase": "B4",
        "status": "pass" if passed else "fail",
        "checks": [asdict(check) for check in checks],
    }
    summary_path = OUTPUT / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    for index, check in enumerate(checks, 1):
        print(f"[{index}/{len(checks)}] {check.name}: {check.status.upper()} — {check.detail}")
    print(f"B4 automated validation: {'PASS' if passed else 'FAIL'}")
    print(f"summary={summary_path}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

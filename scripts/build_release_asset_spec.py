#!/usr/bin/env python3
"""Create the strict multi-platform update asset specification from Release files."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path


def build_spec(asset_dir: Path, version: str, windows_contract_path: Path | None = None) -> dict[str, object]:
    definitions = {
        "linux-x86_64": (
            "deb",
            [
                ("deb", "deb", f"InstPlot-Studio-{version}-linux-x86_64.deb", "Ubuntu 22.04 or compatible"),
                ("portable", "tar-gz", f"InstPlot-Studio-{version}-linux-x86_64.tar.gz", "glibc 2.35 or newer"),
            ],
        ),
        "macos-aarch64": (
            "dmg",
            [("dmg", "dmg", f"InstPlot-Studio-{version}-macos-aarch64.dmg", "macOS 12 arm64 or newer")],
        ),
        "windows-x86_64": (
            "inno-setup",
            [
                (
                    "inno-setup",
                    "exe-installer",
                    f"InstPlot-Studio-{version}-windows-x86_64-setup.exe",
                    "Windows 10 x64 or newer",
                )
            ],
        ),
    }
    expected = {filename for _, packages in definitions.values() for _, _, filename, _ in packages}
    # The explicit evidence input is a checksummed Release sidecar, not a
    # platform download. Keep the package inventory strict in both modes.
    if windows_contract_path is not None:
        if windows_contract_path.resolve() == (asset_dir / "windows-in-place.json").resolve():
            expected.add("windows-in-place.json")
    actual = {
        path.name
        for path in asset_dir.iterdir()
        if path.is_file() and path.name != "SHA256SUMS.txt"
    }
    if actual != expected:
        missing = sorted(expected - actual)
        unexpected = sorted(actual - expected)
        raise ValueError(f"release asset mismatch; missing={missing}, unexpected={unexpected}")
    platforms: dict[str, object] = {}
    for platform, (preferred, packages) in definitions.items():
        platforms[platform] = {
            "preferred": preferred,
            "packages": [
                {
                    "id": package_id,
                    "minimum_system": minimum_system,
                    "package_type": package_type,
                    "path": str((asset_dir / filename).resolve()),
                }
                for package_id, package_type, filename, minimum_system in packages
            ],
        }
    if windows_contract_path is not None:
        if windows_contract_path.is_symlink():
            raise ValueError("Windows contract evidence must not be a symlink")
        evidence = json.loads(windows_contract_path.read_text(encoding="utf-8"))
        if not isinstance(evidence, dict) or set(evidence) != {"scope", "version", "installer_sha256", "contract"}:
            raise ValueError("invalid Windows contract evidence")
        if evidence["scope"] != "preview-components-not-accepted-updater" or evidence["version"] != version:
            raise ValueError("Windows contract evidence scope/version mismatch")
        installer = asset_dir / f"InstPlot-Studio-{version}-windows-x86_64-setup.exe"
        if installer.is_symlink():
            raise ValueError("Windows installer must not be a symlink")
        digest_state = hashlib.sha256()
        with installer.open("rb") as source:
            for block in iter(lambda: source.read(1024 * 1024), b""):
                digest_state.update(block)
        digest = digest_state.hexdigest()
        if evidence["installer_sha256"] != digest:
            raise ValueError("Windows contract evidence installer hash mismatch")
        # Reuse the publisher's exact schema instead of maintaining a looser
        # second contract definition in the build tooling.
        module_spec = importlib.util.spec_from_file_location(
            "windows_contract_publisher", Path(__file__).resolve().parents[1] / "packaging/update/prepare_oss_release.py"
        )
        assert module_spec is not None and module_spec.loader is not None
        publisher = importlib.util.module_from_spec(module_spec)
        module_spec.loader.exec_module(publisher)
        platforms["windows-x86_64"]["windows_in_place"] = publisher.validate_windows_contract(evidence["contract"])
    return {"platforms": platforms}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--asset-dir", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--windows-contract-evidence", type=Path)
    args = parser.parse_args()
    try:
        payload = build_spec(args.asset_dir.resolve(), args.version, args.windows_contract_evidence)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    args.output.write_text(
        json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

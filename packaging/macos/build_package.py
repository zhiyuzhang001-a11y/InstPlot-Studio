#!/usr/bin/env python3
"""Build a versioned InstPlot Studio app bundle and DMG on macOS."""

from __future__ import annotations

import os
import platform
import plistlib
import shutil
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from install_studio_macos import (  # noqa: E402
    BUNDLE_ID,
    BUNDLE_NAME,
    EXECUTABLE,
    ICON_NAME,
    build_icon,
    package_version,
)


def run(*command: str, env: dict[str, str] | None = None) -> None:
    subprocess.run(command, cwd=ROOT, check=True, env=env)


def main() -> int:
    if sys.platform != "darwin":
        raise SystemExit("macOS packages must be built on macOS")
    machine = platform.machine().lower()
    if machine != "arm64":
        raise SystemExit(f"first-release macOS package requires arm64, got {machine}")
    version = package_version()
    output = ROOT / (sys.argv[1] if len(sys.argv) > 1 else "target/packages/macos")
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)

    build_id = os.environ.get(
        "INSTPLOT_BUILD_ID",
        datetime.now(timezone.utc).strftime("%Y%m%d%H%M"),
    )
    build_env = os.environ.copy()
    build_env["INSTPLOT_BUILD_ID"] = build_id
    run(
        "cargo",
        "build",
        "--release",
        "--locked",
        "--package",
        EXECUTABLE,
        "--bin",
        EXECUTABLE,
        env=build_env,
    )

    with tempfile.TemporaryDirectory(prefix="instplot-dmg-") as temporary:
        root = Path(temporary)
        bundle = root / BUNDLE_NAME
        macos = bundle / "Contents/MacOS"
        resources = bundle / "Contents/Resources"
        macos.mkdir(parents=True)
        resources.mkdir()
        installed_binary = macos / EXECUTABLE
        shutil.copy2(ROOT / "target/release" / EXECUTABLE, installed_binary)
        installed_binary.chmod(installed_binary.stat().st_mode | 0o111)
        shutil.copy2(ROOT / "LICENSE", resources / "LICENSE")
        build_icon(root, resources)
        info = {
            "CFBundleDevelopmentRegion": "en",
            "CFBundleDisplayName": "InstPlot Studio",
            "CFBundleExecutable": EXECUTABLE,
            "CFBundleIconFile": ICON_NAME,
            "CFBundleIdentifier": BUNDLE_ID,
            "CFBundleName": "InstPlot Studio",
            "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": version,
            "CFBundleVersion": build_id,
            "CFBundleDocumentTypes": [
                {
                    "CFBundleTypeExtensions": ["instplot"],
                    "CFBundleTypeName": "InstPlot Studio Project",
                    "CFBundleTypeRole": "Editor",
                    "LSHandlerRank": "Owner",
                    "LSItemContentTypes": ["com.instplot.studio.project"],
                }
            ],
            "LSMinimumSystemVersion": "12.0",
            "NSHighResolutionCapable": True,
            "UTExportedTypeDeclarations": [
                {
                    "UTTypeConformsTo": ["public.json", "public.data"],
                    "UTTypeDescription": "InstPlot Studio Project",
                    "UTTypeIdentifier": "com.instplot.studio.project",
                    "UTTypeTagSpecification": {
                        "public.filename-extension": ["instplot"],
                        "public.mime-type": "application/vnd.instplot.project+json",
                    },
                }
            ],
        }
        with (bundle / "Contents/Info.plist").open("wb") as stream:
            plistlib.dump(info, stream)

        identity = os.environ.get("MACOS_SIGNING_IDENTITY", "-")
        run("codesign", "--force", "--deep", "--options", "runtime", "--sign", identity, str(bundle))
        run("codesign", "--verify", "--deep", "--strict", str(bundle))
        actual = subprocess.check_output(
            [str(installed_binary), "--product-info"], cwd=ROOT, text=True
        ).strip()
        expected = f"InstPlot Studio\tinstplot-studio\t{version}"
        if actual != expected:
            raise SystemExit(f"unexpected packaged product identity: {actual}")

        dmg_root = root / "dmg-root"
        dmg_root.mkdir()
        shutil.copytree(bundle, dmg_root / BUNDLE_NAME)
        applications_link = dmg_root / "Applications"
        applications_link.symlink_to("/Applications")
        dmg = output / f"InstPlot-Studio-{version}-macos-aarch64.dmg"
        run(
            "hdiutil",
            "create",
            "-fs",
            "HFS+",
            "-volname",
            "InstPlot Studio",
            "-srcfolder",
            str(dmg_root),
            str(dmg),
        )
        run("hdiutil", "verify", str(dmg))
    if not dmg.is_file() or dmg.stat().st_size == 0:
        raise SystemExit("DMG was not created")
    print(dmg)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

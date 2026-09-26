#!/usr/bin/env python3
"""Build and replace the single local macOS InstPlot Studio app bundle."""

from __future__ import annotations

import json
import os
import plistlib
import shutil
import subprocess
import sys
import tempfile
from datetime import datetime
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BUNDLE_NAME = "InstPlot Studio.app"
BUNDLE_ID = "com.instplot.studio"
EXECUTABLE = "instplot-studio"
ICON_NAME = "InstPlotStudio"
ICON_SOURCE = ROOT / "apps/instplot-studio/assets/InstPlotStudio.png"
LSREGISTER = Path(
    "/System/Library/Frameworks/CoreServices.framework/Frameworks/"
    "LaunchServices.framework/Support/lsregister"
)


def run(*command: str, env: dict[str, str] | None = None) -> None:
    subprocess.run(command, cwd=ROOT, check=True, env=env)


def package_version() -> str:
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"],
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=True,
    )
    packages = json.loads(result.stdout)["packages"]
    return next(package["version"] for package in packages if package["name"] == EXECUTABLE)


def owned_bundle(path: Path) -> bool:
    if path.is_symlink() or not path.is_dir():
        return False
    try:
        with (path / "Contents/Info.plist").open("rb") as stream:
            info = plistlib.load(stream)
    except (OSError, ValueError):
        return False
    return (
        info.get("CFBundleIdentifier") == BUNDLE_ID
        and info.get("CFBundleExecutable") == EXECUTABLE
        and (path / "Contents/MacOS" / EXECUTABLE).is_file()
    )


def is_running(path: Path) -> bool:
    executable_path = str(path / "Contents/MacOS" / EXECUTABLE)
    result = subprocess.run(
        ["ps", "-axo", "command="], check=True, text=True, capture_output=True
    )
    return any(
        command == executable_path or command.startswith(executable_path + " ")
        for command in result.stdout.splitlines()
    )


def build_icon(staging: Path, resources: Path) -> None:
    if not ICON_SOURCE.is_file():
        raise SystemExit(f"Missing app icon source: {ICON_SOURCE}")
    iconset = staging / f"{ICON_NAME}.iconset"
    iconset.mkdir()
    for points in (16, 32, 128, 256, 512):
        for scale in (1, 2):
            pixels = points * scale
            suffix = "@2x" if scale == 2 else ""
            output = iconset / f"icon_{points}x{points}{suffix}.png"
            subprocess.run(
                ["sips", "-z", str(pixels), str(pixels), str(ICON_SOURCE), "--out", str(output)],
                cwd=ROOT,
                check=True,
                stdout=subprocess.DEVNULL,
            )
    run("iconutil", "-c", "icns", str(iconset), "-o", str(resources / f"{ICON_NAME}.icns"))


def main() -> int:
    if sys.platform != "darwin":
        raise SystemExit("This installer is for macOS only.")

    applications = Path.home() / "Applications"
    applications.mkdir(exist_ok=True)
    destination = applications / BUNDLE_NAME
    if destination.exists() and not owned_bundle(destination):
        raise SystemExit(f"Refusing to replace an unrecognized app: {destination}")
    if is_running(destination):
        raise SystemExit("Close InstPlot Studio before updating its app bundle.")
    if not ICON_SOURCE.is_file():
        raise SystemExit(f"Missing app icon source: {ICON_SOURCE}")

    build_id = datetime.now().strftime("%Y%m%d%H%M")
    build_env = os.environ.copy()
    build_env["INSTPLOT_BUILD_ID"] = build_id
    run(
        "cargo",
        "build",
        "--release",
        "--locked",
        "-p",
        EXECUTABLE,
        "--bin",
        EXECUTABLE,
        env=build_env,
    )
    binary = ROOT / "target/release" / EXECUTABLE
    version = package_version()

    with tempfile.TemporaryDirectory(prefix=".instplot-studio-install-", dir=applications) as temporary:
        staging = Path(temporary)
        bundle = staging / BUNDLE_NAME
        macos = bundle / "Contents/MacOS"
        resources = bundle / "Contents/Resources"
        macos.mkdir(parents=True)
        resources.mkdir()
        installed_binary = macos / EXECUTABLE
        shutil.copy2(binary, installed_binary)
        installed_binary.chmod(installed_binary.stat().st_mode | 0o111)
        build_icon(staging, resources)

        info = {
            "CFBundleDevelopmentRegion": "en",
            "CFBundleDisplayName": "InstPlot Studio",
            "CFBundleExecutable": EXECUTABLE,
            "CFBundleIconFile": ICON_NAME,
            "CFBundleIdentifier": BUNDLE_ID,
            "CFBundleName": "InstPlot Studio",
            "CFBundlePackageType": "APPL",
            "CFBundleDocumentTypes": [
                {
                    "CFBundleTypeExtensions": ["instplot"],
                    "CFBundleTypeName": "InstPlot Studio Project",
                    "CFBundleTypeRole": "Editor",
                    "LSHandlerRank": "Owner",
                    "LSItemContentTypes": ["com.instplot.studio.project"],
                }
            ],
            "CFBundleShortVersionString": version,
            "CFBundleVersion": build_id,
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
        run("codesign", "--force", "--sign", "-", "--identifier", BUNDLE_ID, str(bundle))
        run("codesign", "--verify", "--strict", str(bundle))

        previous = staging / "previous-app"
        if destination.exists():
            destination.rename(previous)
        try:
            bundle.rename(destination)
        except Exception:
            if previous.exists():
                previous.rename(destination)
            raise

    if LSREGISTER.is_file():
        run(str(LSREGISTER), "-f", str(destination))
    subprocess.run(["mdimport", "-f", str(destination)], check=False)
    print(
        f"Installed {destination} "
        f"(version {version}, build {build_id}, bundle ID {BUNDLE_ID})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

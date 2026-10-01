#!/usr/bin/env python3
"""Isolated real-GUI helper acceptance fixtures; never touches an installed app.

This exercises the post-verification helper transaction with real Studio builds.
It does NOT claim to validate signed DMG preparation or the download UI. It never
writes a health receipt: only the running candidate app may produce that receipt.
Close fixture windows through the normal UI after observing helper-ready.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import plistlib
import secrets
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
QA_ROOT = ROOT / "target/macos-update-qa"
EXECUTABLE = Path("Contents/MacOS/instplot-studio")


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def private_json(path: Path, value: object) -> None:
    with path.open("x", encoding="utf-8") as stream:
        os.chmod(path, 0o600)
        json.dump(value, stream)
        stream.flush()
        os.fsync(stream.fileno())


def identity(binary: Path) -> str:
    result = subprocess.run([str(binary), "--product-info"], check=True,
                            capture_output=True, text=True, timeout=45)
    fields = result.stdout.strip().split("\t")
    if len(fields) != 3 or fields[:2] != ["InstPlot Studio", "instplot-studio"]:
        raise ValueError("Not a Studio product binary")
    return fields[2]


def bundle(path: Path, binary: Path, version: str) -> None:
    path.mkdir(mode=0o700)
    (path / "Contents/MacOS").mkdir(parents=True)
    shutil.copy2(binary, path / EXECUTABLE)
    with (path / "Contents/Info.plist").open("xb") as stream:
        plistlib.dump({"CFBundleIdentifier": "com.instplot.studio",
                      "CFBundleExecutable": "instplot-studio",
                      "CFBundlePackageType": "APPL",
                      "CFBundleName": "InstPlot Studio Update QA",
                      "CFBundleShortVersionString": version,
                      "CFBundleVersion": version}, stream)
    subprocess.run(["/usr/bin/codesign", "--force", "--sign", "-", str(path)],
                   check=True, capture_output=True, timeout=45)


def validated_root(path: Path) -> Path:
    root = path.resolve(strict=True)
    if root.parent != QA_ROOT.resolve(strict=True) or not root.name.startswith("run."):
        raise ValueError("Use a unique run directory under target/macos-update-qa")
    return root


def setup(root: Path, old: Path, new: Path, scenario: str) -> None:
    os.chmod(root, 0o700)
    (root / ".metadata_never_index").touch(exist_ok=True)
    previous, candidate = identity(old), identity(new)
    if previous == candidate:
        raise ValueError("Use two distinct real Studio versions")
    case = root / scenario
    case.mkdir(mode=0o700)
    (case / ".metadata_never_index").touch()
    target = case / "Studio Update QA.app"
    bundle(target, old, previous)
    transaction_id, nonce = secrets.token_hex(16), secrets.token_hex(32)
    directory = case / f".instplot-studio-update-{transaction_id}"
    directory.mkdir(mode=0o700)
    bundle(directory / "candidate.app", new, candidate)
    project = case / "QA saved project.instplot"
    subprocess.run([str(target / EXECUTABLE), "--create-project", str(project)],
                   check=True, capture_output=True, timeout=45)
    transaction = {"schema": 1, "id": transaction_id, "nonce": nonce,
                   "identity": {"product": "instplot-studio",
                                "platform": "macos-aarch64",
                                "installed_path": str(target),
                                "previous_version": previous,
                                "candidate_version": candidate,
                                "candidate_sha256": digest(new),
                                "candidate_size": new.stat().st_size},
                   "stage": "waiting_for_exit", "candidate_process": None,
                   "last_error": None}
    request = {"directory": str(directory), "transaction": transaction,
               "previous_binary_hash": digest(target / EXECUTABLE),
               "candidate_binary_hash": digest(directory / "candidate.app" / EXECUTABLE),
               "resume_project": str(project)}
    private_json(directory / "request.json", request)
    private_json(directory / "transaction.json", transaction)
    shutil.copy2(target / EXECUTABLE, directory / "update-helper")
    if scenario == "health-write-failure":
        # A real candidate must reject the invalid receipt destination and exit.
        # Do not fake a receipt or modify either real product binary.
        (directory / "health.json").mkdir()
    evidence = {"scenario": scenario, "installed_path": str(target),
                "transaction_path": str(directory), "project": str(project),
                "project_sha256": digest(project), "previous_version": previous,
                "candidate_version": candidate,
                "previous_sha256": request["previous_binary_hash"],
                "candidate_sha256": request["candidate_binary_hash"]}
    log = (case / "old-gui.log").open("xb")
    old_process = subprocess.Popen([str(target / EXECUTABLE), str(project)],
                                   stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                                   start_new_session=True)
    log.close()
    evidence["old_pid"] = old_process.pid
    private_json(case / "qa-evidence.json", evidence)
    print(json.dumps({"scenario": scenario, "old_pid": old_process.pid,
                      "app": str(target), "next": "observe GUI, then start-helper"}))


def start_helper(case: Path) -> None:
    evidence = json.loads((case / "qa-evidence.json").read_text())
    directory = Path(evidence["transaction_path"])
    if (directory / "helper-ready").exists() or (case / "helper-launch.json").exists():
        raise ValueError("Helper already launched; do not launch a second one")
    log = (directory / "update.log").open("xb")
    process = subprocess.Popen([str(directory / "update-helper"), "--apply-update", str(directory)],
                               stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                               start_new_session=True)
    log.close()
    private_json(case / "helper-launch.json", {"pid": process.pid})
    print(json.dumps({"helper_pid": process.pid, "next": "observe helper-ready, close old GUI normally"}))


def inspect(case: Path) -> None:
    evidence = json.loads((case / "qa-evidence.json").read_text())
    directory = Path(evidence["transaction_path"])
    state = json.loads((directory / "transaction.json").read_text())
    ready_path = directory / "helper-ready"
    ready = json.loads(ready_path.read_text()) if ready_path.is_file() else None
    launch_path = case / "helper-launch.json"
    helper = json.loads(launch_path.read_text()) if launch_path.is_file() else None
    ready_matches = bool(ready and helper
                         and ready["transaction_id"] == state["id"]
                         and ready["nonce"] == state["nonce"]
                         and ready["process_id"] == helper["pid"])
    result = {"stage": state["stage"], "helper_ready_matches": ready_matches,
              "project_unchanged": digest(Path(evidence["project"])) == evidence["project_sha256"],
              "installed_version": identity(Path(evidence["installed_path"]) / EXECUTABLE),
              "backup_preserved": (directory / "previous.app").is_dir(),
              "failed_candidate_preserved": (directory / "failed-candidate.app").is_dir()}
    receipt_path = directory / "health.json"
    receipt = json.loads(receipt_path.read_text()) if receipt_path.is_file() else None
    result["real_health_receipt_matches"] = bool(receipt
        and receipt["transaction_id"] == state["id"] and receipt["nonce"] == state["nonce"]
        and receipt["version"] == evidence["candidate_version"]
        and receipt["installed_path"] == evidence["installed_path"]
        and receipt["initialized"] and receipt["window_ready"]
        and state["candidate_process"] == [receipt["process_id"], receipt["process_started"]])
    print(json.dumps(result))
    if state["stage"] not in ("completed", "rolled_back", "failed_before_apply"):
        return
    expected = "completed" if evidence["scenario"] == "success" else "rolled_back"
    assert state["stage"] == expected, result
    assert result["project_unchanged"] and result["helper_ready_matches"], result
    version = evidence["candidate_version"] if expected == "completed" else evidence["previous_version"]
    expected_hash = evidence["candidate_sha256"] if expected == "completed" else evidence["previous_sha256"]
    assert result["installed_version"] == version, result
    assert digest(Path(evidence["installed_path"]) / EXECUTABLE) == expected_hash, result
    if expected == "completed":
        assert result["backup_preserved"] and result["real_health_receipt_matches"], result
    else:
        assert result["failed_candidate_preserved"] and not result["real_health_receipt_matches"], result
    executable = Path(evidence["installed_path"]) / EXECUTABLE
    subprocess.run([str(executable), "--check-project", evidence["project"]],
                   check=True, capture_output=True, timeout=45)
    for suffix, command in (("png", "--export-fixed-png"), ("pdf", "--export-fixed-pdf")):
        destination = case / f"accepted-{state['stage']}.{suffix}"
        subprocess.run([str(executable), command, str(destination)],
                       check=True, capture_output=True, timeout=45)
        assert destination.stat().st_size > 1000, destination
    result["headless_project_and_png_pdf"] = True
    evidence["helper_acceptance"] = result
    # No PID, absolute path or nonce is published from this machine-local evidence.
    output = case / "qa-export-result.json"
    if not output.exists():
        private_json(output, result)
    print(json.dumps(result))


def main() -> None:
    if sys.platform != "darwin":
        raise SystemExit("This fixture requires macOS and an interactive GUI session")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("setup", "start-helper", "inspect"))
    parser.add_argument("--run-root", required=True, type=Path)
    parser.add_argument("--scenario", choices=("success", "health-write-failure"), required=True)
    parser.add_argument("--old-binary", type=Path)
    parser.add_argument("--new-binary", type=Path)
    args = parser.parse_args()
    root = validated_root(args.run_root)
    if args.action == "setup":
        if not args.old_binary or not args.new_binary:
            parser.error("setup requires both real product binaries")
        setup(root, args.old_binary.resolve(strict=True), args.new_binary.resolve(strict=True), args.scenario)
    else:
        case = root / args.scenario
        (start_helper if args.action == "start-helper" else inspect)(case)


if __name__ == "__main__":
    main()

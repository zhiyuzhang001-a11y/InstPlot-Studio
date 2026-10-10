#!/usr/bin/env python3
"""Local, exact-source Release/TLS/DMG fixture. No production publishing or UI input.

UI actions belong to the computer-use driver. Keep the archive, temporary keys,
transactions and logs under ignored target/macos-gui-e2e/run.* for inspection.
The TLS CA is compiled ONLY into this archive; never into the product checkout.
"""
from __future__ import annotations

import argparse
import hashlib
import http.server
import json
import os
import plistlib
import re
import secrets
import shutil
import ssl
import subprocess
import sys
import tarfile
import time
from datetime import datetime, timedelta, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ORIGIN = "https://localhost:38444/instplot-studio"
EXECUTABLE = Path("Contents/MacOS/instplot-studio")


def run(*args: str, cwd: Path = ROOT, env: dict | None = None) -> str:
    return subprocess.check_output(args, cwd=cwd, env=env, text=True,
                                   stderr=subprocess.PIPE).strip()


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        h = hashlib.sha256()
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
        return h.hexdigest()


def save(path: Path, value: object) -> None:
    with path.open("x", encoding="utf-8") as stream:
        os.chmod(path, 0o600)
        json.dump(value, stream, ensure_ascii=False, indent=2)
        stream.flush()
        os.fsync(stream.fileno())


def validate_root(path: Path) -> Path:
    root = path.resolve(strict=True)
    if sys.platform != "darwin" or root.parent != (ROOT / "target/macos-gui-e2e").resolve():
        raise ValueError("macOS fixture must stay in this repository's ignored QA root")
    if not root.name.startswith("run.") or path.is_symlink():
        raise ValueError("unique non-symlink run root required")
    return root


def replace_exact(text: str, before: str, after: str, count: int = 1) -> str:
    if text.count(before) != count:
        raise ValueError(f"snapshot anchor count differs: {before[:60]}")
    return text.replace(before, after)


def fixture_versions(cargo: bytes) -> tuple[str, str]:
    sections = re.findall(r"(?ms)^\[workspace\.package\]\s*\n(.*?)(?=^\[|\Z)", cargo.decode("utf-8"))
    if len(sections) != 1:
        raise ValueError("one workspace package version section required")
    versions = re.findall(r'^version\s*=\s*"([^"]+)"\s*$', sections[0], re.MULTILINE)
    if len(versions) != 1:
        raise ValueError("one exact workspace package version required")
    version = versions[0]
    match = re.fullmatch(r"(\d+\.\d+\.\d+-rc\.)([1-9]\d*)", version)
    if not match:
        raise ValueError("isolated GUI fixture requires an exact rc baseline")
    return version, match[1] + str(int(match[2]) + 1)


def prepare(root: Path, source: str) -> None:
    if not re.fullmatch(r"[0-9a-f]{40}", source):
        raise ValueError("exact committed source required")
    os.chmod(root, 0o700)
    (root / ".metadata_never_index").touch()
    archive = root / "source.tar"
    run("git", "archive", "--format=tar", "-o", str(archive), source)
    snapshot = root / "snapshot"
    snapshot.mkdir(mode=0o700)
    with tarfile.open(archive) as stream:
        stream.extractall(snapshot, filter="data")
    (snapshot / ".studio-disposable-mac-fixture").touch()
    if (snapshot / ".git").exists():
        raise ValueError("archive only")
    baseline, candidate = fixture_versions((snapshot / "Cargo.toml").read_bytes())
    ca, ca_key = root / "ca.pem", root / "ca-key.pem"
    leaf, leaf_key = root / "server.pem", root / "server-key.pem"
    run("openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "7",
        "-subj", "/CN=Studio isolated Mac GUI CA", "-addext", "basicConstraints=critical,CA:TRUE",
        "-addext", "keyUsage=critical,keyCertSign,cRLSign",
        "-keyout", str(ca_key), "-out", str(ca))
    csr = root / "server.csr"
    run("openssl", "req", "-new", "-newkey", "rsa:2048", "-nodes", "-subj", "/CN=localhost",
        "-keyout", str(leaf_key), "-out", str(csr))
    ext = root / "server.ext"
    ext.write_bytes(b"basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost\n")
    run("openssl", "x509", "-req", "-in", str(csr), "-CA", str(ca), "-CAkey", str(ca_key),
        "-CAcreateserial", "-days", "7", "-extfile", str(ext), "-out", str(leaf))
    signer = ROOT / "target/debug/instplot-update-signature"
    keys = []
    for key_id in ("mac-gui-fixture", "mac-gui-fixture-next"):
        key = root / f"{key_id}.pem"
        run("openssl", "genpkey", "-algorithm", "ED25519", "-out", str(key))
        os.chmod(key, 0o600)
        keys.append({"id": key_id, "public_key_hex": run(str(signer), "public-key-hex", str(key))})
    for key in (ca_key, leaf_key):
        os.chmod(key, 0o600)
    trust = snapshot / "packaging/update/trust.json"
    trust_before = digest(trust)
    trust.write_bytes(json.dumps({"public_root": ORIGIN, "keys": keys}).encode())
    code = snapshot / "apps/instplot-studio/src/app_update.rs"
    before = code.read_bytes()
    expected = subprocess.check_output(["git", "show", f"{source}:apps/instplot-studio/src/app_update.rs"], cwd=ROOT)
    if before != expected:
        raise ValueError("source bytes differ from exact archive")
    text = before.decode("utf-8")
    anchor = "ureq::Agent::config_builder()\n        .max_redirects(0)"
    addition = '''ureq::Agent::config_builder()
        .tls_config(ureq::tls::TlsConfig::builder()
            .root_certs(ureq::tls::RootCerts::new_with_certs(&[
                ureq::tls::Certificate::from_pem(include_bytes!(
                    "../../../packaging/update/gui-fixture-ca.pem"
                )).expect("isolated fixture CA")
            ])).build())
        .max_redirects(0)'''
    text = replace_exact(text, anchor, addition, 2)
    # Both cache and sequence/throttle state stay isolated; no user cache deletion.
    text = replace_exact(text, 'ProjectDirs::from("com", "InstPlot", "InstPlot Studio")',
                         f'ProjectDirs::from("com", "InstPlotQA", "{root.name}")', 2)
    code.write_bytes(text.encode("utf-8"))
    (snapshot / "packaging/update/gui-fixture-ca.pem").write_bytes(ca.read_bytes())
    # Match the independent QA bundle exactly; never relax bundle identity.
    adapter = snapshot / "apps/instplot-studio/src/update_macos.rs"
    adapter_before = digest(adapter)
    adapter.write_bytes(replace_exact(adapter.read_text(),
        'bundle_id.trim() != "com.instplot.studio"',
        'bundle_id.trim() != "com.instplot.studio.mac-gui-qa"').encode("utf-8"))
    save(root / "provenance.json", {"source_sha": source, "scope": "isolated-Release-TLS-GUI-not-public-binary",
         "baseline_version": baseline, "candidate_version": candidate,
         "fixture_script_sha256": digest(Path(__file__)),
         "before_sha256": hashlib.sha256(before).hexdigest(), "after_sha256": digest(code),
         "ca_sha256": digest(ca), "trust_before_sha256": trust_before, "trust_after_sha256": digest(trust),
         "adapter_before_sha256": adapter_before, "adapter_after_sha256": digest(adapter),
         "bundle_id": "com.instplot.studio.mac-gui-qa",
         "strict_TLS": True, "cache_identity": root.name, "keys": keys})


def bundle(root: Path, binary: Path, version: str, name: str) -> Path:
    if run(str(binary), "--product-info") != f"InstPlot Studio\tinstplot-studio\t{version}":
        raise ValueError("binary version differs")
    path = root / name
    (path / "Contents/MacOS").mkdir(parents=True, mode=0o700)
    shutil.copy2(binary, path / EXECUTABLE)
    with (path / "Contents/Info.plist").open("xb") as stream:
        plistlib.dump({"CFBundleIdentifier": "com.instplot.studio.mac-gui-qa",
            "CFBundleExecutable": "instplot-studio", "CFBundlePackageType": "APPL",
            "CFBundleName": "Studio Mac GUI QA", "CFBundleShortVersionString": version,
            "CFBundleVersion": version, "NSHighResolutionCapable": True}, stream)
    run("codesign", "--force", "--sign", "-", str(path))
    run("codesign", "--verify", "--deep", "--strict", str(path))
    return path


def build(root: Path) -> None:
    provenance = json.loads((root / "provenance.json").read_bytes())
    snapshot = root / "snapshot"
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(ROOT / "target")
    signer = ROOT / "target/debug/instplot-update-signature"
    web = root / "public"
    now = datetime.now(timezone.utc).replace(microsecond=0)
    proofs = []
    baseline, candidate = provenance["baseline_version"], provenance["candidate_version"]
    for sequence, version in enumerate((baseline, candidate), 1):
        if sequence == 2:
            cargo = snapshot / "Cargo.toml"
            cargo.write_bytes(replace_exact(cargo.read_text(), f'version = "{baseline}"', f'version = "{candidate}"').encode())
            lock = snapshot / "Cargo.lock"
            text = lock.read_text()
            pattern = r'(\[\[package\]\]\nname = "(?:instplot-demo|instplot-layout|instplot-studio|instplot-update-signature)"\nversion = ")' + re.escape(baseline) + r'(")'
            text, count = re.subn(pattern, lambda m: m[1] + candidate + m[2], text)
            if count != 4:
                raise ValueError("four workspace versions required; dependency pins unchanged")
            lock.write_bytes(text.encode())
        run("cargo", "build", "--release", "--locked", "--offline", "-p", "instplot-studio",
            "--bin", "instplot-studio", "--features", "in-place-update-preview", cwd=snapshot, env=env)
        source_dir = root / f"dmg-source-{sequence}"
        source_dir.mkdir(mode=0o700)
        app = bundle(source_dir, ROOT / "target/release/instplot-studio", version, "InstPlot Studio.app")
        directory = web / "instplot-studio/releases" / version
        directory.mkdir(parents=True)
        name = f"InstPlot-Studio-{version}-macos-aarch64.dmg"
        dmg = directory / name
        run("hdiutil", "create", "-quiet", "-srcfolder", str(source_dir), "-format", "UDZO", "-fs", "HFS+", "-volname", "Studio Mac GUI QA", str(dmg))
        run("hdiutil", "verify", str(dmg))
        metadata = directory / "metadata" / str(sequence)
        metadata.mkdir(parents=True)
        manifest, sig = metadata / "manifest.json", metadata / "manifest.json.sig"
        save(manifest, {"schema": 1, "product": "instplot-studio", "version": version,
            "channel": "prerelease", "key_id": "mac-gui-fixture", "release_sequence": sequence,
            "published_at": now.isoformat().replace("+00:00", "Z"),
            "expires_at": (now + timedelta(days=7)).isoformat().replace("+00:00", "Z"),
            "notes_url": "https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/releases/",
            "signature_url": f"{ORIGIN}/releases/{version}/metadata/{sequence}/manifest.json.sig",
            "platforms": {"macos-aarch64": {"preferred": "dmg", "packages": [{"id": "dmg", "package_type": "dmg",
                "file_name": name, "minimum_system": "macOS 12", "size_bytes": dmg.stat().st_size,
                "sha256": digest(dmg), "url": f"{ORIGIN}/releases/{version}/{name}"}]}}})
        run(str(signer), "sign", str(root / "mac-gui-fixture.pem"), str(manifest), str(sig))
        run(str(signer), "verify-manifest", "mac-gui-fixture", provenance["keys"][0]["public_key_hex"],
            ORIGIN, str(manifest), str(sig), version)
        proofs.append({"version": version, "binary_sha256": digest(app / EXECUTABLE),
            "dmg_sha256": digest(dmg), "dmg_size": dmg.stat().st_size, "profile": "release", "source_sha": provenance["source_sha"]})
        if sequence == 1:
            shutil.copytree(app, root / "Studio Mac GUI QA.app")
    latest = web / "instplot-studio/channels/prerelease/latest.json"
    latest.parent.mkdir(parents=True)
    shutil.copyfile(web / f"instplot-studio/releases/{baseline}/metadata/1/manifest.json", latest)
    save(root / "build-proof.json", proofs)
    run(str(root / "Studio Mac GUI QA.app" / EXECUTABLE), "--create-project", str(root / "GUI sentinel.instplot"))


def serve(root: Path) -> None:
    public = root / "public"
    class Handler(http.server.SimpleHTTPRequestHandler):
        extensions_map = {".json": "application/json", ".sig": "application/octet-stream", ".dmg": "application/octet-stream"}
        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(public), **kwargs)
        def do_GET(self):
            if not self.path.startswith("/instplot-studio/") or ".." in self.path:
                self.send_error(403)
                return
            super().do_GET()
        def do_POST(self):
            self.send_error(405)
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 38444), Handler)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    context.load_cert_chain(root / "server.pem", root / "server-key.pem")
    server.socket = context.wrap_socket(server.socket, server_side=True)
    print("Isolated TLS service ready", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


def health_failure(root: Path) -> None:
    """Post-verification fault only; never counted as download/button evidence."""
    case = root / "health-failure"
    provenance = json.loads((root / "provenance.json").read_bytes())
    case.mkdir(mode=0o700)
    (case / ".metadata_never_index").touch()
    target = case / "Studio Mac GUI QA.app"
    shutil.copytree(root / "dmg-source-1/InstPlot Studio.app", target)
    project = case / "Rollback sentinel.instplot"
    run(str(target / EXECUTABLE), "--create-project", str(project))
    identifier, nonce = secrets.token_hex(16), secrets.token_hex(32)
    directory = case / f".instplot-studio-update-{identifier}"
    directory.mkdir(mode=0o700)
    candidate = directory / "candidate.app"
    shutil.copytree(root / "dmg-source-2/InstPlot Studio.app", candidate)
    transaction = {"schema": 1, "id": identifier, "nonce": nonce,
        "identity": {"product": "instplot-studio", "platform": "macos-aarch64",
            "installed_path": str(target), "previous_version": provenance["baseline_version"],
            "candidate_version": provenance["candidate_version"], "candidate_sha256": digest(candidate / EXECUTABLE),
            "candidate_size": (candidate / EXECUTABLE).stat().st_size},
        "stage": "waiting_for_exit", "candidate_process": None, "last_error": None}
    save(directory / "request.json", {"directory": str(directory), "transaction": transaction,
        "previous_binary_hash": digest(target / EXECUTABLE), "candidate_binary_hash": digest(candidate / EXECUTABLE),
        "resume_project": str(project)})
    save(directory / "transaction.json", transaction)
    shutil.copy2(target / EXECUTABLE, directory / "update-helper")
    # The real GUI must fail writing its receipt; no fake receipt is supplied.
    (directory / "health.json").mkdir(mode=0o700)
    save(case / "fault-proof.json", {"scope": "post-verification-health-failure-not-download-UI",
        "directory": str(directory), "project_sha256": digest(project),
        "previous_sha256": digest(target / EXECUTABLE)})
    with (case / "old-gui.log").open("xb") as log:
        child = subprocess.Popen([str(target / EXECUTABLE), str(project)], stdin=subprocess.DEVNULL,
            stdout=log, stderr=log, start_new_session=True)
    print(json.dumps({"pid": child.pid, "helper": str(directory / "update-helper"), "directory": str(directory)}), flush=True)
    save(case / "old-exit.json", {"pid": child.pid, "exit_code": child.wait()})


def apply_failure(root: Path) -> None:
    case = root / "health-failure"
    fault = json.loads((case / "fault-proof.json").read_bytes())
    directory = Path(fault["directory"])
    if directory.parent != case or not directory.name.startswith(".instplot-studio-update-"):
        raise ValueError("exact isolated fault transaction required")
    with (case / "helper.log").open("xb") as log:
        child = subprocess.Popen([str(directory / "update-helper"), "--apply-update", str(directory)],
            stdin=subprocess.DEVNULL, stdout=log, stderr=log, start_new_session=True)
    print(json.dumps({"helper_pid": child.pid}), flush=True)
    save(case / "helper-exit.json", {"pid": child.pid, "exit_code": child.wait()})


def inspect_failure(root: Path) -> None:
    case = root / "health-failure"
    fault = json.loads((case / "fault-proof.json").read_bytes())
    directory = Path(fault["directory"])
    target = case / "Studio Mac GUI QA.app"
    state = json.loads((directory / "transaction.json").read_bytes())
    assert state["stage"] == "rolled_back" and state["last_error"]
    assert (directory / "health.json").is_dir()
    assert (directory / "failed-candidate.app").is_dir()
    assert digest(target / EXECUTABLE) == fault["previous_sha256"]
    assert digest(case / "Rollback sentinel.instplot") == fault["project_sha256"]
    assert json.loads((case / "old-exit.json").read_bytes())["exit_code"] == 0
    assert json.loads((case / "helper-exit.json").read_bytes())["exit_code"] != 0
    identity = None
    for _ in range(30):
        matches = [line.strip().split(maxsplit=1)[0] for line in run("ps", "-axo", "pid=,comm=").splitlines()
                   if line.strip().split(maxsplit=1)[-1] == str(target / EXECUTABLE)]
        assert len(matches) == 1
        current = (matches[0], run("ps", "-p", matches[0], "-o", "lstart="))
        identity = identity or current
        assert current == identity
        time.sleep(1)
    run("codesign", "--verify", "--deep", "--strict", str(target))
    run(str(target / EXECUTABLE), "--check-project", str(case / "Rollback sentinel.instplot"))
    save(case / "rollback-proof.json", {"scope": fault["scope"], "stage": "rolled_back",
        "restored_pid": identity[0], "restored_started": identity[1], "single_restored_GUI_30_seconds": True,
        "previous_binary_sha256": fault["previous_sha256"], "project_sha256": fault["project_sha256"],
        "old_exit_code": 0, "codesign_strict": True, "failed_candidate_preserved": True})
    print("Real health failure, safe rollback and single restored project GUI: PASS")


def inspect_success(root: Path, project_hash: str) -> None:
    directories = list(root.glob(".instplot-studio-update-*/transaction.json"))
    if len(directories) != 1 or not re.fullmatch(r"[0-9a-f]{64}", project_hash):
        raise ValueError("unique actual GUI transaction and pre-update project hash required")
    directory = directories[0].parent
    state = json.loads((directory / "transaction.json").read_bytes())
    receipt = json.loads((directory / "health.json").read_bytes())
    proofs = json.loads((root / "build-proof.json").read_bytes())
    target = root / "Studio Mac GUI QA.app"
    if state["stage"] != "completed" or state["last_error"] is not None:
        raise ValueError("real GUI transaction did not complete")
    assert receipt["transaction_id"] == state["id"] and receipt["nonce"] == state["nonce"]
    assert receipt["initialized"] is True and receipt["window_ready"] is True
    assert receipt["version"] == proofs[1]["version"] and receipt["installed_path"] == str(target)
    assert state["candidate_process"] == [receipt["process_id"], receipt["process_started"]]
    assert digest(target / EXECUTABLE) == proofs[1]["binary_sha256"]
    assert digest(directory / "previous.app" / EXECUTABLE) == proofs[0]["binary_sha256"]
    assert digest(root / "GUI sentinel.instplot") == project_hash
    old_exits = list(root.glob("exit-*.json"))
    assert old_exits and all(json.loads(p.read_bytes())["exit_code"] == 0 for p in old_exits)
    pid = str(receipt["process_id"])
    started = time.monotonic()
    while time.monotonic() - started < 30:
        assert run("ps", "-p", pid, "-o", "lstart=") == receipt["process_started"]
        assert run("ps", "-p", pid, "-o", "comm=") == str(target / EXECUTABLE)
        all_processes = run("ps", "-axo", "pid=,comm=")
        bound = [line for line in all_processes.splitlines() if line.strip().split(maxsplit=1)[-1] == str(target / EXECUTABLE)]
        assert len(bound) == 1
        time.sleep(1)
    run("codesign", "--verify", "--deep", "--strict", str(target))
    assert run("lipo", "-archs", str(target / EXECUTABLE)) == "arm64"
    run(str(target / EXECUTABLE), "--check-project", str(root / "GUI sentinel.instplot"))
    for suffix in ("png", "pdf"):
        run(str(target / EXECUTABLE), f"--export-fixed-{suffix}", str(root / f"accepted.{suffix}"))
        assert (root / f"accepted.{suffix}").stat().st_size > 1000
    save(root / "success-proof.json", {"source_sha": proofs[1]["source_sha"],
        "scope": "isolated-Release-TLS-DMG-GUI", "stage": "completed", "last_error": None,
        "candidate_pid": receipt["process_id"], "candidate_started": receipt["process_started"],
        "single_bound_process_30_seconds": True, "project_sha256": project_hash,
        "actual_health_binding": True, "previous_bundle_preserved": True,
        "old_normal_exit_0": True, "codesign_strict": True, "project_png_pdf": True})
    print("Completed transaction, process, project, bundle, backup and export proof: PASS")


def handoff(root: Path) -> None:
    provenance = json.loads((root / "provenance.json").read_bytes())
    proofs = json.loads((root / "build-proof.json").read_bytes())
    assert json.loads((root / "success-proof.json").read_bytes())["stage"] == "completed"
    assert json.loads((root / "health-failure/rollback-proof.json").read_bytes())["stage"] == "rolled_back"
    output = ROOT / "target/macos-update-qa" / f"final-{provenance['source_sha'][:7]}"
    output.mkdir(mode=0o700)
    stage = output / "dmg-source"
    stage.mkdir(mode=0o700)
    version = proofs[1]["version"]
    app = stage / f"InstPlot Studio Mac QA {version}.app"
    shutil.copytree(root / "dmg-source-2/InstPlot Studio.app", app)
    assert digest(app / EXECUTABLE) == proofs[1]["binary_sha256"]
    run("codesign", "--verify", "--deep", "--strict", str(app))
    assert run(str(app / EXECUTABLE), "--product-info").endswith("\t" + version)
    dmg = output / f"InstPlot-Studio-Mac-QA-{version}-aarch64.dmg"
    run("hdiutil", "create", "-srcfolder", str(stage), "-format", "UDZO", "-fs", "HFS+",
        "-volname", f"Studio Mac QA {version}", str(dmg))
    run("hdiutil", "verify", str(dmg))
    evidence = output / "evidence"
    evidence.mkdir(mode=0o700)
    for name in ("provenance.json", "build-proof.json", "success-proof.json"):
        shutil.copy2(root / name, evidence / name)
    shutil.copy2(root / "health-failure/rollback-proof.json", evidence / "rollback-proof.json")
    save(output / "package-proof.json", {"version": version, "source_sha": provenance["source_sha"],
        "binary_sha256": digest(app / EXECUTABLE), "dmg_sha256": digest(dmg), "size_bytes": dmg.stat().st_size,
        "scope": "local-isolated-QA-not-production-release", "bundle_id": provenance["bundle_id"],
        "signed_update_asset": False, "same_tested_binary": True, "codesign": "ad-hoc-not-notarized",
        "update_origin": ORIGIN, "public_updates_supported": False,
        "note": "Repacked exact tested candidate under an independent QA app name. Local TLS fixture only; no production installation or release changes."})
    print(str(output), flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare", "build", "serve", "activate", "launch", "health-failure", "apply-failure", "inspect-failure", "inspect-success", "handoff"))
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--source-sha")
    parser.add_argument("--project-sha256")
    args = parser.parse_args()
    root = validate_root(args.root)
    if args.action == "prepare":
        prepare(root, args.source_sha or "")
    elif args.action == "build":
        build(root)
    elif args.action == "serve":
        serve(root)
    elif args.action == "health-failure":
        health_failure(root)
    elif args.action == "apply-failure":
        apply_failure(root)
    elif args.action == "inspect-failure":
        inspect_failure(root)
    elif args.action == "handoff":
        handoff(root)
    elif args.action == "inspect-success":
        inspect_success(root, args.project_sha256 or "")
    elif args.action == "activate":
        provenance = json.loads((root / "provenance.json").read_bytes())
        shutil.copyfile(root / f"public/instplot-studio/releases/{provenance['candidate_version']}/metadata/2/manifest.json",
                        root / "public/instplot-studio/channels/prerelease/latest.json")
    else:
        with (root / "baseline-gui.log").open("ab") as log:
            child = subprocess.Popen([str(root / "Studio Mac GUI QA.app" / EXECUTABLE), str(root / "GUI sentinel.instplot")],
                stdin=subprocess.DEVNULL, stdout=log, stderr=log, start_new_session=True)
        save(root / f"launch-{child.pid}.json", {"pid": child.pid, "scope": "isolated-baseline"})
        print(json.dumps({"pid": child.pid, "scope": "isolated-baseline"}), flush=True)
        code = child.wait()
        save(root / f"exit-{child.pid}.json", {"pid": child.pid, "exit_code": code})


if __name__ == "__main__":
    main()

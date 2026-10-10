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
import shutil
import ssl
import subprocess
import sys
import tarfile
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
    save(root / "provenance.json", {"source_sha": source, "scope": "isolated-Release-TLS-GUI-not-public-binary",
         "fixture_script_sha256": digest(Path(__file__)),
         "before_sha256": hashlib.sha256(before).hexdigest(), "after_sha256": digest(code),
         "ca_sha256": digest(ca), "trust_before_sha256": trust_before, "trust_after_sha256": digest(trust),
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
    for sequence, version in enumerate(("0.1.2-rc.2", "0.1.2-rc.3"), 1):
        if sequence == 2:
            cargo = snapshot / "Cargo.toml"
            cargo.write_bytes(replace_exact(cargo.read_text(), 'version = "0.1.2-rc.2"', 'version = "0.1.2-rc.3"').encode())
            lock = snapshot / "Cargo.lock"
            text = lock.read_text()
            pattern = r'(\[\[package\]\]\nname = "(?:instplot-demo|instplot-layout|instplot-studio|instplot-update-signature)"\nversion = ")0\.1\.2-rc\.2(")'
            text, count = re.subn(pattern, r'\g<1>0.1.2-rc.3\2', text)
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
    shutil.copyfile(web / "instplot-studio/releases/0.1.2-rc.2/metadata/1/manifest.json", latest)
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
    server.serve_forever()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare", "build", "serve", "activate", "launch"))
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--source-sha")
    args = parser.parse_args()
    root = validate_root(args.root)
    if args.action == "prepare":
        prepare(root, args.source_sha or "")
    elif args.action == "build":
        build(root)
    elif args.action == "serve":
        serve(root)
    elif args.action == "activate":
        shutil.copyfile(root / "public/instplot-studio/releases/0.1.2-rc.3/metadata/2/manifest.json",
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

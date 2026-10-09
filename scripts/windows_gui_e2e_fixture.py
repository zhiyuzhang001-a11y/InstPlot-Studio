#!/usr/bin/env python3
"""Disposable loopback TLS fixture; never a product transport or public release."""
from __future__ import annotations

import argparse
import hashlib
import http.server
import json
import os
import re
import shutil
import ssl
import subprocess
import tempfile
import threading
from urllib.parse import unquote, urlsplit
import urllib.error
import urllib.request
from datetime import datetime, timedelta, timezone
from pathlib import Path

from scripts import windows_gui_qa as QA

FIXTURE_ROOT = "https://localhost:38443/instplot-studio"
SCOPE = "isolated-snapshot-TLS-GUI-fixture-not-public-exact-binary"


def require_runner(path: Path) -> None:
    if os.environ.get("GITHUB_ACTIONS") != "true" or os.environ.get("RUNNER_OS") != "Windows":
        raise ValueError("disposable Windows GitHub runner only")
    if not path.resolve().is_relative_to(Path(os.environ["RUNNER_TEMP"]).resolve()):
        raise ValueError("fixture secrets and snapshots must stay in this runner's temporary directory")


def certificates(output: Path) -> None:
    require_runner(output)
    output.mkdir()
    ca, ca_key = output / "ca.pem", output / "ca-key.pem"
    leaf, leaf_key, csr = output / "server.pem", output / "server-key.pem", output / "server.csr"
    subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "2",
                    "-subj", "/CN=Studio temporary GUI fixture CA", "-addext", "basicConstraints=critical,CA:TRUE",
                    "-addext", "keyUsage=critical,keyCertSign,cRLSign", "-keyout", str(ca_key), "-out", str(ca)], check=True)
    subprocess.run(["openssl", "req", "-new", "-newkey", "rsa:2048", "-nodes", "-subj", "/CN=localhost",
                    "-keyout", str(leaf_key), "-out", str(csr)], check=True)
    extensions = output / "server.ext"
    extensions.write_text("basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost\n")
    subprocess.run(["openssl", "x509", "-req", "-in", str(csr), "-CA", str(ca), "-CAkey", str(ca_key),
                    "-CAcreateserial", "-days", "2", "-extfile", str(extensions), "-out", str(leaf)], check=True)


def verify_service(ca: Path) -> None:
    context = ssl.create_default_context(cafile=str(ca))
    url = FIXTURE_ROOT + "/channels/prerelease/latest.json"
    with urllib.request.urlopen(url, context=context, timeout=5) as response:
        json.load(response)
    for rejected_url, rejected_context in [
        (url, ssl.create_default_context()),
        (url.replace("localhost", "127.0.0.1"), context),
    ]:
        try:
            urllib.request.urlopen(rejected_url, context=rejected_context, timeout=5)
        except urllib.error.URLError as error:
            if not isinstance(error.reason, ssl.SSLCertVerificationError):
                raise AssertionError("TLS negative test failed for a non-certificate reason") from error
        else:
            raise AssertionError("untrusted CA or wrong hostname accepted")
    for path in ("/fixture-index.json", "/../fixture-index.json", "/instplot-studio/../fixture-index.json"):
        try:
            urllib.request.urlopen("https://localhost:38443" + path, context=context, timeout=5)
        except urllib.error.HTTPError as error:
            if error.code != 403:
                raise
        else:
            raise AssertionError("non-public fixture file exposed")


def patch_snapshot(snapshot: Path, certificate: Path, source_sha: str) -> dict:
    """Strict CA roots only in an explicitly marked disposable git archive."""
    require_runner(snapshot)
    require_runner(certificate)
    if not (snapshot / ".studio-disposable-gui-fixture").is_file() or (snapshot / ".git").exists():
        raise ValueError("only a marked disposable archive snapshot may be patched")
    source = snapshot / "apps/instplot-studio/src/app_update.rs"
    before = source.read_bytes()
    if not re.fullmatch(r"[0-9a-f]{40}", source_sha):
        raise ValueError("exact archive source SHA required")
    archived_source = subprocess.check_output(
        ["git", "show", f"{source_sha}:apps/instplot-studio/src/app_update.rs"], cwd=QA.ROOT,
    )
    if before != archived_source:
        raise ValueError("snapshot source differs from exact git archive source")
    text = before.decode()
    anchor = "ureq::Agent::config_builder()\n        .max_redirects(0)"
    if text.count(anchor) != 2:
        raise ValueError("expected exactly the two independent update HTTP agents")
    # No invalid-certificate option, native root-store modification, hostname
    # rewrite, HTTP downgrade, or runtime switch enters the product source.
    addition = '''ureq::Agent::config_builder()
        .tls_config(ureq::tls::TlsConfig::builder()
            .root_certs(ureq::tls::RootCerts::new_with_certs(&[
                ureq::tls::Certificate::from_pem(include_bytes!(
                    "../../../packaging/update/gui-fixture-ca.pem"
                )).expect("isolated fixture CA")
            ])).build())
        .max_redirects(0)'''
    ca = certificate.read_bytes()
    if b"PRIVATE KEY" in ca or b"BEGIN CERTIFICATE" not in ca:
        raise ValueError("fixture root must contain only a public CA certificate")
    (snapshot / "packaging/update/gui-fixture-ca.pem").write_bytes(ca)
    source.write_bytes(text.replace(anchor, addition).encode("utf-8"))
    return {
        "scope": SCOPE, "source_sha": source_sha,
        "patched_file": "apps/instplot-studio/src/app_update.rs",
        "before_sha256": hashlib.sha256(before).hexdigest(),
        "after_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
        "ca_sha256": hashlib.sha256(ca).hexdigest(),
        "strict_certificate_and_hostname_validation": True,
        "agent_count": 2,
    }


def stage(kit: Path, output: Path, private_key: Path, source_sha: str) -> dict:
    require_runner(private_key)
    require_runner(output)
    trust = json.loads((kit / "fixture-trust.json").read_bytes())
    if trust["public_root"] != FIXTURE_ROOT or trust.get("binaries_use_fixture_trust") is not True or not re.fullmatch(r"[0-9a-f]{40}", source_sha):
        raise ValueError("only exact-source loopback fixture kits are supported")
    patch = json.loads((kit / "gui-snapshot-patch.json").read_bytes())
    if patch.get("scope") != SCOPE or patch.get("source_sha") != source_sha or patch.get("agent_count") != 2 or patch.get("strict_certificate_and_hostname_validation") is not True:
        raise ValueError("isolated snapshot patch evidence missing")
    archived_source = subprocess.check_output(
        ["git", "show", f"{source_sha}:apps/instplot-studio/src/app_update.rs"], cwd=QA.ROOT,
    )
    if hashlib.sha256(archived_source).hexdigest() != patch.get("before_sha256"):
        raise ValueError("fixture build source evidence differs")
    proofs = [json.loads(p.read_bytes()) for p in kit.glob("*-windows-in-place.json")]
    if len(proofs) != 2:
        raise ValueError("exactly two release installers required")
    if any(not re.fullmatch(r"\d+\.\d+\.\d+-rc\.\d+", p.get("version", "")) for p in proofs):
        raise ValueError("invalid fixture versions")
    proofs.sort(key=lambda p: int(p["version"].rsplit(".", 1)[1]))
    baseline, candidate = [proof["version"] for proof in proofs]
    if baseline.rsplit(".", 1)[0] != candidate.rsplit(".", 1)[0] or int(candidate.rsplit(".", 1)[1]) != int(baseline.rsplit(".", 1)[1]) + 1:
        raise ValueError("adjacent baseline and candidate required")
    keys = {k["id"]: k["public_key_hex"] for k in trust["keys"]}
    key_id = "windows-preview-fixture"
    if set(keys) != {key_id, "windows-preview-fixture-next"} or len(set(keys.values())) != 2:
        raise ValueError("fixture dual public keys required")
    if private_key.name != "preview-fixture-key.pem":
        raise ValueError("ephemeral fixture key only")
    QA.PREPARE.verify_private_key(private_key, keys[key_id])
    output.mkdir()
    tree = output / "instplot-studio"
    tree.mkdir()
    now = datetime.now(timezone.utc).replace(microsecond=0)
    for sequence, proof in enumerate(proofs, 1):
        if proof.get("scope") != "preview-components-not-accepted-updater" or proof.get("windows_gui_subsystem") != 2 or proof.get("build_profile") != "release" or proof.get("startup_update_check_enabled") is not True:
            raise ValueError("actual release binary with startup checks required")
        version = proof["version"]
        installer = kit / f"InstPlot-Studio-{version}-windows-x86_64-setup.exe"
        if QA.PREPARE.sha256(installer) != proof["installer_sha256"]:
            raise ValueError("exact installer evidence differs")
        spec = output / f"assets-{sequence}.json"
        spec.write_bytes(QA.PREPARE.deterministic_json({"platforms": {"windows-x86_64": {
            "preferred": "inno-setup", "windows_in_place": proof["contract"],
            "packages": [{"id": "inno-setup", "package_type": "exe-installer", "path": str(installer)}],
        }}}))
        manifest, signature, _ = QA.PREPARE.prepare(
            asset_spec_path=spec, version=version, release_sequence=sequence,
            product="instplot-studio", public_root=FIXTURE_ROOT, private_key=private_key,
            public_key_hex=keys[key_id], key_id=key_id,
            notes_url="https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/releases/",
            published_at=now.isoformat().replace("+00:00", "Z"),
            expires_at=(now + timedelta(days=1)).isoformat().replace("+00:00", "Z"),
            output=output / f"signed-{sequence}", allow_windows_in_place=True,
        )
        subprocess.run(QA.PREPARE.signature_tool(
            "verify-manifest", key_id, keys[key_id], FIXTURE_ROOT,
            str(manifest), str(signature), version,
        ), check=True)
        shutil.copytree(manifest.parents[2], tree / "releases" / version)
    latest = tree / "channels/prerelease/latest.json"
    latest.parent.mkdir(parents=True)
    shutil.copyfile(tree / f"releases/{baseline}/metadata/1/manifest.json", latest)
    index = {"scope": SCOPE, "source_sha": source_sha, "root": FIXTURE_ROOT,
             "baseline": baseline, "candidate": candidate, "keys": trust["keys"], "patch": patch}
    (output / "fixture-index.json").write_bytes(QA.PREPARE.deterministic_json(index))
    return index


def serve(root: Path, certificate: Path, private_key: Path, token: str) -> None:
    require_runner(root)
    require_runner(certificate)
    require_runner(private_key)
    index = json.loads((root / "fixture-index.json").read_bytes())
    if index["scope"] != SCOPE or index["root"] != FIXTURE_ROOT or len(token) < 32:
        raise ValueError("invalid loopback fixture service")

    class Handler(http.server.SimpleHTTPRequestHandler):
        extensions_map = {".json": "application/json", ".sig": "application/octet-stream",
                          ".exe": "application/octet-stream"}

        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(root), **kwargs)

        def do_GET(self):
            try:
                path = (root / unquote(urlsplit(self.path).path).lstrip("/")).resolve()
                allowed = path.is_relative_to((root / "instplot-studio").resolve()) and path.is_file()
            except (ValueError, OSError):
                allowed = False
            if not allowed:
                self.send_error(403)
                return
            super().do_GET()

        def do_HEAD(self):
            self.send_error(405)

        def do_POST(self):
            if self.path == "/_fixture/shutdown" and self.headers.get("X-Fixture-Token") == token:
                self.send_response(204)
                self.end_headers()
                threading.Thread(target=server.shutdown, daemon=True).start()
                return
            if self.path != "/_fixture/activate" or self.headers.get("X-Fixture-Token") != token:
                self.send_error(403)
                return
            latest = root / "instplot-studio/channels/prerelease/latest.json"
            candidate = root / f"instplot-studio/releases/{index['candidate']}/metadata/2/manifest.json"
            # Local fixture switch only. No OSS client, public path or signing key.
            with tempfile.NamedTemporaryFile(dir=latest.parent, delete=False) as stream:
                stream.write(candidate.read_bytes())
                stream.flush()
                os.fsync(stream.fileno())
                temporary = stream.name
            os.replace(temporary, latest)
            self.send_response(204)
            self.end_headers()

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 38443), Handler)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    context.load_cert_chain(certificate, private_key)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    try:
        server.serve_forever()
    finally:
        server.server_close()


def main():
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    cert = commands.add_parser("certificates")
    cert.add_argument("--output", type=Path, required=True)
    verification = commands.add_parser("verify-service")
    verification.add_argument("--ca", type=Path, required=True)
    patch = commands.add_parser("patch-snapshot")
    patch.add_argument("--snapshot", type=Path, required=True)
    patch.add_argument("--ca", type=Path, required=True)
    patch.add_argument("--evidence", type=Path, required=True)
    patch.add_argument("--source-sha", required=True)
    staging = commands.add_parser("stage")
    for name in ("kit", "output", "private-key"):
        staging.add_argument(f"--{name}", type=Path, required=True)
    staging.add_argument("--source-sha", required=True)
    service = commands.add_parser("serve")
    for name in ("root", "certificate", "private-key"):
        service.add_argument(f"--{name}", type=Path, required=True)
    service.add_argument("--token", required=True)
    args = parser.parse_args()
    if args.command == "certificates":
        certificates(args.output)
    elif args.command == "verify-service":
        verify_service(args.ca)
    elif args.command == "patch-snapshot":
        require_runner(args.evidence)
        args.evidence.write_bytes(QA.PREPARE.deterministic_json(patch_snapshot(args.snapshot, args.ca, args.source_sha)))
    elif args.command == "stage":
        stage(args.kit, args.output, args.private_key, args.source_sha)
    else:
        serve(args.root, args.certificate, args.private_key, args.token)


if __name__ == "__main__":
    main()

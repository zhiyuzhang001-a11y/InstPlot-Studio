#!/usr/bin/env python3
"""Isolated Windows GUI QA staging/publication. Never uses production signing keys."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import re
import shutil
import subprocess
import urllib.error
from datetime import datetime, timedelta, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PUBLIC_BASE = "https://instplot-release.oss-cn-beijing.aliyuncs.com"
QA_PREFIX = "instplot-studio/windows-gui-qa"
BUCKET = "instplot-release"


def load_module(name: str, relative: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


PREPARE = load_module("qa_prepare", "packaging/update/prepare_oss_release.py")
VERIFY = load_module("qa_verify", "scripts/verify_public_release.py")


def public_root(identity: str) -> str:
    if not re.fullmatch(r"[1-9][0-9]*-[1-9][0-9]*", identity):
        raise ValueError("QA identity must be a GitHub run ID and attempt")
    return f"{PUBLIC_BASE}/{QA_PREFIX}/{identity}"


def stage(kit: Path, output: Path, identity: str, source_sha: str, private_key: Path) -> dict:
    endpoint = public_root(identity)
    if not re.fullmatch(r"[0-9a-f]{40}", source_sha):
        raise ValueError("source must be an exact commit SHA")
    if os.environ.get("GITHUB_ACTIONS") != "true" or os.environ.get("RUNNER_OS") != "Windows":
        raise ValueError("stage only on a disposable Windows GitHub runner")
    if identity != f"{os.environ.get('GITHUB_RUN_ID')}-{os.environ.get('GITHUB_RUN_ATTEMPT')}":
        raise ValueError("QA identity differs from this build")
    private_key = private_key.resolve()
    if private_key.name != "preview-fixture-key.pem" or not private_key.is_relative_to(Path(os.environ["RUNNER_TEMP"]).resolve()):
        raise ValueError("only this runner's ephemeral fixture key may be used")
    if output.exists():
        raise ValueError("refuse to overwrite QA staging")
    trust = json.loads((kit / "fixture-trust.json").read_bytes())
    if trust.get("public_root") != endpoint or trust.get("binaries_use_fixture_trust") is not True:
        raise ValueError("snapshot trust must exactly match the isolated public endpoint")
    keys = trust.get("keys")
    if not isinstance(keys, list) or len(keys) != 2 or {key.get("id") for key in keys} != {"windows-preview-fixture", "windows-preview-fixture-next"}:
        raise ValueError("invalid test-only trust pair")
    if any(not re.fullmatch(r"[0-9a-f]{64}", key.get("public_key_hex", "")) for key in keys) or keys[0]["public_key_hex"] == keys[1]["public_key_hex"]:
        raise ValueError("invalid or duplicate test public keys")
    current_key = next(key["public_key_hex"] for key in keys if key["id"] == "windows-preview-fixture")
    PREPARE.verify_private_key(private_key, current_key)
    proofs = [json.loads(path.read_bytes()) for path in kit.glob("*-windows-in-place.json")]
    if len(proofs) != 2:
        raise ValueError("two exact compatible installers are required")
    for proof in proofs:
        if proof.get("scope") != "preview-components-not-accepted-updater" or not re.fullmatch(r"\d+\.\d+\.\d+-rc\.\d+", proof.get("version", "")):
            raise ValueError("invalid preview proof")
        if proof.get("windows_gui_subsystem") != 2:
            raise ValueError("preview installer lacks GUI-subsystem build evidence")
        PREPARE.validate_windows_contract(proof["contract"])
        installer = kit / f"InstPlot-Studio-{proof['version']}-windows-x86_64-setup.exe"
        if installer.is_symlink() or PREPARE.sha256(installer) != proof.get("installer_sha256"):
            raise ValueError("installer differs from its exact-file evidence")
    proofs.sort(key=lambda item: tuple(map(int, re.findall(r"\d+", item["version"]))))
    old, new = (item["version"] for item in proofs)
    if old.rsplit(".", 1)[0] != new.rsplit(".", 1)[0] or int(new.rsplit(".", 1)[1]) != int(old.rsplit(".", 1)[1]) + 1:
        raise ValueError("preview versions must be distinct adjacent RC snapshots")
    now = datetime.now(timezone.utc).replace(microsecond=0)
    output.mkdir(parents=True)
    tree = output / "public"
    tree.mkdir()
    for sequence, proof in enumerate(proofs, 1):
        version = proof["version"]
        installer = kit / f"InstPlot-Studio-{version}-windows-x86_64-setup.exe"
        spec = output / f"{version}-assets.json"
        spec.write_bytes(PREPARE.deterministic_json({"platforms": {"windows-x86_64": {
            "preferred": "inno-setup", "windows_in_place": proof["contract"],
            "packages": [{"id": "inno-setup", "package_type": "exe-installer", "path": str(installer)}],
        }}}))
        manifest, signature, _ = PREPARE.prepare(
            asset_spec_path=spec, version=version, release_sequence=sequence,
            product="instplot-studio", public_root=endpoint, private_key=private_key,
            public_key_hex=current_key, key_id="windows-preview-fixture",
            notes_url="https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/releases/",
            published_at=now.isoformat().replace("+00:00", "Z"),
            expires_at=(now + timedelta(days=30)).isoformat().replace("+00:00", "Z"),
            output=output / f"signed-{sequence}", allow_windows_in_place=True,
        )
        subprocess.run(PREPARE.signature_tool("verify", current_key, str(manifest), str(signature)), check=True)
        shutil.copytree(manifest.parents[2], tree / "releases" / version)
    # IMPORTANT: first expose OLD latest so its real installed GUI can retain
    # authenticated recovery metadata. Candidate latest requires a later explicit activation.
    latest = tree / "channels" / "prerelease" / "latest.json"
    latest.parent.mkdir(parents=True)
    shutil.copyfile(tree / "releases" / old / "metadata" / "1" / "manifest.json", latest)
    index = {
        "scope": "windows-gui-qa-not-production-not-GUI-accepted",
        "identity": identity, "source_sha": source_sha, "public_root": endpoint,
        "keys": keys, "baseline": old, "candidate": new,
        "initial_latest": old, "candidate_requires_explicit_activation": True,
        "install_only_in_separate_windows_account": False,
        "update_state_is_source_scoped": True,
    }
    (tree / "qa-index.json").write_bytes(PREPARE.deterministic_json(index))
    (output / "inventory.json").write_bytes(PREPARE.deterministic_json({
        path.relative_to(tree).as_posix(): {"sha256": PREPARE.sha256(path), "size": path.stat().st_size}
        for path in sorted(tree.rglob("*")) if path.is_file()
    }))
    return validate(output, identity)


def validate(output: Path, identity: str) -> dict:
    endpoint = public_root(identity)
    tree = output / "public"
    index = json.loads((tree / "qa-index.json").read_bytes())
    if index.get("identity") != identity or index.get("public_root") != endpoint or index.get("scope") != "windows-gui-qa-not-production-not-GUI-accepted":
        raise ValueError("QA staging identity mismatch")
    if not re.fullmatch(r"[0-9a-f]{40}", index.get("source_sha", "")) or index.get("candidate_requires_explicit_activation") is not True or index.get("install_only_in_separate_windows_account") is not False or index.get("update_state_is_source_scoped") is not True or index.get("initial_latest") != index.get("baseline"):
        raise ValueError("invalid QA source or bootstrap policy")
    inventory = json.loads((output / "inventory.json").read_bytes())
    if any(path.is_symlink() for path in tree.rglob("*")):
        raise ValueError("no symlinks in QA staging")
    actual = {path.relative_to(tree).as_posix() for path in tree.rglob("*") if path.is_file()}
    expected = {"qa-index.json", "channels/prerelease/latest.json"}
    keys = {key["id"]: key["public_key_hex"] for key in index["keys"]}
    if set(keys) != {"windows-preview-fixture", "windows-preview-fixture-next"}:
        raise ValueError("only fixture keys are allowed")
    if len(index["keys"]) != 2 or len(set(keys.values())) != 2 or any(not re.fullmatch(r"[0-9a-f]{64}", key) for key in keys.values()):
        raise ValueError("invalid fixture public key pair")
    for sequence, version in enumerate((index["baseline"], index["candidate"]), 1):
        if not re.fullmatch(r"\d+\.\d+\.\d+-rc\.\d+", version):
            raise ValueError("invalid preview version")
        prefix = f"releases/{version}"
        name = f"InstPlot-Studio-{version}-windows-x86_64-setup.exe"
        manifest = tree / prefix / "metadata" / str(sequence) / "manifest.json"
        signature = manifest.with_name("manifest.json.sig")
        expected.update({f"{prefix}/{name}", f"{prefix}/metadata/{sequence}/manifest.json", f"{prefix}/metadata/{sequence}/manifest.json.sig"})
        key_id = VERIFY.verify_signature(manifest, signature, keys, output)
        data = VERIFY.parse_json(manifest.read_bytes())
        subprocess.run(PREPARE.signature_tool(
            "verify-manifest", key_id, keys[key_id], endpoint,
            str(manifest), str(signature), version,
        ), check=True)
        if data.get("version") != version or data.get("release_sequence") != sequence or data.get("product") != "instplot-studio" or data.get("channel") != "prerelease" or data.get("key_id") != key_id:
            raise ValueError("signed preview identity differs")
        if datetime.fromisoformat(data["expires_at"].replace("Z", "+00:00")) <= datetime.now(timezone.utc):
            raise ValueError("fixture metadata expired; do not publish")
        if data.get("signature_url") != f"{endpoint}/{prefix}/metadata/{sequence}/manifest.json.sig":
            raise ValueError("signature URL escapes exact QA revision")
        if set(data["platforms"]) != {"windows-x86_64"}:
            raise ValueError("QA is Windows x64 only")
        platform = data["platforms"]["windows-x86_64"]
        PREPARE.validate_windows_contract(platform["windows_in_place"])
        if platform["preferred"] != "inno-setup" or len(platform["packages"]) != 1:
            raise ValueError("exactly one Inno installer is required")
        package = platform["packages"][0]
        installer = tree / prefix / name
        if package.get("id") != "inno-setup" or package.get("package_type") != "exe-installer" or package.get("file_name") != name or package.get("url") != f"{endpoint}/{prefix}/{name}" or package.get("sha256") != PREPARE.sha256(installer) or package.get("size_bytes") != installer.stat().st_size:
            raise ValueError("signed installer binding differs")
    if index["baseline"].rsplit(".", 1)[0] != index["candidate"].rsplit(".", 1)[0] or int(index["candidate"].rsplit(".", 1)[1]) != int(index["baseline"].rsplit(".", 1)[1]) + 1:
        raise ValueError("invalid baseline/candidate pair")
    if actual != expected or set(inventory) != expected:
        raise ValueError("unexpected or missing public files (no keys or production paths allowed)")
    for relative, proof in inventory.items():
        path = tree / relative
        if proof != {"sha256": PREPARE.sha256(path), "size": path.stat().st_size}:
            raise ValueError("public staging differs from inventory")
    if (tree / "channels/prerelease/latest.json").read_bytes() != (tree / "releases" / index["baseline"] / "metadata/1/manifest.json").read_bytes():
        raise ValueError("bootstrap latest must expose the baseline, not the candidate")
    return index


def verify_public(output: Path, identity: str, version: str, *, latest: bool = False) -> None:
    index = validate(output, identity)
    sequence = 1 if version == index["baseline"] else 2
    relative = "channels/prerelease/latest.json" if latest else f"releases/{version}/metadata/{sequence}/manifest.json"
    VERIFY.verify_public_release(
        latest_url=f"{index['public_root']}/{relative}", allowed_prefix=index["public_root"],
        keys={key["id"]: key["public_key_hex"] for key in index["keys"]},
        expected_product="instplot-studio", expected_channel="prerelease",
        expected_version=version, cache_bust=identity, timeout=60,
    )


def publish(output: Path, identity: str, ossutil: str, activate: bool) -> dict:
    index = validate(output, identity)
    # Never bypass the protected GitHub Environment by changing branch rules.
    if os.environ.get("GITHUB_ACTIONS") != "true" or os.environ.get("GITHUB_REF") != "refs/heads/main" or os.environ.get("GITHUB_REPOSITORY") != "zhiyuzhang001-a11y/InstPlot-Studio":
        raise ValueError("upload requires the reviewed main workflow and protected environment")
    tree = output / "public"
    prefix = f"{QA_PREFIX}/{identity}"

    def put(relative: str, path: Path, overwrite: bool = False):
        # Only the exact run's QA pointer may ever be overwritten.
        if overwrite and relative != "channels/prerelease/latest.json":
            raise ValueError("only the isolated QA latest pointer may change")
        subprocess.run([
            ossutil, "api", "put-object", "--bucket", BUCKET, "--key", f"{prefix}/{relative}",
            "--body", f"file://{path.resolve()}", "--forbid-overwrite", "false" if overwrite else "true",
            "--content-type", "application/json" if relative.endswith(".json") else "application/octet-stream",
            "--cache-control", "no-store" if relative == "channels/prerelease/latest.json" else "public, max-age=31536000, immutable",
            "--endpoint", "https://oss-cn-beijing.aliyuncs.com", "--region", "cn-beijing",
        ], check=True)

    pointer = "channels/prerelease/latest.json"
    if activate:
        # Called separately, ONLY after baseline GUI has retained recovery metadata.
        # Repeated activation is idempotent; no version/release/package is rewritten.
        verify_public(output, identity, index["baseline"])
        verify_public(output, identity, index["candidate"])
        current = output / "observed-latest.json"
        if current.exists():
            raise ValueError("refuse to overwrite previous observation")
        VERIFY.download(f"{index['public_root']}/{pointer}", current, allowed=VERIFY.AllowedPrefix(index["public_root"]), maximum_bytes=256 * 1024, timeout=60)
        candidate = tree / "releases" / index["candidate"] / "metadata/2/manifest.json"
        if current.read_bytes() not in ((tree / pointer).read_bytes(), candidate.read_bytes()):
            raise ValueError("QA pointer has an unexpected value; stop")
        if current.read_bytes() != candidate.read_bytes():
            put(pointer, candidate, overwrite=True)
        verify_public(output, identity, index["candidate"], latest=True)
    else:
        for path in sorted(tree.rglob("*")):
            if path.is_file() and path.relative_to(tree).as_posix() != pointer:
                relative = path.relative_to(tree).as_posix()
                put_if_missing(relative, path, index, output, put)
        verify_public(output, identity, index["baseline"])
        verify_public(output, identity, index["candidate"])
        put_if_missing(pointer, tree / pointer, index, output, put)
        verify_public(output, identity, index["baseline"], latest=True)
    (output / "publication-result.json").write_bytes(PREPARE.deterministic_json({
        "scope": "verified-public-QA-assets-not-Windows-GUI-acceptance",
        "identity": identity, "source_sha": index["source_sha"],
        "latest_version": index["candidate"] if activate else index["baseline"],
        "public_root": index["public_root"], "signatures_and_package_hashes_verified": True,
        "verified_at": datetime.now(timezone.utc).isoformat(),
    }))
    return index


def put_if_missing(relative: str, path: Path, index: dict, output: Path, put) -> None:
    """Resume a partial upload without overwriting ANY existing object."""
    observation_root = output / "upload-observations"
    observation_root.mkdir(exist_ok=True)
    # Equal-content manifest/latest may share a hash; use a path-derived name.
    observed = observation_root / hashlib.sha256(relative.encode()).hexdigest()
    if observed.exists():
        raise ValueError("previous upload observation exists; use a fresh artifact extraction")
    try:
        VERIFY.download(f"{index['public_root']}/{relative}", observed,
                        allowed=VERIFY.AllowedPrefix(index["public_root"]),
                        maximum_bytes=path.stat().st_size, timeout=60)
    except urllib.error.HTTPError as error:
        if error.code != 404:
            raise
        put(relative, path)
        return
    if observed.stat().st_size != path.stat().st_size or PREPARE.sha256(observed) != PREPARE.sha256(path):
        raise ValueError("existing QA object differs; refuse overwrite")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("stage", "validate", "publish", "activate"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--identity", required=True)
    parser.add_argument("--kit", type=Path)
    parser.add_argument("--source-sha")
    parser.add_argument("--private-key", type=Path)
    parser.add_argument("--ossutil")
    args = parser.parse_args()
    if args.action == "stage":
        if args.kit is None or args.source_sha is None or args.private_key is None:
            parser.error("stage requires kit, source SHA and ephemeral fixture key")
        result = stage(args.kit, args.output, args.identity, args.source_sha, args.private_key)
    elif args.action == "validate":
        result = validate(args.output, args.identity)
    else:
        if not args.ossutil:
            parser.error("publish/activate requires ossutil")
        result = publish(args.output, args.identity, args.ossutil, args.action == "activate")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()

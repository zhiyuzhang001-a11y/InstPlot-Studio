#!/usr/bin/env python3
"""Prepare immutable InstPlot Studio update metadata and signed OSS staging files."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.parse import quote, urlsplit, urlunsplit


SEMVER = re.compile(
    r"^(0|[1-9][0-9]*)\."
    r"(0|[1-9][0-9]*)\."
    r"(0|[1-9][0-9]*)"
    r"(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$"
)
PLATFORM = re.compile(r"^[a-z0-9]+(?:-[a-z0-9_]+)+$")
PACKAGE_ID = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
KEY_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def deterministic_json(payload: dict[str, Any]) -> bytes:
    return (
        json.dumps(
            payload,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
        + "\n"
    ).encode("utf-8")


def https_url(value: str, name: str) -> str:
    parsed = urlsplit(value)
    if (
        parsed.scheme != "https"
        or not parsed.hostname
        or parsed.username is not None
        or parsed.password is not None
        or parsed.fragment
    ):
        raise ValueError(f"{name} must be an HTTPS URL without credentials or fragment")
    path = parsed.path.rstrip("/")
    return urlunsplit((parsed.scheme, parsed.netloc, path, parsed.query, ""))


def channel_for(version: str) -> str:
    match = SEMVER.fullmatch(version)
    if not match:
        raise ValueError(f"invalid SemVer version: {version}")
    return "prerelease" if match.group(4) else "stable"


def verify_private_key(private_key: Path, public_key_hex: str) -> None:
    if not re.fullmatch(r"[0-9a-fA-F]{64}", public_key_hex):
        raise ValueError("Ed25519 public key must contain 64 hexadecimal characters")
    public_der = subprocess.check_output(
        [
            "openssl",
            "pkey",
            "-in",
            str(private_key),
            "-pubout",
            "-outform",
            "DER",
        ]
    )
    if len(public_der) != 44 or public_der[-32:].hex() != public_key_hex.lower():
        raise ValueError("private key does not match the configured public key")


def load_asset_spec(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict) or not isinstance(payload.get("platforms"), dict):
        raise ValueError("asset specification must contain a platforms object")
    if not payload["platforms"]:
        raise ValueError("asset specification must contain at least one platform")
    return payload


def prepare(
    *,
    asset_spec_path: Path,
    version: str,
    release_sequence: int,
    product: str,
    public_root: str,
    private_key: Path,
    public_key_hex: str,
    key_id: str,
    notes_url: str,
    published_at: str,
    expires_at: str,
    output: Path,
) -> tuple[Path, Path, Path]:
    channel = channel_for(version)
    if release_sequence < 1:
        raise ValueError("release sequence must be positive")
    if not KEY_ID.fullmatch(key_id):
        raise ValueError("invalid key ID")
    if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", product):
        raise ValueError("invalid product slug")
    public_root = https_url(public_root, "public root")
    notes_url = https_url(notes_url, "notes URL")
    for value, name in ((published_at, "published_at"), (expires_at, "expires_at")):
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
        if parsed.tzinfo is None:
            raise ValueError(f"{name} must include a timezone")
    if datetime.fromisoformat(expires_at.replace("Z", "+00:00")) <= datetime.fromisoformat(
        published_at.replace("Z", "+00:00")
    ):
        raise ValueError("expires_at must be later than published_at")

    private_key = private_key.resolve()
    if not private_key.is_file():
        raise ValueError(f"private key not found: {private_key}")
    verify_private_key(private_key, public_key_hex)
    spec = load_asset_spec(asset_spec_path)

    staged_root = output.resolve() / product
    if staged_root.exists():
        shutil.rmtree(staged_root)
    release_dir = staged_root / "releases" / version
    metadata_dir = release_dir / "metadata" / str(release_sequence)
    channel_dir = staged_root / "channels" / channel
    metadata_dir.mkdir(parents=True)
    channel_dir.mkdir(parents=True)

    platforms: dict[str, Any] = {}
    for platform, platform_spec in sorted(spec["platforms"].items()):
        if not isinstance(platform, str) or not PLATFORM.fullmatch(platform):
            raise ValueError(f"invalid platform key: {platform!r}")
        if not isinstance(platform_spec, dict):
            raise ValueError(f"platform {platform} must be an object")
        preferred = platform_spec.get("preferred")
        packages = platform_spec.get("packages")
        if not isinstance(preferred, str) or not isinstance(packages, list) or not packages:
            raise ValueError(f"platform {platform} needs preferred and non-empty packages")

        prepared_packages: list[dict[str, Any]] = []
        seen_ids: set[str] = set()
        seen_names: set[str] = set()
        for package in packages:
            if not isinstance(package, dict):
                raise ValueError(f"package for {platform} must be an object")
            package_id = package.get("id")
            package_type = package.get("package_type")
            source_value = package.get("path")
            if not isinstance(package_id, str) or not PACKAGE_ID.fullmatch(package_id):
                raise ValueError(f"invalid package ID for {platform}")
            if package_id in seen_ids:
                raise ValueError(f"duplicate package ID for {platform}: {package_id}")
            if not isinstance(package_type, str) or not PACKAGE_ID.fullmatch(package_type):
                raise ValueError(f"invalid package type for {platform}/{package_id}")
            if not isinstance(source_value, str):
                raise ValueError(f"missing package path for {platform}/{package_id}")
            source = Path(source_value).resolve()
            if not source.is_file() or source.is_symlink():
                raise ValueError(f"package is missing or is a symlink: {source}")
            if version not in source.name:
                raise ValueError(f"package filename does not contain version {version}: {source.name}")
            if source.name in seen_names:
                raise ValueError(f"duplicate package filename: {source.name}")
            seen_ids.add(package_id)
            seen_names.add(source.name)
            destination = release_dir / source.name
            shutil.copy2(source, destination)
            entry: dict[str, Any] = {
                "file_name": source.name,
                "id": package_id,
                "package_type": package_type,
                "sha256": sha256(destination),
                "size_bytes": destination.stat().st_size,
                "url": f"{public_root}/releases/{quote(version)}/{quote(source.name)}",
            }
            minimum_system = package.get("minimum_system")
            if minimum_system is not None:
                if not isinstance(minimum_system, str) or not minimum_system.strip():
                    raise ValueError(f"invalid minimum_system for {platform}/{package_id}")
                entry["minimum_system"] = minimum_system.strip()
            prepared_packages.append(entry)
        if preferred not in seen_ids:
            raise ValueError(f"preferred package does not exist for {platform}: {preferred}")
        platforms[platform] = {
            "packages": sorted(prepared_packages, key=lambda item: item["id"]),
            "preferred": preferred,
        }

    signature_url = (
        f"{public_root}/releases/{quote(version)}/metadata/"
        f"{release_sequence}/manifest.json.sig"
    )
    manifest = {
        "channel": channel,
        "expires_at": expires_at,
        "key_id": key_id,
        "notes_url": notes_url,
        "platforms": platforms,
        "product": product,
        "published_at": published_at,
        "release_sequence": release_sequence,
        "schema": 1,
        "signature_url": signature_url,
        "version": version,
    }
    manifest_bytes = deterministic_json(manifest)
    manifest_path = metadata_dir / "manifest.json"
    signature_path = metadata_dir / "manifest.json.sig"
    latest_path = channel_dir / "latest.json"
    manifest_path.write_bytes(manifest_bytes)
    latest_path.write_bytes(manifest_bytes)
    subprocess.run(
        [
            "openssl",
            "pkeyutl",
            "-sign",
            "-rawin",
            "-inkey",
            str(private_key),
            "-in",
            str(manifest_path),
            "-out",
            str(signature_path),
        ],
        check=True,
    )
    if signature_path.stat().st_size != 64:
        raise ValueError("OpenSSL produced an invalid Ed25519 signature")
    if latest_path.read_bytes() != manifest_path.read_bytes():
        raise AssertionError("channel latest bytes differ from immutable manifest")
    return manifest_path, signature_path, latest_path


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--assets", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--release-sequence", required=True, type=int)
    parser.add_argument("--product", default="instplot-studio")
    parser.add_argument("--public-root", required=True)
    parser.add_argument("--private-key", required=True, type=Path)
    parser.add_argument("--public-key-hex", required=True)
    parser.add_argument("--key-id", required=True)
    parser.add_argument("--notes-url", required=True)
    parser.add_argument("--published-at", default=utc_now())
    parser.add_argument("--expires-at", required=True)
    parser.add_argument("--output", type=Path, default=Path("target/oss-upload"))
    args = parser.parse_args()
    try:
        paths = prepare(
            asset_spec_path=args.assets,
            version=args.version,
            release_sequence=args.release_sequence,
            product=args.product,
            public_root=args.public_root,
            private_key=args.private_key,
            public_key_hex=args.public_key_hex,
            key_id=args.key_id,
            notes_url=args.notes_url,
            published_at=args.published_at,
            expires_at=args.expires_at,
            output=args.output,
        )
    except (OSError, ValueError, json.JSONDecodeError, subprocess.CalledProcessError) as error:
        parser.error(str(error))
    for path in paths:
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

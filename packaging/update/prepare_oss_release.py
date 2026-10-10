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


def signature_tool(*arguments: str) -> list[str]:
    return [
        "cargo",
        "run",
        "--locked",
        "--quiet",
        "--package",
        "instplot-update-signature",
        "--",
        *arguments,
    ]


def verify_private_key(private_key: Path, public_key_hex: str) -> None:
    if not re.fullmatch(r"[0-9a-fA-F]{64}", public_key_hex):
        raise ValueError("Ed25519 public key must contain 64 hexadecimal characters")
    actual = subprocess.check_output(
        signature_tool("public-key-hex", str(private_key)), text=True
    ).strip()
    if actual != public_key_hex.lower():
        raise ValueError("private key does not match the configured public key")


def validate_windows_contract(value: Any) -> dict[str, Any]:
    protocols = {
        "schema", "helper_protocol", "transaction_schema",
        "candidate_health_protocol", "recovery_health_protocol",
    }
    hashes = {"executable_sha256", "license_sha256"}
    if not isinstance(value, dict) or set(value) != protocols | hashes:
        raise ValueError("invalid windows_in_place contract fields")
    if any(type(value[key]) is not int or value[key] != 1 for key in protocols):
        raise ValueError("unsupported windows_in_place protocol")
    if any(not isinstance(value[key], str) or not re.fullmatch(r"[0-9a-f]{64}", value[key]) for key in hashes):
        raise ValueError("invalid windows_in_place fixed-file hash")
    return dict(value)


def load_asset_spec(path: Path, *, allow_windows_in_place: bool = False) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict) or not isinstance(payload.get("platforms"), dict):
        raise ValueError("asset specification must contain a platforms object")
    if not payload["platforms"]:
        raise ValueError("asset specification must contain at least one platform")
    for platform, platform_spec in payload["platforms"].items():
        if isinstance(platform_spec, dict) and "windows_in_place" in platform_spec:
            if not allow_windows_in_place:
                raise ValueError("windows_in_place publication is not enabled")
            if platform != "windows-x86_64":
                raise ValueError("windows_in_place requires windows-x86_64")
            validate_windows_contract(platform_spec["windows_in_place"])
            packages = platform_spec.get("packages")
            if platform_spec.get("preferred") != "inno-setup" or not isinstance(packages, list) or not any(
                isinstance(package, dict) and package.get("id") == "inno-setup"
                and package.get("package_type") == "exe-installer" for package in packages
            ):
                raise ValueError("windows_in_place requires the preferred Inno installer")
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
    allow_windows_in_place: bool = False,
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

    # Reject unsupported capability declarations before accessing signing
    # material or replacing any existing staging directory.
    spec = load_asset_spec(asset_spec_path, allow_windows_in_place=allow_windows_in_place)
    private_key = private_key.resolve()
    if not private_key.is_file():
        raise ValueError(f"private key not found: {private_key}")
    verify_private_key(private_key, public_key_hex)

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
        if "windows_in_place" in platform_spec:
            platforms[platform]["windows_in_place"] = validate_windows_contract(platform_spec["windows_in_place"])

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
        signature_tool(
            "sign", str(private_key), str(manifest_path), str(signature_path)
        ),
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
    parser.add_argument("--windows-in-place-metadata", action="store_true",
                        help="Explicitly sign validated Windows compatibility metadata; does not enable the client entry or certify GUI acceptance")
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
            allow_windows_in_place=args.windows_in_place_metadata,
        )
    except (OSError, ValueError, json.JSONDecodeError, subprocess.CalledProcessError) as error:
        parser.error(str(error))
    for path in paths:
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

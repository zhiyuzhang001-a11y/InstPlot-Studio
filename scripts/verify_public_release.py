#!/usr/bin/env python3
"""Verify an InstPlot Studio update channel from its public HTTPS endpoint."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import tempfile
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path, PurePosixPath
from typing import Any


MAX_MANIFEST_BYTES = 256 * 1024
MAX_SIGNATURE_BYTES = 64
MAX_REDIRECTS = 4
USER_AGENT = "instplot-release-verifier/1"


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def parse_json(raw: bytes) -> dict[str, Any]:
    value = json.loads(raw.decode("utf-8"), object_pairs_hook=unique_object)
    if not isinstance(value, dict):
        raise ValueError("manifest root must be an object")
    return value


class AllowedPrefix:
    def __init__(self, value: str) -> None:
        parsed = urllib.parse.urlsplit(value.rstrip("/") + "/")
        if (
            parsed.scheme != "https"
            or not parsed.hostname
            or parsed.username is not None
            or parsed.password is not None
            or parsed.query
            or parsed.fragment
        ):
            raise ValueError("allowed prefix must be a plain HTTPS origin and path")
        self.scheme = parsed.scheme
        self.hostname = parsed.hostname.lower()
        self.port = parsed.port or 443
        self.path = self._normalized_path(parsed.path)

    @staticmethod
    def _normalized_path(path: str) -> str:
        decoded = path
        for _ in range(3):
            updated = urllib.parse.unquote(decoded)
            if updated == decoded:
                break
            decoded = updated
        if "\\" in decoded or "\x00" in decoded:
            raise ValueError("URL path contains forbidden characters")
        parts = PurePosixPath(decoded).parts
        if ".." in parts:
            raise ValueError("URL path escapes its allowed prefix")
        normalized = "/" + "/".join(part for part in parts if part not in ("/", "."))
        return normalized.rstrip("/") + "/"

    def validate(self, value: str, *, allow_query: bool = False) -> str:
        parsed = urllib.parse.urlsplit(value)
        if (
            parsed.scheme != self.scheme
            or not parsed.hostname
            or parsed.hostname.lower() != self.hostname
            or (parsed.port or 443) != self.port
            or parsed.username is not None
            or parsed.password is not None
            or parsed.fragment
            or (parsed.query and not allow_query)
        ):
            raise ValueError(f"URL is outside the allowed origin: {value}")
        normalized = self._normalized_path(parsed.path)
        if not normalized.startswith(self.path):
            raise ValueError(f"URL is outside the allowed path: {value}")
        return value


class LimitedRedirectHandler(urllib.request.HTTPRedirectHandler):
    def __init__(self, allowed: AllowedPrefix) -> None:
        super().__init__()
        self.allowed = allowed
        self.count = 0

    def redirect_request(self, req: Any, fp: Any, code: int, msg: str, headers: Any, newurl: str) -> Any:
        self.count += 1
        if self.count > MAX_REDIRECTS:
            raise urllib.error.HTTPError(newurl, code, "too many redirects", headers, fp)
        self.allowed.validate(newurl, allow_query=True)
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def with_cache_bust(url: str, value: str) -> str:
    parsed = urllib.parse.urlsplit(url)
    query = urllib.parse.parse_qsl(parsed.query, keep_blank_values=True)
    query.append(("verify", value))
    return urllib.parse.urlunsplit(
        (parsed.scheme, parsed.netloc, parsed.path, urllib.parse.urlencode(query), "")
    )


def download(
    url: str,
    destination: Path,
    *,
    allowed: AllowedPrefix,
    maximum_bytes: int,
    timeout: float,
) -> None:
    allowed.validate(url, allow_query=True)
    opener = urllib.request.build_opener(LimitedRedirectHandler(allowed))
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, "Accept-Encoding": "identity"})
    with opener.open(request, timeout=timeout) as response:
        allowed.validate(response.geturl(), allow_query=True)
        encoding = response.headers.get("Content-Encoding")
        if encoding not in (None, "", "identity"):
            raise ValueError(f"unsupported Content-Encoding: {encoding}")
        declared = response.headers.get("Content-Length")
        if declared is not None and int(declared) > maximum_bytes:
            raise ValueError("response exceeds the configured byte limit")
        total = 0
        with destination.open("xb") as output:
            while True:
                block = response.read(min(1024 * 1024, maximum_bytes + 1 - total))
                if not block:
                    break
                total += len(block)
                if total > maximum_bytes:
                    raise ValueError("response exceeds the configured byte limit")
                output.write(block)


def verify_signature(manifest: Path, signature: Path, keys: dict[str, str], root: Path) -> str:
    if signature.stat().st_size != 64:
        raise ValueError("Ed25519 signature must be exactly 64 bytes")
    successful: list[str] = []
    for key_id, public_key_hex in sorted(keys.items()):
        if len(public_key_hex) != 64:
            raise ValueError(f"invalid public key length for {key_id}")
        public_der = root / f"{key_id}.der"
        public_der.write_bytes(bytes.fromhex("302a300506032b6570032100") + bytes.fromhex(public_key_hex))
        result = subprocess.run(
            [
                "openssl",
                "pkeyutl",
                "-verify",
                "-pubin",
                "-rawin",
                "-keyform",
                "DER",
                "-inkey",
                str(public_der),
                "-in",
                str(manifest),
                "-sigfile",
                str(signature),
            ],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        if result.returncode == 0:
            successful.append(key_id)
    if len(successful) != 1:
        raise ValueError(f"manifest verified with {len(successful)} configured keys")
    return successful[0]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def verify_public_release(
    *,
    latest_url: str,
    allowed_prefix: str,
    keys: dict[str, str],
    expected_product: str,
    expected_channel: str,
    expected_version: str,
    cache_bust: str,
    timeout: float,
) -> None:
    allowed = AllowedPrefix(allowed_prefix)
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        manifest_path = root / "latest.json"
        signature_path = root / "manifest.json.sig"
        download(
            with_cache_bust(latest_url, cache_bust),
            manifest_path,
            allowed=allowed,
            maximum_bytes=MAX_MANIFEST_BYTES,
            timeout=timeout,
        )
        untrusted = parse_json(manifest_path.read_bytes())
        signature_url = untrusted.get("signature_url")
        if not isinstance(signature_url, str):
            raise ValueError("manifest signature_url is missing")
        allowed.validate(signature_url)
        download(
            with_cache_bust(signature_url, cache_bust),
            signature_path,
            allowed=allowed,
            maximum_bytes=MAX_SIGNATURE_BYTES,
            timeout=timeout,
        )
        successful_key = verify_signature(manifest_path, signature_path, keys, root)
        manifest = parse_json(manifest_path.read_bytes())
        expected = {
            "schema": 1,
            "product": expected_product,
            "channel": expected_channel,
            "version": expected_version,
            "key_id": successful_key,
        }
        for field, value in expected.items():
            if manifest.get(field) != value:
                raise ValueError(f"unexpected {field}: {manifest.get(field)!r}")
        platforms = manifest.get("platforms")
        if not isinstance(platforms, dict) or not platforms:
            raise ValueError("manifest platforms are missing")
        for platform, platform_value in sorted(platforms.items()):
            if not isinstance(platform_value, dict):
                raise ValueError(f"invalid platform object: {platform}")
            preferred = platform_value.get("preferred")
            packages = platform_value.get("packages")
            if not isinstance(preferred, str) or not isinstance(packages, list) or not packages:
                raise ValueError(f"invalid platform packages: {platform}")
            package_ids: set[str] = set()
            for index, package in enumerate(packages):
                if not isinstance(package, dict):
                    raise ValueError(f"invalid package entry: {platform}/{index}")
                package_id = package.get("id")
                url = package.get("url")
                size = package.get("size_bytes")
                expected_hash = package.get("sha256")
                if not isinstance(package_id, str) or package_id in package_ids:
                    raise ValueError(f"invalid or duplicate package ID: {platform}/{package_id}")
                package_ids.add(package_id)
                if not isinstance(url, str):
                    raise ValueError(f"missing package URL: {platform}/{package_id}")
                allowed.validate(url)
                if not isinstance(size, int) or size < 1:
                    raise ValueError(f"invalid package size: {platform}/{package_id}")
                if not isinstance(expected_hash, str) or len(expected_hash) != 64:
                    raise ValueError(f"invalid package hash: {platform}/{package_id}")
                destination = root / f"asset-{platform}-{index}"
                download(
                    with_cache_bust(url, cache_bust),
                    destination,
                    allowed=allowed,
                    maximum_bytes=size,
                    timeout=timeout,
                )
                if destination.stat().st_size != size or sha256(destination) != expected_hash:
                    raise ValueError(f"public package does not match manifest: {platform}/{package_id}")
            if preferred not in package_ids:
                raise ValueError(f"preferred package is absent: {platform}/{preferred}")


def parse_key(value: str) -> tuple[str, str]:
    key_id, separator, public_key_hex = value.partition("=")
    if not separator or not key_id or len(public_key_hex) != 64:
        raise argparse.ArgumentTypeError("public key must use KEY_ID=64_HEX format")
    try:
        bytes.fromhex(public_key_hex)
    except ValueError as error:
        raise argparse.ArgumentTypeError("public key is not hexadecimal") from error
    return key_id, public_key_hex.lower()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--latest-url", required=True)
    parser.add_argument("--allowed-prefix", required=True)
    parser.add_argument("--public-key", action="append", required=True, type=parse_key)
    parser.add_argument("--expected-product", default="instplot-studio")
    parser.add_argument("--expected-channel", choices=("stable", "prerelease"), required=True)
    parser.add_argument("--expected-version", required=True)
    parser.add_argument("--cache-bust", required=True)
    parser.add_argument("--timeout", type=float, default=60.0)
    args = parser.parse_args()
    try:
        keys = dict(args.public_key)
        if len(keys) != len(args.public_key):
            raise ValueError("duplicate public key ID")
        verify_public_release(
            latest_url=args.latest_url,
            allowed_prefix=args.allowed_prefix,
            keys=keys,
            expected_product=args.expected_product,
            expected_channel=args.expected_channel,
            expected_version=args.expected_version,
            cache_bust=args.cache_bust,
            timeout=args.timeout,
        )
    except (OSError, ValueError, UnicodeDecodeError, json.JSONDecodeError, urllib.error.URLError) as error:
        parser.error(str(error))
    print("Public OSS release verification passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

//! Signed, platform-aware update metadata contracts.
//!
//! Network transport and UI state live outside this module. Keeping the trust
//! and manifest rules independent makes them testable without production URL
//! overrides or a running GUI.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use semver::Version;
use serde::Deserialize;
use url::Url;

const PRODUCT_SLUG: &str = "instplot-studio";
const MANIFEST_SCHEMA: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    Stable,
    Prerelease,
}

impl UpdateChannel {
    pub fn for_version(version: &Version) -> Self {
        if version.pre.is_empty() {
            Self::Stable
        } else {
            Self::Prerelease
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UpdatePackage {
    pub file_name: String,
    pub id: String,
    pub minimum_system: Option<String>,
    pub package_type: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UpdatePlatform {
    pub packages: Vec<UpdatePackage>,
    pub preferred: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UpdateManifest {
    pub channel: UpdateChannel,
    pub expires_at: String,
    pub key_id: String,
    pub notes_url: String,
    pub platforms: BTreeMap<String, UpdatePlatform>,
    pub product: String,
    pub published_at: String,
    pub release_sequence: u64,
    pub schema: u32,
    pub signature_url: String,
    pub version: String,
}

#[derive(Clone, Copy, Debug)]
pub struct TrustedUpdateKey {
    pub id: &'static str,
    pub bytes: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllowedUpdateRoot {
    scheme: String,
    host: String,
    port: u16,
    path_prefix: String,
}

impl AllowedUpdateRoot {
    pub fn parse(value: &str) -> Result<Self, SignedManifestError> {
        let parsed = Url::parse(value)
            .map_err(|error| SignedManifestError::InvalidUrl(error.to_string()))?;
        if parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(SignedManifestError::InvalidUrl(value.to_owned()));
        }
        let path_prefix = normalized_path(parsed.path())?;
        Ok(Self {
            scheme: parsed.scheme().to_owned(),
            host: parsed.host_str().unwrap_or_default().to_ascii_lowercase(),
            port: parsed.port_or_known_default().unwrap_or(443),
            path_prefix,
        })
    }

    pub fn permits(&self, value: &str) -> Result<(), SignedManifestError> {
        let parsed = Url::parse(value)
            .map_err(|error| SignedManifestError::InvalidUrl(error.to_string()))?;
        if parsed.scheme() != self.scheme
            || parsed.host_str().map(str::to_ascii_lowercase).as_deref() != Some(self.host.as_str())
            || parsed.port_or_known_default().unwrap_or(443) != self.port
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(SignedManifestError::UrlOutsideAllowedRoot(value.to_owned()));
        }
        let path = normalized_path(parsed.path())?;
        if !path.starts_with(&self.path_prefix) {
            return Err(SignedManifestError::UrlOutsideAllowedRoot(value.to_owned()));
        }
        Ok(())
    }
}

fn normalized_path(value: &str) -> Result<String, SignedManifestError> {
    let mut decoded = value.to_owned();
    for _ in 0..3 {
        let next = percent_encoding::percent_decode_str(&decoded)
            .decode_utf8()
            .map_err(|_| SignedManifestError::InvalidUrl(value.to_owned()))?
            .into_owned();
        if next == decoded {
            break;
        }
        decoded = next;
    }
    if decoded.contains(['\\', '\0']) {
        return Err(SignedManifestError::InvalidUrl(value.to_owned()));
    }
    let mut parts = Vec::new();
    for part in decoded.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err(SignedManifestError::InvalidUrl(value.to_owned())),
            other => parts.push(other),
        }
    }
    Ok(format!("/{}/", parts.join("/")))
}

#[derive(Debug)]
pub enum SignedManifestError {
    InvalidJson(String),
    InvalidSignature,
    InvalidUrl(String),
    KeyIdMismatch,
    Manifest(String),
    UrlOutsideAllowedRoot(String),
}

impl fmt::Display for SignedManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(message) => write!(formatter, "invalid update manifest: {message}"),
            Self::InvalidSignature => formatter.write_str("update manifest signature is invalid"),
            Self::InvalidUrl(value) => write!(formatter, "invalid update URL: {value}"),
            Self::KeyIdMismatch => {
                formatter.write_str("update manifest key ID does not match signer")
            }
            Self::Manifest(message) => write!(formatter, "invalid update manifest: {message}"),
            Self::UrlOutsideAllowedRoot(value) => {
                write!(formatter, "update URL is outside the allowed root: {value}")
            }
        }
    }
}

impl std::error::Error for SignedManifestError {}

#[derive(Deserialize)]
struct SignatureLocator {
    signature_url: String,
}

pub fn verify_signed_manifest(
    raw: &[u8],
    signature_bytes: &[u8],
    keys: &[TrustedUpdateKey],
    allowed_root: &AllowedUpdateRoot,
    expected_channel: UpdateChannel,
) -> Result<UpdateManifest, SignedManifestError> {
    let locator: SignatureLocator = serde_json::from_slice(raw)
        .map_err(|error| SignedManifestError::InvalidJson(error.to_string()))?;
    allowed_root.permits(&locator.signature_url)?;
    let signature = Signature::from_slice(signature_bytes)
        .map_err(|_| SignedManifestError::InvalidSignature)?;
    let mut successful_key = None;
    for key in keys {
        let verifier = VerifyingKey::from_bytes(&key.bytes)
            .map_err(|_| SignedManifestError::InvalidSignature)?;
        if verifier.verify(raw, &signature).is_ok() {
            if successful_key.is_some() {
                return Err(SignedManifestError::InvalidSignature);
            }
            successful_key = Some(key.id);
        }
    }
    let successful_key = successful_key.ok_or(SignedManifestError::InvalidSignature)?;
    let manifest: UpdateManifest = serde_json::from_slice(raw)
        .map_err(|error| SignedManifestError::InvalidJson(error.to_string()))?;
    if manifest.key_id != successful_key {
        return Err(SignedManifestError::KeyIdMismatch);
    }
    validate_manifest(&manifest, allowed_root, expected_channel)?;
    Ok(manifest)
}

fn validate_manifest(
    manifest: &UpdateManifest,
    allowed_root: &AllowedUpdateRoot,
    expected_channel: UpdateChannel,
) -> Result<(), SignedManifestError> {
    if manifest.schema != MANIFEST_SCHEMA {
        return Err(SignedManifestError::Manifest(
            "unsupported schema".to_owned(),
        ));
    }
    if manifest.product != PRODUCT_SLUG {
        return Err(SignedManifestError::Manifest("wrong product".to_owned()));
    }
    if manifest.channel != expected_channel {
        return Err(SignedManifestError::Manifest(
            "wrong release channel".to_owned(),
        ));
    }
    if manifest.release_sequence == 0 {
        return Err(SignedManifestError::Manifest(
            "release sequence must be positive".to_owned(),
        ));
    }
    let version = Version::parse(&manifest.version)
        .map_err(|error| SignedManifestError::Manifest(error.to_string()))?;
    if UpdateChannel::for_version(&version) != manifest.channel {
        return Err(SignedManifestError::Manifest(
            "version and channel disagree".to_owned(),
        ));
    }
    allowed_root.permits(&manifest.signature_url)?;
    if manifest.platforms.is_empty() {
        return Err(SignedManifestError::Manifest(
            "no platform assets".to_owned(),
        ));
    }
    for (platform, assets) in &manifest.platforms {
        if assets.packages.is_empty() {
            return Err(SignedManifestError::Manifest(format!(
                "platform {platform} has no packages"
            )));
        }
        let mut ids = BTreeSet::new();
        for package in &assets.packages {
            if package.id.is_empty()
                || package.package_type.is_empty()
                || package.file_name.is_empty()
                || package.size_bytes == 0
                || package.sha256.len() != 64
                || !package.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
                || !ids.insert(package.id.as_str())
            {
                return Err(SignedManifestError::Manifest(format!(
                    "invalid package in {platform}"
                )));
            }
            allowed_root.permits(&package.url)?;
        }
        if !ids.contains(assets.preferred.as_str()) {
            return Err(SignedManifestError::Manifest(format!(
                "preferred package is absent for {platform}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    const ROOT: &str = "https://downloads.example.test/instplot-studio";
    const FIXTURE_PUBLIC_KEY: [u8; 32] = [
        0xea, 0x4a, 0x6c, 0x63, 0xe2, 0x9c, 0x52, 0x0a, 0xbe, 0xf5, 0x50, 0x7b, 0x13, 0x2e, 0xc5,
        0xf9, 0x95, 0x47, 0x76, 0xae, 0xbe, 0xbe, 0x7b, 0x92, 0x42, 0x1e, 0xea, 0x69, 0x14, 0x46,
        0xd2, 0x2c,
    ];

    fn decode_hex(value: &str) -> Vec<u8> {
        let value = value.trim();
        (0..value.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&value[index..index + 2], 16).unwrap())
            .collect()
    }

    fn manifest(key_id: &str, channel: &str, version: &str) -> Vec<u8> {
        format!(
            concat!(
                "{{\"channel\":\"{}\",\"expires_at\":\"2026-12-29T00:00:00Z\",",
                "\"key_id\":\"{}\",\"notes_url\":\"https://github.com/example/release\",",
                "\"platforms\":{{\"linux-x86_64\":{{\"packages\":[{{",
                "\"file_name\":\"InstPlot-Studio-{2}.deb\",\"id\":\"deb\",",
                "\"minimum_system\":\"Ubuntu 22.04\",\"package_type\":\"deb\",",
                "\"sha256\":\"{3}\",\"size_bytes\":10,",
                "\"url\":\"{4}/releases/{2}/InstPlot-Studio-{2}.deb\"}}],",
                "\"preferred\":\"deb\"}}}},\"product\":\"instplot-studio\",",
                "\"published_at\":\"2026-09-29T00:00:00Z\",\"release_sequence\":1,",
                "\"schema\":1,\"signature_url\":\"{4}/releases/{2}/metadata/1/manifest.json.sig\",",
                "\"version\":\"{2}\"}}\n"
            ),
            channel,
            key_id,
            version,
            "ab".repeat(32),
            ROOT
        )
        .into_bytes()
    }

    #[test]
    fn signed_manifest_requires_matching_key_id_and_channel() {
        let signing = SigningKey::from_bytes(&[7; 32]);
        let raw = manifest("current", "prerelease", "0.1.2-rc.1");
        let signature = signing.sign(&raw).to_bytes();
        let key = TrustedUpdateKey {
            id: "current",
            bytes: signing.verifying_key().to_bytes(),
        };
        let allowed = AllowedUpdateRoot::parse(ROOT).unwrap();
        let parsed = verify_signed_manifest(
            &raw,
            &signature,
            &[key],
            &allowed,
            UpdateChannel::Prerelease,
        )
        .unwrap();
        assert_eq!(parsed.version, "0.1.2-rc.1");

        assert!(matches!(
            verify_signed_manifest(
                &raw,
                &signature,
                &[TrustedUpdateKey { id: "next", ..key }],
                &allowed,
                UpdateChannel::Prerelease,
            ),
            Err(SignedManifestError::KeyIdMismatch)
        ));
        assert!(
            verify_signed_manifest(&raw, &signature, &[key], &allowed, UpdateChannel::Stable,)
                .is_err()
        );
    }

    #[test]
    fn python_and_rust_share_one_signed_raw_byte_fixture() {
        let raw = include_bytes!("../tests/fixtures/update-manifest.json");
        let signature = decode_hex(include_str!("../tests/fixtures/update-manifest.sig.hex"));
        let manifest = verify_signed_manifest(
            raw,
            &signature,
            &[TrustedUpdateKey {
                id: "fixture-current",
                bytes: FIXTURE_PUBLIC_KEY,
            }],
            &AllowedUpdateRoot::parse(ROOT).unwrap(),
            UpdateChannel::Prerelease,
        )
        .unwrap();
        assert_eq!(manifest.version, "0.1.2-rc.1");
    }

    #[test]
    fn url_root_rejects_host_and_path_confusion() {
        let allowed = AllowedUpdateRoot::parse(ROOT).unwrap();
        assert!(
            allowed
                .permits("https://downloads.example.test/instplot-studio/releases/file")
                .is_ok()
        );
        for invalid in [
            "https://downloads.example.test.evil/instplot-studio/file",
            "https://downloads.example.test/instplot-studio-evil/file",
            "https://downloads.example.test/instplot-studio/%2e%2e/private",
            "http://downloads.example.test/instplot-studio/file",
        ] {
            assert!(allowed.permits(invalid).is_err(), "accepted {invalid}");
        }
    }
}

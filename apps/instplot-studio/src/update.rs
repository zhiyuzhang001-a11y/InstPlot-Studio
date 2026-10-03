//! Signed, platform-aware update metadata contracts.
//!
//! Network transport and UI state live outside this module. Keeping the trust
//! and manifest rules independent makes them testable without production URL
//! overrides or a running GUI.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use semver::Version;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use url::Url;

const PRODUCT_SLUG: &str = "instplot-studio";
const MANIFEST_SCHEMA: u32 = 1;
pub const GITHUB_RELEASES_ROOT: &str =
    "https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/releases/";

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

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Prerelease => "prerelease",
        }
    }
}

pub fn production_latest_url(channel: UpdateChannel) -> String {
    format!(
        "{PRODUCTION_PUBLIC_ROOT}/channels/{}/latest.json",
        channel.as_str()
    )
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
    /// Absent on legacy releases: download remains supported, automatic
    /// application must fail closed. Covered by the existing raw-byte signature.
    #[serde(default)]
    pub windows_in_place: Option<WindowsInPlaceContract>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WindowsInPlaceContract {
    pub schema: u32,
    pub helper_protocol: u32,
    pub transaction_schema: u32,
    pub candidate_health_protocol: u32,
    pub recovery_health_protocol: u32,
    pub executable_sha256: String,
    pub license_sha256: String,
}

impl WindowsInPlaceContract {
    fn structurally_valid(&self) -> bool {
        self.schema == 1
            && self.helper_protocol > 0
            && self.transaction_schema > 0
            && self.candidate_health_protocol > 0
            && self.recovery_health_protocol > 0
            && [&self.executable_sha256, &self.license_sha256]
                .into_iter()
                .all(|hash| {
                    hash.len() == 64
                        && hash
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                })
    }
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

include!(concat!(env!("OUT_DIR"), "/update_trust.rs"));

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

pub fn signature_url_from_manifest_bytes(raw: &[u8]) -> Result<String, SignedManifestError> {
    reject_duplicate_json_keys(raw)?;
    let locator: SignatureLocator = serde_json::from_slice(raw)
        .map_err(|error| SignedManifestError::InvalidJson(error.to_string()))?;
    Ok(locator.signature_url)
}

pub fn verify_signed_manifest(
    raw: &[u8],
    signature_bytes: &[u8],
    keys: &[TrustedUpdateKey],
    allowed_root: &AllowedUpdateRoot,
    expected_channel: UpdateChannel,
) -> Result<UpdateManifest, SignedManifestError> {
    verify_signed_manifest_at(
        raw,
        signature_bytes,
        keys,
        allowed_root,
        expected_channel,
        OffsetDateTime::now_utc(),
    )
}

pub fn verify_signed_manifest_at(
    raw: &[u8],
    signature_bytes: &[u8],
    keys: &[TrustedUpdateKey],
    allowed_root: &AllowedUpdateRoot,
    expected_channel: UpdateChannel,
    now: OffsetDateTime,
) -> Result<UpdateManifest, SignedManifestError> {
    let signature_url = signature_url_from_manifest_bytes(raw)?;
    allowed_root.permits(&signature_url)?;
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
    validate_manifest(&manifest, allowed_root, expected_channel, now)?;
    Ok(manifest)
}

fn validate_manifest(
    manifest: &UpdateManifest,
    allowed_root: &AllowedUpdateRoot,
    expected_channel: UpdateChannel,
    now: OffsetDateTime,
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
    validate_notes_url(&manifest.notes_url)?;
    let published_at = OffsetDateTime::parse(&manifest.published_at, &Rfc3339)
        .map_err(|error| SignedManifestError::Manifest(error.to_string()))?;
    let expires_at = OffsetDateTime::parse(&manifest.expires_at, &Rfc3339)
        .map_err(|error| SignedManifestError::Manifest(error.to_string()))?;
    if expires_at <= now {
        return Err(SignedManifestError::Manifest(
            "update metadata has expired".to_owned(),
        ));
    }
    if published_at > now + time::Duration::hours(24) || expires_at <= published_at {
        return Err(SignedManifestError::Manifest(
            "invalid update metadata validity window".to_owned(),
        ));
    }
    if expires_at - published_at > time::Duration::days(120) {
        return Err(SignedManifestError::Manifest(
            "update metadata validity window is too long".to_owned(),
        ));
    }
    if manifest.platforms.is_empty() {
        return Err(SignedManifestError::Manifest(
            "no platform assets".to_owned(),
        ));
    }
    for (platform, assets) in &manifest.platforms {
        if let Some(contract) = &assets.windows_in_place
            && (platform != "windows-x86_64" || !contract.structurally_valid())
        {
            return Err(SignedManifestError::Manifest(
                "invalid Windows in-place installation contract".into(),
            ));
        }
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
                || !package
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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

fn validate_notes_url(value: &str) -> Result<(), SignedManifestError> {
    let parsed =
        Url::parse(value).map_err(|error| SignedManifestError::InvalidUrl(error.to_string()))?;
    let allowed = Url::parse(GITHUB_RELEASES_ROOT)
        .expect("the compiled GitHub release root must be a valid URL");
    if parsed.scheme() != "https"
        || parsed.host_str() != allowed.host_str()
        || parsed.port_or_known_default() != allowed.port_or_known_default()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !normalized_path(parsed.path())?.starts_with(&normalized_path(allowed.path())?)
    {
        return Err(SignedManifestError::UrlOutsideAllowedRoot(value.to_owned()));
    }
    Ok(())
}

fn reject_duplicate_json_keys(raw: &[u8]) -> Result<(), SignedManifestError> {
    serde_json::from_slice::<UniqueJson>(raw)
        .map(|_| ())
        .map_err(|error| SignedManifestError::InvalidJson(error.to_string()))
}

struct UniqueJson;

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueJsonVisitor)
    }
}

struct UniqueJsonVisitor;

impl<'de> Visitor<'de> for UniqueJsonVisitor {
    type Value = UniqueJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object keys")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(de::Error::custom(format!("duplicate object key: {key}")));
            }
            map.next_value::<UniqueJson>()?;
        }
        Ok(UniqueJson)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<UniqueJson>()?.is_some() {}
        Ok(UniqueJson)
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
        Ok(UniqueJson)
    }

    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
        Ok(UniqueJson)
    }

    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
        Ok(UniqueJson)
    }

    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
        Ok(UniqueJson)
    }

    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E> {
        Ok(UniqueJson)
    }

    fn visit_string<E>(self, _: String) -> Result<Self::Value, E> {
        Ok(UniqueJson)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueJson)
    }
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
                "\"key_id\":\"{}\",\"notes_url\":\"https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/releases/tag/v{2}\",",
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
        let now = OffsetDateTime::parse("2026-09-29T12:00:00Z", &Rfc3339).unwrap();
        let parsed = verify_signed_manifest_at(
            &raw,
            &signature,
            &[key],
            &allowed,
            UpdateChannel::Prerelease,
            now,
        )
        .unwrap();
        assert_eq!(parsed.version, "0.1.2-rc.1");

        assert!(matches!(
            verify_signed_manifest_at(
                &raw,
                &signature,
                &[TrustedUpdateKey { id: "next", ..key }],
                &allowed,
                UpdateChannel::Prerelease,
                now,
            ),
            Err(SignedManifestError::KeyIdMismatch)
        ));
        assert!(
            verify_signed_manifest_at(
                &raw,
                &signature,
                &[key],
                &allowed,
                UpdateChannel::Stable,
                now,
            )
            .is_err()
        );
    }

    #[test]
    fn python_and_rust_share_one_signed_raw_byte_fixture() {
        let raw = include_bytes!("../tests/fixtures/update-manifest.json");
        let signature = decode_hex(include_str!("../tests/fixtures/update-manifest.sig.hex"));
        let manifest = verify_signed_manifest_at(
            raw,
            &signature,
            &[TrustedUpdateKey {
                id: "fixture-current",
                bytes: FIXTURE_PUBLIC_KEY,
            }],
            &AllowedUpdateRoot::parse(ROOT).unwrap(),
            UpdateChannel::Prerelease,
            OffsetDateTime::parse("2026-09-29T12:00:00Z", &Rfc3339).unwrap(),
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

    #[test]
    fn duplicate_json_keys_are_rejected_before_signature_location_is_trusted() {
        let raw = br#"{"signature_url":"https://downloads.example.test/instplot-studio/a","signature_url":"https://downloads.example.test/instplot-studio/b"}"#;
        assert!(matches!(
            signature_url_from_manifest_bytes(raw),
            Err(SignedManifestError::InvalidJson(_))
        ));
    }
}

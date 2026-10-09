//! Retain already-authenticated metadata for the CURRENT installed version.
//! No guessed archive sequence, network request, watermark decrease, installer
//! execution, or cleanup. Missing/expired evidence blocks automatic preparation.
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{VerifiedWindowsInstallerManifest, WindowsInstallation, invalid};
use crate::update_transaction::TransactionStore;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataPointer {
    schema: u32,
    version: String,
    release_sequence: u64,
    manifest_sha256: String,
}

fn version_directory(root: &Path, version: &str) -> PathBuf {
    let source = crate::AllowedUpdateRoot::parse(crate::PRODUCTION_PUBLIC_ROOT)
        .expect("build-validated update root");
    version_directory_for_source(root, &source, version)
}

fn version_directory_for_source(
    root: &Path,
    source: &crate::AllowedUpdateRoot,
    version: &str,
) -> PathBuf {
    // Production retains its old recovery paths. Isolated feeds cannot clash
    // with another signed record for the same version/sequence in that cache.
    let identity = if source.owns_legacy_state() {
        version.to_owned()
    } else {
        format!("{}|{version}", source.state_namespace())
    };
    root.join(format!("{:x}", Sha256::digest(identity.as_bytes())))
}

fn pointer_at(directory: &Path, version: &str) -> io::Result<MetadataPointer> {
    let raw = read_private(&directory.join("current.json"), 4096)?;
    let pointer: MetadataPointer = serde_json::from_slice(&raw).map_err(invalid)?;
    if pointer.schema != 1
        || pointer.version != version
        || pointer.release_sequence == 0
        || pointer.manifest_sha256.len() != 64
        || !pointer
            .manifest_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid("invalid recovery metadata pointer"));
    }
    Ok(pointer)
}

fn read_private(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    super::validate_private_file(path)?;
    let mut raw = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut raw)?;
    if raw.len() as u64 > limit {
        return Err(invalid("oversized recovery metadata"));
    }
    super::validate_private_file(path)?;
    Ok(raw)
}

/// Called only after a normal latest check has accepted its sequence. The
/// running, registered installation must exactly match the signed manifest.
/// This stores a description, NOT a ready-to-use recovery installer.
pub fn remember_installed_manifest(raw: &[u8], signature: &[u8]) -> io::Result<()> {
    let installed = super::discover_current_installation()
        .map_err(|error| recovery_error("安装身份检查", error))?;
    let metadata = VerifiedWindowsInstallerManifest::from_raw(
        raw,
        signature,
        &installed.version().to_string(),
    )
    .map_err(|error| recovery_error("恢复清单校验", error))?;
    let root = super::private_download_root()
        .map_err(|error| recovery_error("私有下载目录检查/创建", error))?
        .join("recovery-metadata");
    retain_at(&root, &metadata).map_err(|error| recovery_error("恢复记录保存", error))
}

fn recovery_error(stage: &str, error: io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{stage}：{error}"))
}

fn retain_at(root: &Path, metadata: &VerifiedWindowsInstallerManifest) -> io::Result<()> {
    ensure_private_directory(root)?;
    let version = metadata.version().to_string();
    let directory = version_directory(root, &version);
    ensure_private_directory(&directory)?;
    // Serialize concurrent writers. Readers see an old or new atomic pointer,
    // never metadata files that have not both been completely synced.
    let _lock = TransactionStore::lock(&directory)?;
    let manifest_sha256 = metadata.sha256();
    let pointer_path = directory.join("current.json");
    if pointer_path.try_exists()? {
        let previous = pointer_at(&directory, &version)?;
        if metadata.release_sequence() < previous.release_sequence
            || (metadata.release_sequence() == previous.release_sequence
                && manifest_sha256 != previous.manifest_sha256)
        {
            return Err(invalid(
                "recovery metadata sequence regression or conflicting immutable record",
            ));
        }
    }
    let payload = directory.join(&manifest_sha256);
    if payload.try_exists()? {
        super::validate_private_directory(&payload)?;
        if read_private(&payload.join("manifest.json"), 256 * 1024)? != metadata.raw()
            || read_private(&payload.join("manifest.json.sig"), 64)? != metadata.signature()
        {
            return Err(invalid("existing immutable recovery evidence differs"));
        }
    } else {
        super::create_private_directory(&payload)?;
        for (name, bytes) in [
            ("manifest.json", metadata.raw()),
            ("manifest.json.sig", metadata.signature()),
        ] {
            let mut file = super::create_private_file(&payload.join(name))?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }
    }
    let pointer = MetadataPointer {
        schema: 1,
        version,
        release_sequence: metadata.release_sequence(),
        manifest_sha256,
    };
    super::write_private_atomic(
        &pointer_path,
        &serde_json::to_vec(&pointer).map_err(invalid)?,
    )
}

fn ensure_private_directory(path: &Path) -> io::Result<()> {
    // Another instance may have created this exact directory. Accept only
    // after the same strict ownership/DACL validation, never repair its ACL.
    if let Err(error) = super::create_private_directory(path)
        && super::validate_private_directory(path).is_err()
    {
        return Err(error);
    }
    Ok(())
}

/// Resolve ONLY the exact previously registered installed version. The normal
/// update sequence watermark is not read, changed, or reduced by recovery lookup.
/// The metadata must be freshly authenticated and unexpired at preparation.
pub fn cached_recovery_manifest(
    installed: &WindowsInstallation,
) -> io::Result<(PathBuf, VerifiedWindowsInstallerManifest)> {
    super::native::revalidate_installation(installed)?;
    let root = super::private_download_root()?.join("recovery-metadata");
    load_at(
        &root,
        &installed.version().to_string(),
        VerifiedWindowsInstallerManifest::from_raw,
    )
}

fn load_at(
    root: &Path,
    version: &str,
    verify: impl FnOnce(&[u8], &[u8], &str) -> io::Result<VerifiedWindowsInstallerManifest>,
) -> io::Result<(PathBuf, VerifiedWindowsInstallerManifest)> {
    super::validate_private_directory(root)?;
    let directory = version_directory(root, version);
    super::validate_private_directory(&directory)?;
    let pointer = pointer_at(&directory, version)?;
    let payload = directory.join(&pointer.manifest_sha256);
    super::validate_private_directory(&payload)?;
    let raw = read_private(&payload.join("manifest.json"), 256 * 1024)?;
    let signature = read_private(&payload.join("manifest.json.sig"), 64)?;
    if format!("{:x}", Sha256::digest(&raw)) != pointer.manifest_sha256 {
        return Err(invalid("recovery metadata content address mismatch"));
    }
    let metadata = verify(&raw, &signature, version)?;
    if metadata.version().to_string() != version
        || metadata.release_sequence() != pointer.release_sequence
    {
        return Err(invalid(
            "recovery metadata pointer disagrees with signed identity",
        ));
    }
    Ok((fs::canonicalize(payload)?, metadata))
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

    #[test]
    fn recovery_diagnostic_preserves_stage_kind_and_underlying_error() {
        let error = recovery_error(
            "私有下载目录检查/创建",
            io::Error::new(io::ErrorKind::PermissionDenied, "native access denied"),
        );
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(
            error.to_string(),
            "私有下载目录检查/创建：native access denied"
        );
    }

    fn verify_fixture(
        raw: &[u8],
        signature: &[u8],
        version: &str,
    ) -> io::Result<VerifiedWindowsInstallerManifest> {
        super::super::assets::verify_fixture_manifest(
            raw,
            signature,
            version,
            OffsetDateTime::parse("2026-10-01T00:00:00Z", &Rfc3339).unwrap(),
        )
    }

    #[test]
    fn recovery_paths_are_source_scoped_and_preserve_production_layout() {
        let root = Path::new("fixture");
        let production = crate::AllowedUpdateRoot::parse(
            "https://instplot-release.oss-cn-beijing.aliyuncs.com/instplot-studio",
        )
        .unwrap();
        let qa = crate::AllowedUpdateRoot::parse("https://instplot-release.oss-cn-beijing.aliyuncs.com/instplot-studio/windows-gui-qa/123-1").unwrap();
        let other_qa = crate::AllowedUpdateRoot::parse("https://instplot-release.oss-cn-beijing.aliyuncs.com/instplot-studio/windows-gui-qa/456-1").unwrap();
        let old_path = root.join(format!("{:x}", Sha256::digest(b"0.1.2-rc.2")));
        assert_eq!(
            version_directory_for_source(root, &production, "0.1.2-rc.2"),
            old_path
        );
        let qa_path = version_directory_for_source(root, &qa, "0.1.2-rc.2");
        assert_ne!(qa_path, old_path);
        assert_ne!(
            qa_path,
            version_directory_for_source(root, &other_qa, "0.1.2-rc.2")
        );
    }

    #[test]
    fn retained_metadata_is_exact_private_immutable_and_monotonic() {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).unwrap();
        let root = std::env::temp_dir().join(format!(
            "studio-recovery-metadata-{:032x}",
            u128::from_le_bytes(random)
        ));
        let original = super::super::assets::metadata_fixture(2);
        retain_at(&root, &original).unwrap();
        let (old_path, loaded) = load_at(&root, "0.1.2-rc.2", verify_fixture).unwrap();
        assert_eq!(loaded.raw(), original.raw());
        assert!(
            !old_path.join(loaded.package().file_name.clone()).exists(),
            "metadata does not mean the installer is downloaded"
        );
        super::super::validate_private_file(&old_path.join("manifest.json")).unwrap();
        retain_at(&root, &original).unwrap();
        assert!(load_at(&root, "0.1.2-rc.1", verify_fixture).is_err());
        // A competing writer cannot bypass the common private cache lock.
        let directory = version_directory(&root, "0.1.2-rc.2");
        let lock = TransactionStore::lock(&directory).unwrap();
        assert!(retain_at(&root, &original).is_err());
        drop(lock);
        // A different authenticated record at the same sequence is refused.
        use ed25519_dalek::{Signer, SigningKey};
        let mut changed: serde_json::Value = serde_json::from_slice(original.raw()).unwrap();
        changed["expires_at"] = "2026-12-28T00:00:00Z".into();
        let changed = serde_json::to_vec(&changed).unwrap();
        let signature = SigningKey::from_bytes(&[7; 32]).sign(&changed).to_bytes();
        let conflict = verify_fixture(&changed, &signature, "0.1.2-rc.2").unwrap();
        assert!(retain_at(&root, &conflict).is_err());
        let fresh = super::super::assets::metadata_fixture(3);
        // Pointer replacement failure must not publish a half-ready new record.
        use std::os::windows::fs::OpenOptionsExt;
        let blocked_pointer = fs::OpenOptions::new()
            .read(true)
            .share_mode(windows::Win32::Storage::FileSystem::FILE_SHARE_READ.0)
            .open(directory.join("current.json"))
            .unwrap();
        assert!(retain_at(&root, &fresh).is_err());
        assert_eq!(
            load_at(&root, "0.1.2-rc.2", verify_fixture)
                .unwrap()
                .1
                .release_sequence(),
            2
        );
        drop(blocked_pointer);
        retain_at(&root, &fresh).unwrap();
        assert_eq!(
            load_at(&root, "0.1.2-rc.2", verify_fixture)
                .unwrap()
                .1
                .release_sequence(),
            3
        );
        assert!(retain_at(&root, &original).is_err());
        assert_eq!(
            read_private(&old_path.join("manifest.json"), 256 * 1024).unwrap(),
            original.raw()
        );
        let (path, _) = load_at(&root, "0.1.2-rc.2", verify_fixture).unwrap();
        super::super::write_private_atomic(&path.join("manifest.json"), b"changed").unwrap();
        assert!(load_at(&root, "0.1.2-rc.2", verify_fixture).is_err());
        assert!(retain_at(&root, &fresh).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_expired_and_foreign_metadata_never_become_recovery_assets() {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).unwrap();
        let root = std::env::temp_dir().join(format!(
            "studio-recovery-expiry-{:032x}",
            u128::from_le_bytes(random)
        ));
        assert!(load_at(&root, "0.1.2-rc.2", verify_fixture).is_err());
        let proof = super::super::assets::metadata_fixture(2);
        retain_at(&root, &proof).unwrap();
        assert!(
            load_at(&root, "0.1.2-rc.2", |raw, sig, version| {
                super::super::assets::verify_fixture_manifest(
                    raw,
                    sig,
                    version,
                    OffsetDateTime::parse("2027-01-01T00:00:00Z", &Rfc3339).unwrap(),
                )
            })
            .is_err()
        );
        let directory = version_directory(&root, "0.1.2-rc.2");
        let mut pointer: serde_json::Value =
            serde_json::from_slice(&read_private(&directory.join("current.json"), 4096).unwrap())
                .unwrap();
        pointer["manifest_sha256"] = "../foreign".into();
        super::super::write_private_atomic(
            &directory.join("current.json"),
            &serde_json::to_vec(&pointer).unwrap(),
        )
        .unwrap();
        assert!(load_at(&root, "0.1.2-rc.2", verify_fixture).is_err());
        assert!(retain_at(&root, &proof).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn foreign_acl_is_rejected_without_repair_or_overwrite() {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).unwrap();
        let root = std::env::temp_dir().join(format!(
            "studio-recovery-foreign-{:032x}",
            u128::from_le_bytes(random)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("sentinel.txt"), b"untouched").unwrap();
        assert!(retain_at(&root, &super::super::assets::metadata_fixture(2)).is_err());
        assert_eq!(fs::read(root.join("sentinel.txt")).unwrap(), b"untouched");
        assert!(super::super::validate_private_directory(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}

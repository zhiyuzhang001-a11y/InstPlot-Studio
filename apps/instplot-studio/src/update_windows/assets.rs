//! Verified candidate and old recovery installers. No network or process execution.
//! Expired metadata is rejected before application, including recovery metadata.
//! A later helper may use an already prepared recovery transaction offline, but
//! that contract must not be confused with fresh acceptance of expired updates.
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use semver::Version;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use super::{WindowsInstallation, invalid, reject_redirected_path};
use crate::{
    AllowedUpdateRoot, PRODUCTION_PUBLIC_ROOT, PRODUCTION_TRUSTED_KEYS, TrustedUpdateKey,
    UpdateChannel, UpdatePackage, verify_signed_manifest_at,
};

const MAX_INSTALLER_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// A signed Windows installer description, not a downloaded installer.
/// Keeping metadata does not make automatic apply or offline recovery ready.
#[derive(Clone, Debug)]
pub struct VerifiedWindowsInstallerManifest {
    raw: Vec<u8>,
    signature: Vec<u8>,
    version: Version,
    package: UpdatePackage,
    sequence: u64,
}

impl VerifiedWindowsInstallerManifest {
    pub fn from_raw(raw: &[u8], signature: &[u8], expected_version: &str) -> io::Result<Self> {
        Self::verify_at(
            raw,
            signature,
            expected_version,
            &PRODUCTION_TRUSTED_KEYS,
            &AllowedUpdateRoot::parse(PRODUCTION_PUBLIC_ROOT).map_err(invalid)?,
            OffsetDateTime::now_utc(),
        )
    }

    fn verify_at(
        raw: &[u8],
        signature: &[u8],
        expected_version: &str,
        keys: &[TrustedUpdateKey],
        root: &AllowedUpdateRoot,
        now: OffsetDateTime,
    ) -> io::Result<Self> {
        if raw.len() > 256 * 1024 || signature.len() != 64 {
            return Err(invalid("invalid Windows installer metadata size"));
        }
        let version = Version::parse(expected_version).map_err(invalid)?;
        let manifest = verify_signed_manifest_at(
            raw,
            signature,
            keys,
            root,
            UpdateChannel::for_version(&version),
            now,
        )
        .map_err(invalid)?;
        if manifest.version != expected_version {
            return Err(invalid(
                "installer manifest does not match the exact required version",
            ));
        }
        let platform = manifest
            .platforms
            .get("windows-x86_64")
            .ok_or_else(|| invalid("no Windows x64 installer in verified manifest"))?;
        let package = platform
            .packages
            .iter()
            .find(|p| p.id == platform.preferred)
            .ok_or_else(|| invalid("preferred Windows installer is missing"))?
            .clone();
        if package.id != "inno-setup"
            || package.package_type != "inno-setup"
            || package.file_name
                != format!("InstPlot-Studio-{expected_version}-windows-x86_64-setup.exe")
            || package.size_bytes > MAX_INSTALLER_BYTES
        {
            return Err(invalid("wrong Windows installer type/name/size"));
        }
        Ok(Self {
            raw: raw.to_vec(),
            signature: signature.to_vec(),
            version,
            package,
            sequence: manifest.release_sequence,
        })
    }

    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
    pub fn signature(&self) -> &[u8] {
        &self.signature
    }
    pub fn version(&self) -> &Version {
        &self.version
    }
    pub fn package(&self) -> &UpdatePackage {
        &self.package
    }
    pub fn release_sequence(&self) -> u64 {
        self.sequence
    }
    pub fn sha256(&self) -> String {
        format!("{:x}", Sha256::digest(&self.raw))
    }
}

#[derive(Clone, Debug)]
pub struct VerifiedWindowsInstaller {
    path: PathBuf,
    version: Version,
    package: UpdatePackage,
    manifest_sha256: String,
    #[cfg(windows)]
    enforce_private_acl: bool,
}

impl VerifiedWindowsInstaller {
    /// Production trust is compiled in; callers cannot inject their own keys.
    pub fn from_cache(directory: &Path, expected_version: &str) -> io::Result<Self> {
        #[cfg(windows)]
        super::validate_private_directory(directory)?;
        let installer = Self::verify_at(
            directory,
            expected_version,
            &PRODUCTION_TRUSTED_KEYS,
            &AllowedUpdateRoot::parse(PRODUCTION_PUBLIC_ROOT).map_err(invalid)?,
            OffsetDateTime::now_utc(),
        )?;
        #[cfg(windows)]
        let installer = {
            for evidence in [
                directory.join("manifest.json"),
                directory.join("manifest.json.sig"),
                installer.path.clone(),
            ] {
                super::validate_private_file(&evidence)?;
            }
            Self {
                enforce_private_acl: true,
                ..installer
            }
        };
        Ok(installer)
    }

    fn verify_at(
        directory: &Path,
        expected_version: &str,
        keys: &[TrustedUpdateKey],
        root: &AllowedUpdateRoot,
        now: OffsetDateTime,
    ) -> io::Result<Self> {
        reject_redirected_path(directory)?;
        if !directory.is_dir() {
            return Err(invalid("installer cache is not a directory"));
        }
        let directory = fs::canonicalize(directory)?;
        let raw = read_bounded(&directory.join("manifest.json"), 256 * 1024)?;
        let signature = read_bounded(&directory.join("manifest.json.sig"), 64)?;
        let metadata = VerifiedWindowsInstallerManifest::verify_at(
            &raw,
            &signature,
            expected_version,
            keys,
            root,
            now,
        )?;
        let installer = Self {
            path: directory.join(&metadata.package.file_name),
            version: metadata.version,
            package: metadata.package,
            manifest_sha256: format!("{:x}", Sha256::digest(&raw)),
            #[cfg(windows)]
            enforce_private_acl: false,
        };
        installer.revalidate()?;
        Ok(installer)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn version(&self) -> &Version {
        &self.version
    }
    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }
    pub fn sha256(&self) -> &str {
        &self.package.sha256
    }
    pub fn size_bytes(&self) -> u64 {
        self.package.size_bytes
    }

    /// Repeat immediately before use, within the platform's private-cache lock.
    /// Signature/expiry verification above is a prepare-time requirement; this
    /// method only proves the already accepted file has not changed.
    pub fn revalidate(&self) -> io::Result<()> {
        self.validate_path()?;
        self.verify_file(&mut File::open(&self.path)?)
    }

    fn validate_path(&self) -> io::Result<()> {
        #[cfg(windows)]
        if self.enforce_private_acl {
            super::validate_private_directory(
                self.path
                    .parent()
                    .ok_or_else(|| invalid("installer has no cache parent"))?,
            )?;
            super::validate_private_file(&self.path)?;
        }
        reject_redirected_path(&self.path)?;
        Ok(())
    }

    fn verify_file(&self, file: &mut File) -> io::Result<()> {
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.len() != self.package.size_bytes
            || metadata.len() == 0
            || metadata.len() > MAX_INSTALLER_BYTES
        {
            return Err(invalid("installer size does not match verified manifest"));
        }
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        let mut total = 0_u64;
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            total += count as u64;
            if total > self.package.size_bytes {
                return Err(invalid("installer grew during verification"));
            }
            hash.update(&buffer[..count]);
        }
        if total != self.package.size_bytes
            || format!("{:x}", hash.finalize()) != self.package.sha256
        {
            return Err(invalid("installer hash does not match verified manifest"));
        }
        Ok(())
    }

    /// Hold a read-only Windows sharing lease through installation/recovery.
    /// Revalidation alone cannot prevent a later write or path replacement.
    #[cfg(windows)]
    pub fn pin(&self) -> io::Result<PinnedWindowsInstaller> {
        use std::os::windows::fs::OpenOptionsExt;
        use windows::Win32::Storage::FileSystem::FILE_SHARE_READ;

        self.validate_path()?;
        let mut file = fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ.0)
            .open(&self.path)?;
        // Check again after obtaining the lease; writes/deletes are now denied
        // for this file, including writers that were already open.
        self.validate_path()?;
        self.verify_file(&mut file)?;
        Ok(PinnedWindowsInstaller {
            installer: self.clone(),
            _lease: file,
        })
    }
}

/// Not Clone/Deserialize: the descriptor is usable only while its lease lives.
/// This prevents installer bytes being changed between verification and use;
/// it is not an installation lock or protection against the current user.
#[cfg(windows)]
#[derive(Debug)]
pub struct PinnedWindowsInstaller {
    installer: VerifiedWindowsInstaller,
    _lease: File,
}

#[cfg(windows)]
impl PinnedWindowsInstaller {
    pub fn installer(&self) -> &VerifiedWindowsInstaller {
        &self.installer
    }
}

#[derive(Clone, Debug)]
pub struct WindowsInstallerPair {
    recovery: VerifiedWindowsInstaller,
    candidate: VerifiedWindowsInstaller,
}
impl WindowsInstallerPair {
    /// Both recovery and candidate assets must already be present and trusted
    /// BEFORE the GUI exits. No lazy recovery download or downgrade of latest.
    pub fn from_caches(
        installation: &WindowsInstallation,
        recovery_cache: &Path,
        candidate_cache: &Path,
        candidate_version: &str,
    ) -> io::Result<Self> {
        installation.validate_candidate_version(candidate_version)?;
        let recovery = VerifiedWindowsInstaller::from_cache(
            recovery_cache,
            &installation.version().to_string(),
        )?;
        let candidate = VerifiedWindowsInstaller::from_cache(candidate_cache, candidate_version)?;
        Self::bind_versions(installation.version(), recovery, candidate)
    }

    fn bind_versions(
        installed_version: &Version,
        recovery: VerifiedWindowsInstaller,
        candidate: VerifiedWindowsInstaller,
    ) -> io::Result<Self> {
        if recovery.version() != installed_version
            || candidate.version() <= installed_version
            || candidate.version().pre.is_empty() != installed_version.pre.is_empty()
        {
            return Err(invalid(
                "installer roles do not match the installed version/channel",
            ));
        }
        if recovery.path == candidate.path || recovery.sha256() == candidate.sha256() {
            return Err(invalid(
                "recovery and candidate installers must be distinct",
            ));
        }
        Ok(Self {
            recovery,
            candidate,
        })
    }
    pub fn recovery(&self) -> &VerifiedWindowsInstaller {
        &self.recovery
    }
    pub fn candidate(&self) -> &VerifiedWindowsInstaller {
        &self.candidate
    }
    pub fn revalidate(&self) -> io::Result<()> {
        self.recovery.revalidate()?;
        self.candidate.revalidate()
    }

    /// Acquire both leases before asking the application to exit. Failure to
    /// pin either installer releases the other and leaves the GUI untouched.
    #[cfg(windows)]
    pub fn pin(&self) -> io::Result<PinnedWindowsInstallerPair> {
        Ok(PinnedWindowsInstallerPair {
            recovery: self.recovery.pin()?,
            candidate: self.candidate.pin()?,
        })
    }
}

#[cfg(windows)]
#[derive(Debug)]
pub struct PinnedWindowsInstallerPair {
    pub recovery: PinnedWindowsInstaller,
    pub candidate: PinnedWindowsInstaller,
}

fn read_bounded(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    reject_redirected_path(path)?;
    let file = File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > limit {
        return Err(invalid("invalid signed installer evidence file"));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid("oversized installer evidence"));
    }
    Ok(bytes)
}

#[cfg(all(windows, test))]
pub(super) fn metadata_fixture(sequence: u64) -> VerifiedWindowsInstallerManifest {
    tests::metadata_fixture(sequence)
}

#[cfg(all(windows, test))]
pub(super) fn verify_fixture_manifest(
    raw: &[u8],
    signature: &[u8],
    version: &str,
    now: OffsetDateTime,
) -> io::Result<VerifiedWindowsInstallerManifest> {
    use ed25519_dalek::SigningKey;
    VerifiedWindowsInstallerManifest::verify_at(
        raw,
        signature,
        version,
        &[TrustedUpdateKey {
            id: "fixture",
            bytes: SigningKey::from_bytes(&[7; 32]).verifying_key().to_bytes(),
        }],
        &AllowedUpdateRoot::parse(tests::ROOT).unwrap(),
        now,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use time::format_description::well_known::Rfc3339;

    pub(super) const ROOT: &str = "https://downloads.example.test/instplot-studio";
    const VERSION: &str = "0.1.2-rc.2";
    struct Fixture {
        directory: PathBuf,
        signing: SigningKey,
        raw: serde_json::Value,
    }
    impl Fixture {
        fn new() -> Self {
            let mut random = [0; 8];
            getrandom::fill(&mut random).unwrap();
            let directory = std::env::temp_dir().join(format!(
                "studio-install-assets-{:x}",
                u64::from_le_bytes(random)
            ));
            fs::create_dir(&directory).unwrap();
            let directory = fs::canonicalize(directory).unwrap();
            let name = format!("InstPlot-Studio-{VERSION}-windows-x86_64-setup.exe");
            let bytes = b"not a real installer; signed fixture only";
            fs::write(directory.join(&name), bytes).unwrap();
            let raw = serde_json::json!({
                "schema":1,"product":"instplot-studio","version":VERSION,"channel":"prerelease",
                "key_id":"fixture","release_sequence":2,"published_at":"2026-09-29T00:00:00Z",
                "expires_at":"2026-12-29T00:00:00Z",
                "notes_url":format!("https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/releases/tag/v{VERSION}"),
                "signature_url":format!("{ROOT}/releases/{VERSION}/metadata/2/manifest.json.sig"),
                "platforms":{"windows-x86_64":{"preferred":"inno-setup","packages":[{
                    "id":"inno-setup","package_type":"inno-setup","file_name":name,
                    "minimum_system":"Windows 10","size_bytes":bytes.len(),
                    "sha256":format!("{:x}",Sha256::digest(bytes)),"url":format!("{ROOT}/releases/{VERSION}/{name}")
                }]}}
            });
            let fixture = Self {
                directory,
                signing: SigningKey::from_bytes(&[7; 32]),
                raw,
            };
            fixture.save();
            fixture
        }
        fn save(&self) {
            let raw = serde_json::to_vec(&self.raw).unwrap();
            fs::write(
                self.directory.join("manifest.json.sig"),
                self.signing.sign(&raw).to_bytes(),
            )
            .unwrap();
            fs::write(self.directory.join("manifest.json"), raw).unwrap();
        }
        fn verify(&self) -> io::Result<VerifiedWindowsInstaller> {
            VerifiedWindowsInstaller::verify_at(
                &self.directory,
                VERSION,
                &[TrustedUpdateKey {
                    id: "fixture",
                    bytes: self.signing.verifying_key().to_bytes(),
                }],
                &AllowedUpdateRoot::parse(ROOT).unwrap(),
                OffsetDateTime::parse("2026-10-01T00:00:00Z", &Rfc3339).unwrap(),
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.directory).unwrap();
        }
    }

    #[cfg(windows)]
    pub(super) fn metadata_fixture(sequence: u64) -> VerifiedWindowsInstallerManifest {
        let mut fixture = Fixture::new();
        fixture.raw["release_sequence"] = sequence.into();
        fixture.raw["signature_url"] =
            format!("{ROOT}/releases/{VERSION}/metadata/{sequence}/manifest.json.sig").into();
        fixture.save();
        super::verify_fixture_manifest(
            &fs::read(fixture.directory.join("manifest.json")).unwrap(),
            &fs::read(fixture.directory.join("manifest.json.sig")).unwrap(),
            VERSION,
            OffsetDateTime::parse("2026-10-01T00:00:00Z", &Rfc3339).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn signed_metadata_without_package_is_not_a_verified_installer() {
        let fixture = Fixture::new();
        let raw = fs::read(fixture.directory.join("manifest.json")).unwrap();
        let signature = fs::read(fixture.directory.join("manifest.json.sig")).unwrap();
        let proof = VerifiedWindowsInstallerManifest::verify_at(
            &raw,
            &signature,
            VERSION,
            &[TrustedUpdateKey {
                id: "fixture",
                bytes: fixture.signing.verifying_key().to_bytes(),
            }],
            &AllowedUpdateRoot::parse(ROOT).unwrap(),
            OffsetDateTime::parse("2026-10-01T00:00:00Z", &Rfc3339).unwrap(),
        )
        .unwrap();
        assert_eq!(proof.version().to_string(), VERSION);
        assert_eq!(proof.release_sequence(), 2);
        assert_eq!(proof.raw(), raw);
        assert_eq!(proof.signature(), signature);
        fs::remove_file(fixture.directory.join(&proof.package().file_name)).unwrap();
        assert!(fixture.verify().is_err());
        assert_eq!(proof.package().package_type, "inno-setup");
        let mut bad_signature = signature.clone();
        bad_signature[0] ^= 1;
        assert!(
            VerifiedWindowsInstallerManifest::verify_at(
                &raw,
                &bad_signature,
                VERSION,
                &[TrustedUpdateKey {
                    id: "fixture",
                    bytes: fixture.signing.verifying_key().to_bytes()
                }],
                &AllowedUpdateRoot::parse(ROOT).unwrap(),
                OffsetDateTime::parse("2026-10-01T00:00:00Z", &Rfc3339).unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn trusted_current_version_installer_is_verified_and_tampering_is_rejected() {
        let fixture = Fixture::new();
        let asset = fixture.verify().unwrap();
        assert_eq!(asset.version().to_string(), VERSION);
        assert_eq!(asset.manifest_sha256().len(), 64);
        asset.revalidate().unwrap();
        let mut bytes = fs::read(asset.path()).unwrap();
        bytes[0] ^= 1;
        fs::write(asset.path(), bytes).unwrap();
        assert!(asset.revalidate().is_err());
        assert!(fixture.verify().is_err());
    }

    #[test]
    fn missing_package_signature_and_expired_metadata_fail_closed() {
        let mut fixture = Fixture::new();
        let asset = fixture.verify().unwrap();
        fs::remove_file(asset.path()).unwrap();
        assert!(fixture.verify().is_err());
        fixture.raw["expires_at"] = "2026-09-30T00:00:00Z".into();
        fixture.save();
        assert!(
            fixture
                .verify()
                .unwrap_err()
                .to_string()
                .contains("expired")
        );
        fs::write(fixture.directory.join("manifest.json.sig"), [0; 64]).unwrap();
        assert!(fixture.verify().is_err());
    }

    #[test]
    fn wrong_product_platform_version_name_and_type_are_rejected() {
        let cases = [
            ("/product", "instplot-lite"),
            ("/version", "0.1.2-rc.1"),
            ("/channel", "stable"),
            (
                "/platforms/windows-x86_64/packages/0/url",
                "https://foreign.example/setup.exe",
            ),
            (
                "/platforms/windows-x86_64/packages/0/file_name",
                "../setup.exe",
            ),
            ("/platforms/windows-x86_64/packages/0/package_type", "zip"),
        ];
        for (pointer, value) in cases {
            let mut fixture = Fixture::new();
            *fixture.raw.pointer_mut(pointer).unwrap() = value.into();
            fixture.save();
            assert!(fixture.verify().is_err(), "{pointer}");
        }
        let mut fixture = Fixture::new();
        let platform = fixture.raw["platforms"].as_object_mut().unwrap();
        let value = platform.remove("windows-x86_64").unwrap();
        platform.insert("macos-aarch64".into(), value);
        fixture.save();
        assert!(fixture.verify().is_err());
    }

    #[test]
    fn oversized_metadata_and_changed_package_size_are_rejected() {
        let fixture = Fixture::new();
        let asset = fixture.verify().unwrap();
        fs::write(asset.path(), b"too short").unwrap();
        assert!(asset.revalidate().is_err());
        fs::write(
            fixture.directory.join("manifest.json"),
            vec![b' '; 256 * 1024 + 1],
        )
        .unwrap();
        assert!(fixture.verify().is_err());
    }

    #[cfg(windows)]
    #[test]
    fn installer_test_child_waits_for_normal_parent_pipe_close() {
        if std::env::var_os("STUDIO_INSTALLER_TEST_WAIT").is_some() {
            let mut input = Vec::new();
            std::io::stdin().read_to_end(&mut input).unwrap();
        }
    }

    #[cfg(windows)]
    #[test]
    fn native_runner_requires_normal_exit_and_retains_exclusive_access() {
        use super::super::{RunningWindowsInstaller, TrackedWindowsProcess, WindowsInstallAccess};
        use crate::update_transaction::{
            TransactionStore, UpdateIdentity, UpdateStage, UpdateTransaction,
        };
        use std::process::{Command, Stdio};
        use std::time::Duration;

        let mut fixture = Fixture::new();
        let executable = std::env::current_exe().unwrap();
        let name = fixture.raw["platforms"]["windows-x86_64"]["packages"][0]["file_name"]
            .as_str()
            .unwrap()
            .to_owned();
        // Signed isolated test harness, NOT an Inno/product installer. It
        // proves native process/lock wiring only; real Inno is tested separately.
        let path = fixture.directory.join(name);
        fs::copy(&executable, &path).unwrap();
        let bytes = fs::read(&path).unwrap();
        let package = &mut fixture.raw["platforms"]["windows-x86_64"]["packages"][0];
        package["size_bytes"] = bytes.len().into();
        package["sha256"] = format!("{:x}", Sha256::digest(&bytes)).into();
        fixture.save();
        let asset = fixture.verify().unwrap();
        let locks = fixture.directory.join("locks");
        super::super::create_private_directory(&locks).unwrap();
        let access = WindowsInstallAccess::acquire(&fixture.directory, &locks, true).unwrap();
        let installation = WindowsInstallation {
            directory: fixture.directory.clone(),
            executable: executable.clone(),
            version: Version::parse(VERSION).unwrap(),
            desktop_shortcut: false,
        };
        let store = TransactionStore::lock(&locks).unwrap();
        let mut transaction = UpdateTransaction::new(UpdateIdentity {
            product: "instplot-studio".into(),
            platform: "windows-x86_64".into(),
            installed_path: installation.directory().to_owned(),
            previous_version: VERSION.into(),
            candidate_version: "0.1.2-rc.4".into(),
            candidate_sha256: "c".repeat(64),
            candidate_size: 1,
        })
        .unwrap();
        for stage in [
            UpdateStage::WaitingForExit,
            UpdateStage::Applying,
            UpdateStage::RecoveryRequired,
            UpdateStage::Restoring,
        ] {
            transaction.transition(stage).unwrap();
        }
        store.write(&transaction).unwrap();
        let mut previous = Command::new(&executable)
            .args(["--exact", "update_windows::assets::tests::installer_test_child_waits_for_normal_parent_pipe_close"])
            .env("STUDIO_INSTALLER_TEST_WAIT", "1")
            .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null())
            .spawn().unwrap();
        let tracked = TrackedWindowsProcess::bind_child(&previous, &executable).unwrap();
        let log = locks.join("installer.log");
        assert!(
            RunningWindowsInstaller::start(
                &installation,
                asset.pin().unwrap(),
                &access,
                &tracked,
                &log,
                &store,
                &transaction
            )
            .is_err()
        );
        assert!(!log.exists());
        assert!(!locks.join("restore-installer.json").exists());
        drop(previous.stdin.take());
        assert!(tracked.wait_for_exit(Duration::from_secs(5)).unwrap());
        assert!(previous.wait().unwrap().success());
        assert_eq!(tracked.exit_code_if_exited().unwrap(), Some(0));
        let mut foreign_transaction =
            UpdateTransaction::new(transaction.identity().clone()).unwrap();
        for stage in [
            UpdateStage::WaitingForExit,
            UpdateStage::Applying,
            UpdateStage::RecoveryRequired,
            UpdateStage::Restoring,
        ] {
            foreign_transaction.transition(stage).unwrap();
        }
        assert!(
            RunningWindowsInstaller::start(
                &installation,
                asset.pin().unwrap(),
                &access,
                &tracked,
                &locks.join("foreign-transaction.log"),
                &store,
                &foreign_transaction
            )
            .is_err()
        );
        assert!(!locks.join("foreign-transaction.log").exists());
        assert!(!locks.join("restore-installer.json").exists());
        let mut running = RunningWindowsInstaller::start(
            &installation,
            asset.pin().unwrap(),
            &access,
            &tracked,
            &log,
            &store,
            &transaction,
        )
        .unwrap();
        assert_ne!(running.process_id(), 0);
        use super::super::{InstallerAttemptStatus, installer_attempt_status};
        assert!(
            matches!(installer_attempt_status(&store, &transaction, &installation, &asset).unwrap(),
            InstallerAttemptStatus::Running { process_id, created } if process_id == running.process_id() && created > 0)
        );
        assert!(WindowsInstallAccess::acquire(&fixture.directory, &locks, false).is_err());
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        // Windows sharing denies atomic checkpoint replacement. Even after
        // Child exits, a failed durable write must not return successful exit
        // or release the installer lease/access lock.
        use std::os::windows::fs::OpenOptionsExt;
        let blocked_checkpoint = fs::OpenOptions::new()
            .read(true)
            .share_mode(windows::Win32::Storage::FileSystem::FILE_SHARE_READ.0)
            .open(locks.join("restore-installer.json"))
            .unwrap();
        loop {
            match running.try_wait() {
                Err(_) => break,
                Ok(None) => {}
                Ok(Some(_)) => panic!("exit was returned without durable checkpoint"),
            }
            assert!(
                std::time::Instant::now() < deadline,
                "checkpoint fault was not reached"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            fs::OpenOptions::new()
                .write(true)
                .open(asset.path())
                .is_err()
        );
        assert!(WindowsInstallAccess::acquire(&fixture.directory, &locks, false).is_err());
        drop(blocked_checkpoint);
        loop {
            if running.try_wait().unwrap().is_some() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "native runner test child did not exit"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(running);
        assert!(matches!(
            installer_attempt_status(&store, &transaction, &installation, &asset).unwrap(),
            InstallerAttemptStatus::Exited { .. }
        ));
        let recorded = fs::read(locks.join("restore-installer.json")).unwrap();
        assert!(
            RunningWindowsInstaller::start(
                &installation,
                asset.pin().unwrap(),
                &access,
                &tracked,
                &locks.join("duplicate.log"),
                &store,
                &transaction
            )
            .is_err()
        );
        assert!(!locks.join("duplicate.log").exists());
        assert_eq!(
            fs::read(locks.join("restore-installer.json")).unwrap(),
            recorded
        );
        // Read-only restart inspection binds a real still-live fixture process;
        // wrong creation time must not turn into an inferred exit.
        let mut inspection_child = Command::new(asset.path())
            .args(["--exact", "update_windows::assets::tests::installer_test_child_waits_for_normal_parent_pipe_close"])
            .env("STUDIO_INSTALLER_TEST_WAIT", "1").stdin(Stdio::piped())
            .stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
        let inspection_process =
            TrackedWindowsProcess::bind_child(&inspection_child, asset.path()).unwrap();
        let created = super::super::process::child_process_created(&inspection_child).unwrap();
        let mut inspecting: serde_json::Value = serde_json::from_slice(&recorded).unwrap();
        inspecting["phase"] = "running".into();
        inspecting["process"] = serde_json::json!([inspection_child.id(), created]);
        inspecting["exit_code"] = serde_json::Value::Null;
        let journal_path = locks.join("restore-installer.json");
        super::super::write_private_atomic(
            &journal_path,
            &serde_json::to_vec(&inspecting).unwrap(),
        )
        .unwrap();
        let lease = asset.pin().unwrap();
        assert_eq!(
            super::super::refresh_installer_attempt(
                &store,
                &transaction,
                &installation,
                &lease,
                &access
            )
            .unwrap(),
            InstallerAttemptStatus::Running {
                process_id: inspection_child.id(),
                created
            }
        );
        inspecting["process"] = serde_json::json!([inspection_child.id(), created + 1]);
        super::super::write_private_atomic(
            &journal_path,
            &serde_json::to_vec(&inspecting).unwrap(),
        )
        .unwrap();
        assert!(
            super::super::refresh_installer_attempt(
                &store,
                &transaction,
                &installation,
                &lease,
                &access
            )
            .is_err()
        );
        drop(inspection_child.stdin.take());
        assert!(
            inspection_process
                .wait_for_exit(Duration::from_secs(5))
                .unwrap()
        );
        assert!(inspection_child.wait().unwrap().success());
        assert_eq!(inspection_process.exit_code_if_exited().unwrap(), Some(0));
        drop(lease);
        // A simulated crash-before-checkpoint must never become retry/exit
        // permission. These edits affect this isolated fixture's journal only.
        let mut interrupted: serde_json::Value = serde_json::from_slice(&recorded).unwrap();
        interrupted["phase"] = "intent".into();
        interrupted["process"] = serde_json::Value::Null;
        interrupted["exit_code"] = serde_json::Value::Null;
        super::super::write_private_atomic(
            &journal_path,
            &serde_json::to_vec(&interrupted).unwrap(),
        )
        .unwrap();
        assert_eq!(
            installer_attempt_status(&store, &transaction, &installation, &asset).unwrap(),
            InstallerAttemptStatus::Unresolved
        );
        assert!(
            RunningWindowsInstaller::start(
                &installation,
                asset.pin().unwrap(),
                &access,
                &tracked,
                &locks.join("uncertain-replay.log"),
                &store,
                &transaction
            )
            .is_err()
        );
        assert!(!locks.join("uncertain-replay.log").exists());
        interrupted["transaction_id"] = "0".repeat(32).into();
        super::super::write_private_atomic(
            &journal_path,
            &serde_json::to_vec(&interrupted).unwrap(),
        )
        .unwrap();
        assert!(installer_attempt_status(&store, &transaction, &installation, &asset).is_err());
        interrupted["transaction_id"] = transaction.id().into();
        interrupted["unexpected"] = true.into();
        super::super::write_private_atomic(
            &journal_path,
            &serde_json::to_vec(&interrupted).unwrap(),
        )
        .unwrap();
        assert!(installer_attempt_status(&store, &transaction, &installation, &asset).is_err());
        // The helper still owns the installation lock after installer exit,
        // so it can verify health or restore without opening a race.
        assert!(WindowsInstallAccess::acquire(&fixture.directory, &locks, false).is_err());
        drop(access);
        WindowsInstallAccess::acquire(&fixture.directory, &locks, false).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn installer_lease_denies_writes_and_deletes_until_released() {
        let fixture = Fixture::new();
        let asset = fixture.verify().unwrap();
        let lease = asset.pin().unwrap();
        assert_eq!(lease.installer().sha256(), asset.sha256());
        assert!(File::open(asset.path()).is_ok());
        let write_error = fs::OpenOptions::new()
            .write(true)
            .open(asset.path())
            .unwrap_err();
        assert_eq!(write_error.raw_os_error(), Some(32));
        assert_eq!(
            fs::remove_file(asset.path()).unwrap_err().raw_os_error(),
            Some(32)
        );
        assert!(fs::rename(asset.path(), fixture.directory.join("moved.exe")).is_err());
        assert!(asset.path().is_file());
        drop(lease);
        let mut bytes = fs::read(asset.path()).unwrap();
        bytes[0] ^= 1;
        fs::write(asset.path(), bytes).unwrap();
        assert!(asset.pin().is_err());
    }

    #[cfg(windows)]
    #[test]
    fn installer_lease_rejects_an_existing_writer() {
        let fixture = Fixture::new();
        let asset = fixture.verify().unwrap();
        let writer = fs::OpenOptions::new()
            .write(true)
            .open(asset.path())
            .unwrap();
        assert_eq!(asset.pin().unwrap_err().raw_os_error(), Some(32));
        drop(writer);
        asset.pin().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn production_private_acl_requirement_is_retained_by_clones() {
        let fixture = Fixture::new();
        let mut asset = fixture.verify().unwrap();
        // This fixture deliberately uses a default temp directory. Once the
        // production ACL policy is enabled, a clone cannot drop that policy.
        asset.enforce_private_acl = true;
        assert!(asset.revalidate().is_err());
        assert!(asset.clone().revalidate().is_err());
    }

    #[test]
    fn pair_requires_exact_recovery_newer_candidate_and_distinct_assets() {
        let recovery_fixture = Fixture::new();
        let recovery = recovery_fixture.verify().unwrap();
        let mut candidate_fixture = Fixture::new();
        let next = "0.1.2-rc.3";
        let name = format!("InstPlot-Studio-{next}-windows-x86_64-setup.exe");
        let bytes = b"different signed candidate fixture";
        fs::write(candidate_fixture.directory.join(&name), bytes).unwrap();
        candidate_fixture.raw["version"] = next.into();
        candidate_fixture.raw["notes_url"] =
            format!("https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/releases/tag/v{next}")
                .into();
        candidate_fixture.raw["signature_url"] =
            format!("{ROOT}/releases/{next}/metadata/2/manifest.json.sig").into();
        let package = &mut candidate_fixture.raw["platforms"]["windows-x86_64"]["packages"][0];
        package["file_name"] = name.clone().into();
        package["url"] = format!("{ROOT}/releases/{next}/{name}").into();
        package["size_bytes"] = bytes.len().into();
        package["sha256"] = format!("{:x}", Sha256::digest(bytes)).into();
        candidate_fixture.save();
        let candidate = VerifiedWindowsInstaller::verify_at(
            &candidate_fixture.directory,
            next,
            &[TrustedUpdateKey {
                id: "fixture",
                bytes: candidate_fixture.signing.verifying_key().to_bytes(),
            }],
            &AllowedUpdateRoot::parse(ROOT).unwrap(),
            OffsetDateTime::parse("2026-10-01T00:00:00Z", &Rfc3339).unwrap(),
        )
        .unwrap();
        let installed = Version::parse(VERSION).unwrap();
        let pair =
            WindowsInstallerPair::bind_versions(&installed, recovery.clone(), candidate.clone())
                .unwrap();
        pair.revalidate().unwrap();
        assert!(
            WindowsInstallerPair::bind_versions(&installed, candidate.clone(), recovery.clone())
                .is_err()
        );
        assert!(
            WindowsInstallerPair::bind_versions(
                &Version::parse("0.1.2-rc.1").unwrap(),
                recovery.clone(),
                candidate.clone()
            )
            .is_err()
        );
        assert!(
            WindowsInstallerPair::bind_versions(&installed, recovery.clone(), recovery.clone())
                .is_err()
        );
        let mut same_hash = candidate.clone();
        same_hash.package.sha256 = recovery.sha256().into();
        assert!(
            WindowsInstallerPair::bind_versions(&installed, recovery.clone(), same_hash).is_err()
        );
        fs::write(pair.recovery().path(), b"modified after preparation").unwrap();
        assert!(pair.revalidate().is_err());
    }
}

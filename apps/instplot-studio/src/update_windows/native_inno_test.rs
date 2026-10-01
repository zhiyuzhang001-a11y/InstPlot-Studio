//! Opt-in disposable Windows CI integration. Never run on a user installation.
use super::*;
use crate::update_transaction::{TransactionStore, UpdateIdentity, UpdateStage, UpdateTransaction};
use crate::update_windows::{
    InstallScope, RunningWindowsInstaller, STUDIO_APP_ID, TrackedWindowsProcess,
    WindowsInstallAccess, WindowsInstallRecord,
};
use ed25519_dalek::{Signer, SigningKey};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;
use time::format_description::well_known::Rfc3339;

const ROOT: &str = "https://downloads.example.test/instplot-studio";

fn environment_path(name: &str, root: &Path) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).expect("missing explicit CI fixture path"));
    reject_redirected_path(&path).unwrap();
    let path = fs::canonicalize(path).unwrap();
    assert!(
        path.starts_with(root) && path != root,
        "fixture must be inside the disposable runner directory"
    );
    path
}

fn hash(path: &Path) -> String {
    let mut file = File::open(path).unwrap();
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    format!("{:x}", digest.finalize())
}

fn signed_installer(source: &Path, directory: &Path, version: &str) -> VerifiedWindowsInstaller {
    crate::update_windows::create_private_directory(directory).unwrap();
    let name = format!("InstPlot-Studio-{version}-windows-x86_64-setup.exe");
    let destination = directory.join(&name);
    let mut input = File::open(source).unwrap();
    let mut output = crate::update_windows::create_private_file(&destination).unwrap();
    io::copy(&mut input, &mut output).unwrap();
    output.sync_all().unwrap();
    drop(output);
    let package_size = fs::metadata(&destination).unwrap().len();
    assert!(package_size > 0 && package_size <= MAX_INSTALLER_BYTES);
    let raw = serde_json::to_vec(&serde_json::json!({
        "schema":1,"product":"instplot-studio","version":version,"channel":"prerelease",
        "key_id":"fixture","release_sequence":2,"published_at":"2026-09-29T00:00:00Z",
        "expires_at":"2026-12-29T00:00:00Z",
        "notes_url":format!("https://github.com/zhiyuzhang001-a11y/InstPlot-Studio/releases/tag/v{version}"),
        "signature_url":format!("{ROOT}/releases/{version}/metadata/2/manifest.json.sig"),
        "platforms":{"windows-x86_64":{"preferred":"inno-setup","packages":[{
            "id":"inno-setup","package_type":"inno-setup","file_name":name,
            "minimum_system":"Windows 10","size_bytes":package_size,
            "sha256":hash(&destination),"url":format!("{ROOT}/releases/{version}/{name}")
        }]}}
    })).unwrap();
    let signing = SigningKey::from_bytes(&[7; 32]);
    for (name, bytes) in [
        ("manifest.json", raw.clone()),
        ("manifest.json.sig", signing.sign(&raw).to_bytes().to_vec()),
    ] {
        let mut file = crate::update_windows::create_private_file(&directory.join(name)).unwrap();
        file.write_all(&bytes).unwrap();
        file.sync_all().unwrap();
    }
    // Fixture key/root only inside cfg(test). Production verification is NOT
    // relaxed or redirected to this source.
    let verified = VerifiedWindowsInstaller::verify_at(
        directory,
        version,
        &[TrustedUpdateKey {
            id: "fixture",
            bytes: signing.verifying_key().to_bytes(),
        }],
        &AllowedUpdateRoot::parse(ROOT).unwrap(),
        OffsetDateTime::parse("2026-10-01T00:00:00Z", &Rfc3339).unwrap(),
    )
    .unwrap();
    for name in ["manifest.json", "manifest.json.sig", &name] {
        crate::update_windows::validate_private_file(&directory.join(name)).unwrap();
    }
    VerifiedWindowsInstaller {
        enforce_private_acl: true,
        ..verified
    }
}

fn installed(directory: &Path, version: &str, desktop: bool) -> WindowsInstallation {
    let record = WindowsInstallRecord {
        app_id: STUDIO_APP_ID.into(),
        scope: InstallScope::CurrentUser,
        directory: directory.to_path_buf(),
        version: version.into(),
        desktop_shortcut: desktop,
    };
    let installation =
        WindowsInstallation::bind(&record, &directory.join("instplot-studio.exe"), version)
            .unwrap();
    crate::update_windows::native::revalidate_installation(&installation).unwrap();
    let info = Command::new(installation.executable())
        .arg("--product-info")
        .output()
        .unwrap();
    assert!(info.status.success());
    assert_eq!(
        String::from_utf8(info.stdout).unwrap().trim(),
        format!("InstPlot Studio\tinstplot-studio\t{version}")
    );
    installation
}

fn wait_installer(mut running: RunningWindowsInstaller<'_>, log: &Path, evidence: &Path) {
    // Keep the running object and its leases/lock even on checkpoint errors.
    // The workflow's disposable runner timeout remains the external test bound.
    let mut last_checkpoint_error = None;
    let status = loop {
        match running.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                let error = error.to_string();
                if last_checkpoint_error.as_ref() != Some(&error) {
                    eprintln!("installer checkpoint not durable yet: {error}");
                    last_checkpoint_error = Some(error);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    fs::copy(log, evidence).unwrap();
    assert!(
        status.success(),
        "real native Inno execution failed: {status}"
    );
}

#[test]
#[ignore = "explicit disposable Windows installer recovery workflow only"]
fn real_inno_native_runner_updates_and_restores() {
    assert_eq!(std::env::var("GITHUB_ACTIONS").as_deref(), Ok("true"));
    assert_eq!(std::env::var("RUNNER_OS").as_deref(), Ok("Windows"));
    assert_eq!(
        std::env::var("INSTPLOT_NATIVE_INNO_PROTOTYPE").as_deref(),
        Ok("1")
    );
    let runner_root = fs::canonicalize(std::env::var_os("RUNNER_TEMP").unwrap()).unwrap();
    let directory = environment_path("INSTPLOT_NATIVE_INNO_DIRECTORY", &runner_root);
    let old_source = environment_path("INSTPLOT_NATIVE_INNO_OLD", &runner_root);
    let new_source = environment_path("INSTPLOT_NATIVE_INNO_NEW", &runner_root);
    let version = std::env::var("INSTPLOT_NATIVE_INNO_NEXT_VERSION").unwrap();
    let desktop = match std::env::var("INSTPLOT_NATIVE_INNO_DESKTOP")
        .unwrap()
        .as_str()
    {
        "true" => true,
        "false" => false,
        _ => panic!("invalid desktop task choice"),
    };
    let workspace = fs::canonicalize(std::env::var_os("GITHUB_WORKSPACE").unwrap()).unwrap();
    let evidence_directory = workspace.join("target/windows-update-recovery");
    assert!(evidence_directory.is_dir());
    let name = if desktop { "desktop-on" } else { "desktop-off" };
    let original = installed(&directory, env!("CARGO_PKG_VERSION"), desktop);
    original.validate_candidate_version(&version).unwrap();
    let original_hash = hash(original.executable());
    let user_file = directory.join("user-project.instplot");
    let user_hash = hash(&user_file);

    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).unwrap();
    let cache = runner_root.join(format!(
        "studio-native-inno 中文-{:032x}",
        u128::from_le_bytes(random)
    ));
    crate::update_windows::create_private_directory(&cache).unwrap();
    let cache = fs::canonicalize(cache).unwrap();
    let recovery = signed_installer(
        &old_source,
        &cache.join("old"),
        &original.version().to_string(),
    );
    let candidate = signed_installer(&new_source, &cache.join("new"), &version);
    let pair =
        WindowsInstallerPair::bind_versions(original.version(), recovery, candidate).unwrap();
    let pins = pair.pin().unwrap(); // BOTH obtained before the actual old process exits.
    let mut previous = Command::new(original.executable())
        .args([
            std::ffi::OsStr::new("--export-fixed-png"),
            cache.join("old-process.png").as_os_str(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let tracked = TrackedWindowsProcess::bind_child(&previous, original.executable()).unwrap();
    assert!(tracked.wait_for_exit(Duration::from_secs(30)).unwrap());
    assert!(previous.wait().unwrap().success());
    let access = WindowsInstallAccess::exclusive(original.directory()).unwrap();
    let transaction_directory = cache.join("transaction");
    crate::update_windows::create_private_directory(&transaction_directory).unwrap();
    let store = TransactionStore::lock(&transaction_directory).unwrap();
    let mut transaction = UpdateTransaction::new(UpdateIdentity {
        product: "instplot-studio".into(),
        platform: "windows-x86_64".into(),
        installed_path: original.directory().to_path_buf(),
        previous_version: original.version().to_string(),
        candidate_version: version.clone(),
        candidate_sha256: pair.candidate().sha256().into(),
        candidate_size: pair.candidate().size_bytes(),
    })
    .unwrap();
    transaction.transition(UpdateStage::WaitingForExit).unwrap();
    transaction.transition(UpdateStage::Applying).unwrap();
    store.write(&transaction).unwrap();
    let apply_log = transaction_directory.join("apply.log");
    let running = RunningWindowsInstaller::start(
        &original,
        pins.candidate,
        &access,
        &tracked,
        &apply_log,
        &store,
        &transaction,
    )
    .unwrap();
    wait_installer(
        running,
        &apply_log,
        &evidence_directory.join(format!("{name}-native-apply.log")),
    );
    let candidate_installation = installed(&directory, &version, desktop);
    assert_eq!(candidate_installation.directory(), original.directory());
    assert_ne!(hash(candidate_installation.executable()), original_hash);
    assert_eq!(hash(&user_file), user_hash);
    transaction
        .transition(UpdateStage::RecoveryRequired)
        .unwrap();
    transaction.transition(UpdateStage::Restoring).unwrap();
    store.write(&transaction).unwrap();
    let restore_log = transaction_directory.join("restore.log");
    let running = RunningWindowsInstaller::start(
        &original,
        pins.recovery,
        &access,
        &tracked,
        &restore_log,
        &store,
        &transaction,
    )
    .unwrap();
    wait_installer(
        running,
        &restore_log,
        &evidence_directory.join(format!("{name}-native-restore.log")),
    );
    let restored = installed(&directory, &original.version().to_string(), desktop);
    assert_eq!(restored.directory(), original.directory());
    assert_eq!(hash(restored.executable()), original_hash);
    assert_eq!(hash(&user_file), user_hash);
    // Installation/identity proof only. No candidate GUI or health receipt was
    // produced, so do NOT call this transaction Completed or GUI rollback passed.
    let receipt = serde_json::to_vec(&serde_json::json!({
        "product":"instplot-studio","scope":"real-native-runner-inno-not-GUI-updater",
        "applied":true,"restored":true,"user_data_preserved":true,
        "previous_version":original.version().to_string(),"candidate_version":version,
        "desktop_shortcut":desktop
    }))
    .unwrap();
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(evidence_directory.join(format!("{name}-native-runner.json")))
        .unwrap();
    output.write_all(&receipt).unwrap();
    output.sync_all().unwrap();
    drop(output);
    drop(store);
    drop(access);
    // This exact disposable test cache only; never installation or recovery backups.
    fs::remove_dir_all(cache).unwrap();
}

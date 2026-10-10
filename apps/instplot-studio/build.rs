use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::PathBuf;

use ed25519_dalek::VerifyingKey;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustConfig {
    public_root: String,
    keys: Vec<TrustKey>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustKey {
    id: String,
    public_key_hex: String,
}

fn main() {
    embed_windows_icon();
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let trust_path = manifest_dir.join("../../packaging/update/trust.json");
    println!("cargo:rerun-if-changed={}", trust_path.display());
    let config: TrustConfig =
        serde_json::from_slice(&fs::read(&trust_path).expect("read packaging/update/trust.json"))
            .expect("parse packaging/update/trust.json");
    assert!(config.public_root.starts_with("https://"));
    assert_eq!(config.keys.len(), 2, "current and next keys are required");
    let mut generated = format!(
        "pub const PRODUCTION_PUBLIC_ROOT: &str = {:?};\n",
        config.public_root
    );
    generated.push_str("pub const PRODUCTION_TRUSTED_KEYS: [TrustedUpdateKey; 2] = [\n");
    let mut key_ids = BTreeSet::new();
    let mut key_bytes = BTreeSet::new();
    for key in config.keys {
        assert!(!key.id.is_empty());
        assert!(key_ids.insert(key.id.clone()), "duplicate update key id");
        assert_eq!(key.public_key_hex.len(), 64);
        assert!(
            key.public_key_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
        let decoded = (0..32)
            .map(|index| {
                u8::from_str_radix(&key.public_key_hex[index * 2..index * 2 + 2], 16)
                    .expect("valid public key hex")
            })
            .collect::<Vec<_>>();
        let bytes: [u8; 32] = decoded.try_into().expect("32-byte public key");
        assert!(key_bytes.insert(bytes), "duplicate update public key");
        VerifyingKey::from_bytes(&bytes).expect("valid Ed25519 public key");
        let bytes = bytes
            .into_iter()
            .map(|byte| format!("0x{byte:02x}"))
            .collect::<Vec<_>>()
            .join(", ");
        generated.push_str(&format!(
            "TrustedUpdateKey {{ id: {:?}, bytes: [{bytes}] }},\n",
            key.id
        ));
    }
    generated.push_str("];\n");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("update_trust.rs");
    fs::write(output, generated).expect("write generated update trust constants");
}

#[cfg(windows)]
fn embed_windows_icon() {
    let icon = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"))
        .join("assets/InstPlotStudio.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon(icon.to_str().expect("icon path"))
            .set("ProductName", "InstPlot Studio")
            .set("FileDescription", "InstPlot Studio")
            .compile()
            .expect("embed Windows product icon");
    }
}

#[cfg(not(windows))]
fn embed_windows_icon() {}

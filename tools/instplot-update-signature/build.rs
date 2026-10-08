use std::{env, fs, path::PathBuf};

fn main() {
    // The verifier includes Studio's exact manifest module. Its legacy built-in
    // constants must therefore be generated too; CLI verification uses explicit
    // fixture keys/root instead, never a signing secret or a relaxed validator.
    let path = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../packaging/update/trust.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let trust: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let mut generated = format!(
        "pub const PRODUCTION_PUBLIC_ROOT: &str = {:?};\n",
        trust["public_root"].as_str().unwrap()
    );
    generated.push_str("pub const PRODUCTION_TRUSTED_KEYS: [TrustedUpdateKey; 2] = [\n");
    let keys = trust["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 2);
    for key in keys {
        let hex = key["public_key_hex"].as_str().unwrap();
        assert_eq!(hex.len(), 64);
        let bytes = (0..32)
            .map(|index| format!("0x{}", &hex[index * 2..index * 2 + 2]))
            .collect::<Vec<_>>()
            .join(",");
        generated.push_str(&format!(
            "TrustedUpdateKey {{ id: {:?}, bytes: [{}] }},\n",
            key["id"].as_str().unwrap(),
            bytes
        ));
    }
    generated.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("update_trust.rs"),
        generated,
    )
    .unwrap();
}

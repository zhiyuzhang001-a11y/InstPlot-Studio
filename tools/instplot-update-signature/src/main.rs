use std::env;
use std::fs;
use std::path::Path;

use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

// Compile the SAME validator the GUI calls; do not duplicate its URL/date/schema rules.
#[allow(dead_code)]
#[path = "../../../apps/instplot-studio/src/update.rs"]
mod studio_manifest;

fn usage() -> ! {
    eprintln!(
        "usage: instplot-update-signature public-key-hex PRIVATE.pem\n\
         or: instplot-update-signature sign PRIVATE.pem INPUT OUTPUT\n\
         or: instplot-update-signature verify PUBLIC_KEY_HEX INPUT SIGNATURE\n\
         or: instplot-update-signature verify-manifest KEY_ID PUBLIC_KEY_HEX ROOT INPUT SIGNATURE EXPECTED_VERSION"
    );
    std::process::exit(2);
}

fn load_signing_key(path: &Path) -> Result<SigningKey, String> {
    let pem = fs::read_to_string(path).map_err(|error| error.to_string())?;
    SigningKey::from_pkcs8_pem(&pem).map_err(|error| error.to_string())
}

fn decode_public_key(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("public key must contain exactly 64 hexadecimal characters".to_owned());
    }
    let mut bytes = [0_u8; 32];
    for (index, output) in bytes.iter_mut().enumerate() {
        *output = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|error| error.to_string())?;
    }
    Ok(bytes)
}

fn public_key_hex(key: &SigningKey) -> String {
    key.verifying_key()
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn run() -> Result<(), String> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match arguments.as_slice() {
        [command, key_id, public_key, root, input, signature, version]
            if command == "verify-manifest" =>
        {
            let expected = semver::Version::parse(version).map_err(|error| error.to_string())?;
            let key = studio_manifest::TrustedUpdateKey {
                id: Box::leak(key_id.clone().into_boxed_str()),
                bytes: decode_public_key(public_key)?,
            };
            let allowed = studio_manifest::AllowedUpdateRoot::parse(root)
                .map_err(|error| error.to_string())?;
            let manifest = studio_manifest::verify_signed_manifest(
                &fs::read(input).map_err(|error| error.to_string())?,
                &fs::read(signature).map_err(|error| error.to_string())?,
                &[key],
                &allowed,
                studio_manifest::UpdateChannel::for_version(&expected),
            )
            .map_err(|error| error.to_string())?;
            if manifest.version != *version {
                return Err("signed manifest version differs from expected version".into());
            }
            println!("Studio client manifest validation: PASS ({version})");
            Ok(())
        }
        [command, private_key] if command == "public-key-hex" => {
            println!(
                "{}",
                public_key_hex(&load_signing_key(Path::new(private_key))?)
            );
            Ok(())
        }
        [command, private_key, input, output] if command == "sign" => {
            let key = load_signing_key(Path::new(private_key))?;
            let message = fs::read(input).map_err(|error| error.to_string())?;
            let signature = key.sign(&message);
            fs::write(output, signature.to_bytes()).map_err(|error| error.to_string())
        }
        [command, public_key, input, signature] if command == "verify" => {
            let key = VerifyingKey::from_bytes(&decode_public_key(public_key)?)
                .map_err(|error| error.to_string())?;
            let message = fs::read(input).map_err(|error| error.to_string())?;
            let signature = fs::read(signature).map_err(|error| error.to_string())?;
            let signature = Signature::from_slice(&signature).map_err(|error| error.to_string())?;
            key.verify(&message, &signature)
                .map_err(|_| "signature verification failed".to_owned())
        }
        _ => usage(),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("instplot-update-signature: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_hex_decoder_is_strict() {
        assert_eq!(decode_public_key(&"ab".repeat(32)).unwrap(), [0xab; 32]);
        assert!(decode_public_key("ab").is_err());
        assert!(decode_public_key(&"zz".repeat(32)).is_err());
    }
}

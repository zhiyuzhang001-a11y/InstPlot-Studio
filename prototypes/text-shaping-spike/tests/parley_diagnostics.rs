#![cfg(feature = "parley-candidate")]

use std::process::Command;

#[test]
fn unsupported_cjk_is_reported_without_system_fallback() {
    let output = Command::new(env!("CARGO_BIN_EXE_parley-probe"))
        .output()
        .expect("run parley-probe");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("No segmentation model for complex script"),
        "unexpected ICU4X diagnostic: {stderr}"
    );
    assert!(stderr.trim().is_empty(), "unexpected stderr: {stderr}");

    let stdout = String::from_utf8(output.stdout).expect("probe output must be UTF-8");
    let cjk = stdout
        .split("LABEL unsupported-cjk\n")
        .nth(1)
        .and_then(|section| section.split("LABEL missing-glyph\n").next())
        .expect("unsupported CJK section");
    assert!(!cjk.contains("RUN "), "unsupported text must not be shaped");
    assert!(stdout.contains("WARNING unsupported-cjk: unsupported-script"));
    assert!(stdout.contains("WARNING missing-glyph:"));
}

use std::process::Command;

#[test]
fn headless_export_does_not_create_a_window() {
    let directory = std::env::temp_dir().join(format!("sciplot-a6-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let output = directory.join("headless.svg");
    let status = Command::new(env!("CARGO_BIN_EXE_ui-shell-spike"))
        .arg("--headless-svg")
        .arg(&output)
        .status()
        .unwrap();
    assert!(status.success());
    let svg = std::fs::read_to_string(&output).unwrap();
    assert!(svg.contains("252.28346pt"));
    assert!(svg.contains("184.25197pt"));
    let _ = std::fs::remove_file(output);
    let _ = std::fs::remove_dir(directory);
}

#[cfg(feature = "publication-stack")]
#[test]
fn publication_pdf_export_does_not_create_a_window() {
    let directory = std::env::temp_dir().join(format!("sciplot-a6-pdf-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let output = directory.join("headless.pdf");
    let status = Command::new(env!("CARGO_BIN_EXE_ui-shell-spike"))
        .arg("--publication-pdf")
        .arg(&output)
        .status()
        .unwrap();
    assert!(status.success());
    let pdf = std::fs::read(&output).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    let _ = std::fs::remove_file(output);
    let _ = std::fs::remove_dir(directory);
}

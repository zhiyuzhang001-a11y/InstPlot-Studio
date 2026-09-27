use instplot_demo::QuickPlotApp;
use instplot_ui::ShellEvent;

#[test]
fn second_product_imports_selects_labels_and_exports_without_copying_studio_ui() {
    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("instplot-studio")
        .join("tests")
        .join("fixtures")
        .join("smoke.csv");
    let output = std::env::temp_dir().join(format!(
        "instplot-quick-workflow-{}.svg",
        std::process::id()
    ));
    let mut app = QuickPlotApp::default();
    let bytes = app
        .import_select_label_export(
            &input,
            "field",
            "response",
            "Applied field (mT)",
            "Response (a.u.)",
            &output,
        )
        .unwrap();
    let svg = std::fs::read_to_string(&output).unwrap();
    assert!(bytes > 1_000);
    assert!(svg.starts_with("<svg "));
    assert!(svg.contains("Applied field (mT)"));
    assert!(svg.contains("Response (a.u.)"));
    assert_eq!(app.branding.product_name, "InstPlot Quick");
    assert_eq!(app.last_event, Some(ShellEvent::ExportSvg));
    assert!(!app.features.project_files && !app.features.publication_check);
    std::fs::remove_file(output).unwrap();
}

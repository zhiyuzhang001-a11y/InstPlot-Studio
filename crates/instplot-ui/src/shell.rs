#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branding {
    pub product_name: String,
    pub version: String,
    pub build_id: Option<String>,
}

impl Branding {
    pub fn new(product_name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            product_name: product_name.into(),
            version: version.into(),
            build_id: None,
        }
    }

    pub fn with_build_id(mut self, build_id: impl Into<String>) -> Self {
        self.build_id = Some(build_id.into());
        self
    }

    pub fn window_title(&self, document_name: &str, dirty: bool) -> String {
        format!(
            "{} — {}{}",
            self.product_name,
            document_name,
            if dirty { " *" } else { "" }
        )
    }

    pub fn footer_label(&self) -> String {
        match self.build_id.as_deref() {
            Some(build_id) => format!("{} · v{} · {build_id}", self.product_name, self.version),
            None => format!("{} · v{}", self.product_name, self.version),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeatureSet {
    pub data_import: bool,
    pub manual_data: bool,
    pub project_files: bool,
    pub annotations: bool,
    pub publication_check: bool,
    pub export_pdf: bool,
    pub export_png: bool,
    pub export_svg: bool,
}

impl FeatureSet {
    pub const STUDIO: Self = Self {
        data_import: true,
        manual_data: true,
        project_files: true,
        annotations: true,
        publication_check: true,
        export_pdf: true,
        export_png: true,
        export_svg: true,
    };
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShellEvent {
    NewProject,
    ImportData,
    OpenProject,
    SaveProject,
    EnterData,
    InsertAnnotation,
    ExportPdf,
    ExportPng,
    ExportSvg,
    TogglePublicationCheck,
}

pub trait AppServices {
    type Error;

    fn dispatch(&mut self, event: ShellEvent) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branding_owns_only_product_identity() {
        let branding = Branding::new("Plot Demo", "1.2.3").with_build_id("qa");
        assert_eq!(
            branding.window_title("sample.csv", true),
            "Plot Demo — sample.csv *"
        );
        assert_eq!(branding.footer_label(), "Plot Demo · v1.2.3 · qa");

        let public_branding = Branding::new("Plot Demo", "1.2.3");
        assert_eq!(public_branding.footer_label(), "Plot Demo · v1.2.3");
    }

    #[test]
    fn feature_sets_can_disable_product_workflows_without_changing_components() {
        let mut features = FeatureSet::STUDIO;
        features.publication_check = false;
        features.project_files = false;
        assert!(features.data_import && features.export_png);
        assert!(!features.publication_check && !features.project_files);
    }
}

use std::error::Error;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Instant;

use eframe::egui;
use instplot_studio::{
    FigureDocument, HandoffCleanup, HandoffImport, PRODUCT_NAME, StudioSession, check_publication,
    import_handoff, product_info, resolve_document, save_fixed_figure_pdf, save_fixed_figure_png,
    write_handoff,
};

use super::StudioApp;

pub(crate) fn run(arguments: impl IntoIterator<Item = OsString>) -> Result<(), Box<dyn Error>> {
    match StartupCommand::parse(arguments)? {
        StartupCommand::ProductInfo => {
            println!("{}", product_info());
            Ok(())
        }
        StartupCommand::ExportFixedPdf(path) => {
            let size = save_fixed_figure_pdf(&path)?;
            println!("exported_pdf={} bytes={size}", path.display());
            Ok(())
        }
        StartupCommand::ExportFixedPng(path) => {
            let size = save_fixed_figure_png(&path, 300)?;
            println!("exported_png={} dpi=300 bytes={size}", path.display());
            Ok(())
        }
        StartupCommand::CreateProject(path) => {
            let document = FigureDocument::fixed();
            document.save(&path)?;
            println!(
                "created_project={} schema={}",
                path.display(),
                document.project().schema_version
            );
            Ok(())
        }
        StartupCommand::CheckProject(path) => {
            let (document, report) = FigureDocument::open(&path)?;
            document.layout_figure()?;
            println!(
                "checked_project={} schema={} source={:?} warnings={}",
                path.display(),
                document.project().schema_version,
                report.source,
                report.warnings.len()
            );
            for warning in report.warnings {
                println!("warning={warning}");
            }
            Ok(())
        }
        StartupCommand::PublicationCheck(path) => {
            let (document, _) = FigureDocument::open(&path)?;
            let resolved = resolve_document(&document)?;
            let report = check_publication(
                &document,
                &resolved,
                document.export_preferences().selected_raster_dpi,
            );
            println!("{}", serde_json::to_string_pretty(&report)?);
            if report.error_count() > 0 {
                Err(format!("publication check found {} error(s)", report.error_count()).into())
            } else {
                Ok(())
            }
        }
        StartupCommand::CreateHandoff { source, output } => {
            let mut session = StudioSession::default();
            let outcome = session.import_data_file(&source)?;
            let size = write_handoff(
                &output,
                session.datasets(),
                "InstPlot Lite compatible producer",
                env!("CARGO_PKG_VERSION"),
            )?;
            println!(
                "created_handoff={} datasets={} bytes={size}",
                output.display(),
                outcome.read
            );
            Ok(())
        }
        StartupCommand::ImportHandoff { source, project } => {
            let imported = import_handoff(&source, HandoffCleanup::Keep)?;
            imported.document.save(&project)?;
            println!(
                "imported_handoff={} project={} datasets={} source_removed=false",
                source.display(),
                project.display(),
                imported.datasets.len()
            );
            Ok(())
        }
        StartupCommand::OpenHandoff(path) => {
            let imported = import_handoff(&path, HandoffCleanup::DeleteAfterImport)?;
            launch_gui(Some(imported), None).map_err(Into::into)
        }
        StartupCommand::OpenProject(path) => launch_gui(None, Some(path)).map_err(Into::into),
        StartupCommand::Gui => launch_gui(None, None).map_err(Into::into),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum StartupCommand {
    Gui,
    ProductInfo,
    ExportFixedPdf(PathBuf),
    ExportFixedPng(PathBuf),
    CreateProject(PathBuf),
    CheckProject(PathBuf),
    PublicationCheck(PathBuf),
    CreateHandoff { source: PathBuf, output: PathBuf },
    ImportHandoff { source: PathBuf, project: PathBuf },
    OpenHandoff(PathBuf),
    OpenProject(PathBuf),
}

impl StartupCommand {
    fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, &'static str> {
        let mut arguments = arguments.into_iter();
        match (arguments.next(), arguments.next(), arguments.next()) {
            (None, None, None) => Ok(Self::Gui),
            (Some(flag), None, None) if flag == "--product-info" => Ok(Self::ProductInfo),
            (Some(flag), Some(path), None) if flag == "--export-fixed-pdf" => {
                Ok(Self::ExportFixedPdf(path.into()))
            }
            (Some(flag), Some(path), None) if flag == "--export-fixed-png" => {
                Ok(Self::ExportFixedPng(path.into()))
            }
            (Some(flag), Some(path), None) if flag == "--create-project" => {
                Ok(Self::CreateProject(path.into()))
            }
            (Some(flag), Some(path), None) if flag == "--check-project" => {
                Ok(Self::CheckProject(path.into()))
            }
            (Some(flag), Some(path), None) if flag == "--publication-check" => {
                Ok(Self::PublicationCheck(path.into()))
            }
            (Some(flag), Some(path), None) if flag == "--open-handoff" => {
                Ok(Self::OpenHandoff(path.into()))
            }
            (Some(path), None, None)
                if PathBuf::from(&path)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("instplot")) =>
            {
                Ok(Self::OpenProject(path.into()))
            }
            (Some(flag), Some(source), Some(output)) if flag == "--create-handoff" => {
                Ok(Self::CreateHandoff {
                    source: source.into(),
                    output: output.into(),
                })
            }
            (Some(flag), Some(source), Some(project)) if flag == "--import-handoff" => {
                Ok(Self::ImportHandoff {
                    source: source.into(),
                    project: project.into(),
                })
            }
            _ => Err(
                "usage: instplot-studio [--product-info | --export-fixed-pdf PATH | --export-fixed-png PATH | --create-project PATH | --check-project PATH | --publication-check PATH | --create-handoff SOURCE PACKAGE | --import-handoff PACKAGE PROJECT | --open-handoff PACKAGE]",
            ),
        }
    }
}

fn launch_gui(startup: Option<HandoffImport>, project_path: Option<PathBuf>) -> eframe::Result {
    let started = Instant::now();
    #[cfg(target_os = "macos")]
    let macos_open_files = crate::macos_open_files::MacOpenFiles::start();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(PRODUCT_NAME)
            .with_inner_size([1160.0, 780.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        "instplot-studio",
        options,
        Box::new(move |creation| {
            let mut app = StudioApp::new(creation, started, startup);
            #[cfg(target_os = "macos")]
            {
                macos_open_files.finish_install(creation.egui_ctx.clone());
                app.macos_open_files = Some(macos_open_files);
            }
            if let Some(path) = project_path {
                app.open_project_path(path);
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_commands_keep_headless_work_before_gui_creation() {
        assert_eq!(StartupCommand::parse([]).unwrap(), StartupCommand::Gui);
        assert_eq!(
            StartupCommand::parse([OsString::from("--product-info")]).unwrap(),
            StartupCommand::ProductInfo
        );
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--export-fixed-pdf"),
                OsString::from("figure.pdf")
            ])
            .unwrap(),
            StartupCommand::ExportFixedPdf(PathBuf::from("figure.pdf"))
        );
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--export-fixed-png"),
                OsString::from("figure.png")
            ])
            .unwrap(),
            StartupCommand::ExportFixedPng(PathBuf::from("figure.png"))
        );
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--create-project"),
                OsString::from("figure.instplot")
            ])
            .unwrap(),
            StartupCommand::CreateProject(PathBuf::from("figure.instplot"))
        );
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--check-project"),
                OsString::from("figure.instplot")
            ])
            .unwrap(),
            StartupCommand::CheckProject(PathBuf::from("figure.instplot"))
        );
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--publication-check"),
                OsString::from("figure.instplot")
            ])
            .unwrap(),
            StartupCommand::PublicationCheck(PathBuf::from("figure.instplot"))
        );
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--open-handoff"),
                OsString::from("transfer.instplot-handoff")
            ])
            .unwrap(),
            StartupCommand::OpenHandoff(PathBuf::from("transfer.instplot-handoff"))
        );
        assert_eq!(
            StartupCommand::parse([OsString::from("figure.instplot")]).unwrap(),
            StartupCommand::OpenProject(PathBuf::from("figure.instplot"))
        );
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--create-handoff"),
                OsString::from("lite.txt"),
                OsString::from("transfer.instplot-handoff")
            ])
            .unwrap(),
            StartupCommand::CreateHandoff {
                source: PathBuf::from("lite.txt"),
                output: PathBuf::from("transfer.instplot-handoff")
            }
        );
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--import-handoff"),
                OsString::from("transfer.instplot-handoff"),
                OsString::from("figure.instplot")
            ])
            .unwrap(),
            StartupCommand::ImportHandoff {
                source: PathBuf::from("transfer.instplot-handoff"),
                project: PathBuf::from("figure.instplot")
            }
        );
        assert!(StartupCommand::parse([OsString::from("--unknown")]).is_err());
    }
}

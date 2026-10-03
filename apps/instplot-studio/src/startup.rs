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
        StartupCommand::UpdateProtocol => {
            println!("{}", if cfg!(target_os = "macos") { 1 } else { 0 });
            Ok(())
        }
        #[cfg(windows)]
        StartupCommand::CheckUpdateInstallation => {
            let installed = instplot_studio::update_windows::discover_current_installation()?;
            println!(
                "{}",
                serde_json::json!({
                    "product": "instplot-studio", "version": installed.version().to_string(),
                    "scope": "current_user", "running_path_matches": true,
                    "desktop_shortcut": installed.desktop_shortcut(),
                })
            );
            Ok(())
        }
        #[cfg(target_os = "macos")]
        StartupCommand::ApplyUpdate(path) => crate::update_macos::apply(&path).map_err(Into::into),
        #[cfg(any(target_os = "macos", all(windows, feature = "in-place-update-preview")))]
        StartupCommand::UpdateHealth(path) => {
            let health = HealthStartup::load(&path)?;
            launch_gui_with_health(health).map_err(Into::into)
        }
        #[cfg(all(windows, feature = "in-place-update-preview"))]
        StartupCommand::UpdateRecoveryHealth(path) => {
            let health = HealthStartup::load_recovery(&path)?;
            launch_gui_with_health(health).map_err(Into::into)
        }
        #[cfg(all(windows, feature = "in-place-update-preview"))]
        StartupCommand::WindowsUpdateHelper(path) => {
            let helper =
                instplot_studio::update_windows::WindowsHelperSession::load_waiting(&path)?;
            instplot_studio::update_windows::run_helper_after_preflight(&helper)?;
            Ok(())
        }
        #[cfg(all(windows, target_arch = "x86_64", feature = "in-place-update-preview"))]
        StartupCommand::WindowsUpdateCapabilities => {
            println!("{}", windows_preview_capabilities());
            Ok(())
        }
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
    UpdateProtocol,
    #[cfg(windows)]
    CheckUpdateInstallation,
    #[cfg(target_os = "macos")]
    ApplyUpdate(PathBuf),
    #[cfg(any(target_os = "macos", all(windows, feature = "in-place-update-preview")))]
    UpdateHealth(PathBuf),
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    UpdateRecoveryHealth(PathBuf),
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    WindowsUpdateHelper(PathBuf),
    #[cfg(all(windows, target_arch = "x86_64", feature = "in-place-update-preview"))]
    WindowsUpdateCapabilities,
    ProductInfo,
    ExportFixedPdf(PathBuf),
    ExportFixedPng(PathBuf),
    CreateProject(PathBuf),
    CheckProject(PathBuf),
    PublicationCheck(PathBuf),
    CreateHandoff {
        source: PathBuf,
        output: PathBuf,
    },
    ImportHandoff {
        source: PathBuf,
        project: PathBuf,
    },
    OpenHandoff(PathBuf),
    OpenProject(PathBuf),
}

impl StartupCommand {
    fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, &'static str> {
        let mut arguments = arguments.into_iter();
        match (arguments.next(), arguments.next(), arguments.next()) {
            (None, None, None) => Ok(Self::Gui),
            (Some(flag), None, None) if flag == "--update-protocol" => Ok(Self::UpdateProtocol),
            #[cfg(all(windows, target_arch = "x86_64", feature = "in-place-update-preview"))]
            (Some(flag), None, None) if flag == "--windows-update-capabilities" => {
                Ok(Self::WindowsUpdateCapabilities)
            }
            #[cfg(windows)]
            (Some(flag), None, None) if flag == "--check-update-installation" => {
                Ok(Self::CheckUpdateInstallation)
            }
            #[cfg(target_os = "macos")]
            (Some(flag), Some(path), None) if flag == "--apply-update" => {
                Ok(Self::ApplyUpdate(path.into()))
            }
            #[cfg(any(target_os = "macos", all(windows, feature = "in-place-update-preview")))]
            (Some(flag), Some(path), None) if flag == "--update-health" => {
                Ok(Self::UpdateHealth(path.into()))
            }
            #[cfg(all(windows, feature = "in-place-update-preview"))]
            (Some(flag), Some(path), None) if flag == "--update-recovery-health" => {
                Ok(Self::UpdateRecoveryHealth(path.into()))
            }
            #[cfg(all(windows, feature = "in-place-update-preview"))]
            (Some(flag), Some(path), None) if flag == "--windows-update-helper" => {
                Ok(Self::WindowsUpdateHelper(path.into()))
            }
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

/// Read-only introspection of compiled preview components. This deliberately
/// does NOT claim accepted GUI updates or enable publication/installation.
#[cfg(all(windows, target_arch = "x86_64", feature = "in-place-update-preview"))]
fn windows_preview_capabilities() -> serde_json::Value {
    serde_json::json!({
        "schema": 1,
        "product": "instplot-studio",
        "version": env!("CARGO_PKG_VERSION"),
        "platform": "windows-x86_64",
        "scope": "preview-components-not-accepted-updater",
        "public_update_protocol": 0,
        "gui_acceptance_complete": false,
        "public_apply_entry_enabled": false,
        "components": {
            "helper_protocol": 1,
            "transaction_schema": 1,
            "candidate_health_protocol": 1,
            "recovery_health_protocol": 1,
        },
    })
}

fn launch_gui(startup: Option<HandoffImport>, project_path: Option<PathBuf>) -> eframe::Result {
    launch_gui_inner(startup, project_path, None)
}

#[cfg(any(target_os = "macos", all(windows, feature = "in-place-update-preview")))]
fn launch_gui_with_health(health: HealthStartup) -> eframe::Result {
    launch_gui_inner(None, health.project(), Some(health))
}

#[cfg(not(any(target_os = "macos", all(windows, feature = "in-place-update-preview"))))]
type HealthStartup = ();
#[cfg(target_os = "macos")]
use crate::update_macos::HealthStartup;
#[cfg(all(windows, feature = "in-place-update-preview"))]
use instplot_studio::update_windows::WindowsHealthStartup as HealthStartup;

fn launch_gui_inner(
    startup: Option<HandoffImport>,
    project_path: Option<PathBuf>,
    health: Option<HealthStartup>,
) -> eframe::Result {
    #[cfg(target_os = "macos")]
    let guard = match crate::update_macos::current_bundle() {
        Ok(target) => {
            if health.is_none() && crate::update_macos::has_unfinished_apply(&target) {
                return Err(eframe::Error::AppCreation(
                    std::io::Error::other("原位更新或恢复尚未结束，请查看该事务的更新日志。")
                        .into(),
                ));
            }
            match instplot_studio::update_bundle::BundleAccess::shared(&target) {
                Ok(guard) => Ok(guard),
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock || health.is_some() =>
                {
                    return Err(eframe::Error::AppCreation(error.into()));
                }
                Err(error) => Err(error.to_string()),
            }
        }
        Err(error) => Err(error),
    };
    #[cfg(not(any(target_os = "macos", all(windows, feature = "in-place-update-preview"))))]
    let _ = health;
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    let windows_guard = {
        let executable = std::env::current_exe()
            .and_then(std::fs::canonicalize)
            .map_err(|error| eframe::Error::AppCreation(error.into()))?;
        let target = executable.parent().ok_or_else(|| {
            eframe::Error::AppCreation(std::io::Error::other("无法确定应用目录。").into())
        })?;
        // Every preview GUI holds access, including portable instances in the
        // same directory. An unlocked preview must not run during replacement.
        let guard = instplot_studio::update_windows::WindowsInstallAccess::shared(target)
            .map_err(|error| eframe::Error::AppCreation(error.into()))?;
        if health.is_none() {
            guard
                .require_idle_startup()
                .map_err(|error| eframe::Error::AppCreation(error.into()))?;
        }
        Ok(guard)
    };
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
            #[cfg(all(windows, feature = "in-place-update-preview"))]
            {
                app.update.set_windows_instance_guard(windows_guard);
                app.update_health = health;
            }
            #[cfg(target_os = "macos")]
            {
                app.update_health = health;
                app.update.set_instance_guard(guard);
            }
            #[cfg(target_os = "macos")]
            {
                macos_open_files.finish_install(creation.egui_ctx.clone());
                app.macos_open_files = Some(macos_open_files);
            }
            if let Some(path) = project_path {
                app.open_project_path(path.clone());
                #[cfg(any(target_os = "macos", all(windows, feature = "in-place-update-preview")))]
                if app.update_health.is_some()
                    && app.workspace.project_path() != Some(path.as_path())
                {
                    return Err(std::io::Error::other(
                        "更新启动未能重开指定主项目，不能确认健康。",
                    )
                    .into());
                }
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(windows, target_arch = "x86_64", feature = "in-place-update-preview"))]
    #[test]
    fn windows_capability_probe_is_exact_and_does_not_claim_accepted_updates() {
        let flag = "--windows-update-capabilities";
        assert_eq!(
            StartupCommand::parse([OsString::from(flag)]).unwrap(),
            StartupCommand::WindowsUpdateCapabilities
        );
        assert!(StartupCommand::parse([OsString::from(flag), OsString::from("extra")]).is_err());
        let value = windows_preview_capabilities();
        assert_eq!(value["product"], "instplot-studio");
        assert_eq!(value["platform"], "windows-x86_64");
        assert_eq!(value["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value["scope"], "preview-components-not-accepted-updater");
        assert_eq!(value["public_update_protocol"], 0);
        assert_eq!(value["gui_acceptance_complete"], false);
        assert_eq!(value["public_apply_entry_enabled"], false);
        assert_eq!(value["components"]["helper_protocol"], 1);
        assert_eq!(value["components"]["transaction_schema"], 1);
        assert_eq!(value["components"]["candidate_health_protocol"], 1);
        assert_eq!(value["components"]["recovery_health_protocol"], 1);
        assert!(run([OsString::from(flag)]).is_ok());
    }

    #[cfg(not(all(windows, target_arch = "x86_64", feature = "in-place-update-preview")))]
    #[test]
    fn normal_or_non_windows_build_rejects_preview_capability_command() {
        assert!(StartupCommand::parse([OsString::from("--windows-update-capabilities")]).is_err());
    }

    #[cfg(all(windows, feature = "in-place-update-preview"))]
    #[test]
    fn windows_helper_command_has_one_exact_private_transaction_argument() {
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--windows-update-helper"),
                OsString::from("私有 事务"),
            ])
            .unwrap(),
            StartupCommand::WindowsUpdateHelper(PathBuf::from("私有 事务"))
        );
        assert!(StartupCommand::parse([OsString::from("--windows-update-helper")]).is_err());
        assert!(
            StartupCommand::parse([
                OsString::from("--windows-update-helper"),
                OsString::from("transaction"),
                OsString::from("extra"),
            ])
            .is_err()
        );
        // The normal executable must fail authentication before entering the
        // helper loop: no private request can make its path a copied helper.
        assert!(
            run([
                OsString::from("--windows-update-helper"),
                std::env::temp_dir().into_os_string(),
            ])
            .is_err()
        );
    }

    #[cfg(not(all(windows, feature = "in-place-update-preview")))]
    #[test]
    fn normal_builds_do_not_expose_the_windows_helper_command() {
        assert!(
            StartupCommand::parse([
                OsString::from("--windows-update-helper"),
                OsString::from("transaction"),
            ])
            .is_err()
        );
    }

    #[cfg(any(target_os = "macos", all(windows, feature = "in-place-update-preview")))]
    #[test]
    fn health_startup_accepts_one_exact_transaction_path_only() {
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--update-health"),
                OsString::from("事务 中文 路径"),
            ])
            .unwrap(),
            StartupCommand::UpdateHealth(PathBuf::from("事务 中文 路径")),
        );
        assert!(StartupCommand::parse([OsString::from("--update-health")]).is_err());
        assert!(
            StartupCommand::parse([
                OsString::from("--update-health"),
                OsString::from("transaction"),
                OsString::from("another-project.instplot"),
            ])
            .is_err()
        );
    }

    #[test]
    #[cfg(all(windows, feature = "in-place-update-preview"))]
    fn recovery_health_startup_has_a_separate_exact_path_command() {
        assert_eq!(
            StartupCommand::parse([
                OsString::from("--update-recovery-health"),
                OsString::from("恢复 中文 路径"),
            ])
            .unwrap(),
            StartupCommand::UpdateRecoveryHealth(PathBuf::from("恢复 中文 路径")),
        );
        assert!(StartupCommand::parse([OsString::from("--update-recovery-health")]).is_err());
        assert!(
            StartupCommand::parse([
                OsString::from("--update-recovery-health"),
                OsString::from("transaction"),
                OsString::from("extra"),
            ])
            .is_err()
        );
    }

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

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceOrigin {
    New,
    Data,
    Lite,
    Project,
    RecoveredBackup,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceState {
    origin: WorkspaceOrigin,
    project_path: Option<PathBuf>,
    display_name: String,
}

impl WorkspaceState {
    pub fn new(untitled: &str) -> Self {
        Self {
            origin: WorkspaceOrigin::New,
            project_path: None,
            display_name: untitled.to_owned(),
        }
    }

    pub fn from_lite(untitled: &str) -> Self {
        Self {
            origin: WorkspaceOrigin::Lite,
            ..Self::new(untitled)
        }
    }

    pub fn from_project(path: &Path, recovered_backup: bool) -> Self {
        Self {
            origin: if recovered_backup {
                WorkspaceOrigin::RecoveredBackup
            } else {
                WorkspaceOrigin::Project
            },
            project_path: (!recovered_backup).then(|| path.to_path_buf()),
            display_name: path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("figure")
                .to_owned(),
        }
    }

    pub fn note_data_import(&mut self, path: &Path) {
        if self.project_path.is_none() && self.origin == WorkspaceOrigin::New {
            self.origin = WorkspaceOrigin::Data;
            if let Some(name) = path.file_stem().and_then(|name| name.to_str()) {
                self.display_name = name.to_owned();
            }
        }
    }

    pub fn note_saved(&mut self, path: PathBuf) {
        self.display_name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("figure")
            .to_owned();
        self.project_path = Some(path);
        self.origin = WorkspaceOrigin::Project;
    }

    pub fn project_path(&self) -> Option<&Path> {
        self.project_path.as_deref()
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_and_space_project_names_are_preserved() {
        let path = Path::new("/tmp/实验 数据/磁化 曲线.instplot");
        let workspace = WorkspaceState::from_project(path, false);
        assert_eq!(workspace.display_name(), "磁化 曲线");
        assert_eq!(workspace.project_path(), Some(path));
    }

    #[test]
    fn recovered_backup_requires_save_as() {
        let workspace = WorkspaceState::from_project(Path::new("/tmp/figure.instplot"), true);
        assert_eq!(workspace.origin, WorkspaceOrigin::RecoveredBackup);
        assert_eq!(workspace.project_path(), None);
        assert_eq!(workspace.display_name(), "figure");
    }
}

//! Saved-project binding for the post-confirmation Windows restart handoff.
//! Never saves, migrates, repairs, or falls back to another project file.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use windows::Win32::Storage::FileSystem::FILE_SHARE_READ;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowsResumeProject {
    path: PathBuf,
    sha256: String,
    size: u64,
}

impl WindowsResumeProject {
    pub(super) fn capture(path: &Path, installation: &Path) -> io::Result<(Self, File)> {
        super::reject_redirected_path(path)?;
        let path = std::fs::canonicalize(path)?;
        if path.starts_with(installation) {
            return Err(super::invalid(
                "resume project cannot be inside the installation",
            ));
        }
        let (file, size, sha256) = read_bound(&path)?;
        require_renderable_primary(&path)?;
        Ok((Self { path, sha256, size }, file))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn pin(&self, installation: &Path) -> io::Result<File> {
        if self.path.starts_with(installation)
            || std::fs::canonicalize(&self.path)? != self.path
            || self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(super::invalid("invalid saved project binding"));
        }
        let (file, size, hash) = read_bound(&self.path)?;
        if self.size != size || self.sha256 != hash {
            return Err(super::invalid(
                "saved project changed after restart confirmation",
            ));
        }
        require_renderable_primary(&self.path)?;
        Ok(file)
    }
}

fn read_bound(path: &Path) -> io::Result<(File, u64, String)> {
    super::reject_redirected_path(path)?;
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ.0)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 128 * 1024 * 1024 {
        return Err(super::invalid("invalid or oversized saved project"));
    }
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut size = 0_u64;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        size += count as u64;
        if size > 128 * 1024 * 1024 {
            return Err(super::invalid("saved project grew too large"));
        }
        hash.update(&buffer[..count]);
    }
    if size != metadata.len() {
        return Err(super::invalid("saved project changed while reading"));
    }
    Ok((file, size, format!("{:x}", hash.finalize())))
}

fn require_renderable_primary(path: &Path) -> io::Result<()> {
    // Reject a corrupt primary BEFORE open() can read any backup. The held
    // Windows lease prevents mutation between these reads.
    let mut raw = Vec::new();
    File::open(path)?
        .take(128 * 1024 * 1024 + 1)
        .read_to_end(&mut raw)?;
    if raw.len() > 128 * 1024 * 1024 {
        return Err(super::invalid("oversized saved project"));
    }
    crate::decode_project(&raw)
        .map(drop)
        .map_err(super::invalid)?;
    drop(raw);
    let (document, report) = crate::FigureDocument::open(path).map_err(super::invalid)?;
    if report.source != crate::project::OpenProjectSource::Primary {
        return Err(super::invalid("saved project requires primary-file repair"));
    }
    // Non-fatal source warnings do not invalidate embedded retained data.
    // Actual inability to reconstruct the figure must still reject the handoff.
    document.layout_figure().map_err(super::invalid)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_project_binding_is_exact_read_only_and_never_repairs() {
        let mut random = [0; 16];
        getrandom::fill(&mut random).unwrap();
        let root = std::env::temp_dir().join(format!(
            "studio-resume-{:032x}",
            u128::from_le_bytes(random)
        ));
        std::fs::create_dir(&root).unwrap();
        let installation = root.join("Studio");
        std::fs::create_dir(&installation).unwrap();
        let path = root.join("保存 项目.instplot");
        crate::FigureDocument::showcase().save(&path).unwrap();
        // Produce a valid backup; the update handoff must still reject a
        // subsequently corrupt primary, unlike ordinary interactive opening.
        crate::FigureDocument::showcase().save(&path).unwrap();
        let before = std::fs::read(&path).unwrap();
        let (bound, lease) = WindowsResumeProject::capture(&path, &installation).unwrap();
        assert_eq!(bound.path(), std::fs::canonicalize(&path).unwrap());
        assert!(std::fs::write(&path, b"changed").is_err());
        assert!(std::fs::remove_file(&path).is_err());
        drop(bound.pin(&installation).unwrap());
        let mut wrong = bound.clone();
        wrong.sha256 = "0".repeat(64);
        assert!(wrong.pin(&installation).is_err());
        assert!(WindowsResumeProject::capture(&path, &root).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        drop(lease);
        std::fs::write(&path, b"corrupt").unwrap();
        assert!(bound.pin(&installation).is_err());
        assert!(WindowsResumeProject::capture(&path, &installation).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"corrupt");
        std::fs::remove_dir_all(root).unwrap();
    }
}

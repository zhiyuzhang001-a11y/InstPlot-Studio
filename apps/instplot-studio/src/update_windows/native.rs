//! Read-only native Windows discovery. No shell commands or registry mutations.
use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize, IPersistFile, STGM_READ,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    REG_SAM_FLAGS, RRF_RT_REG_SZ, RegCloseKey, RegGetValueW, RegOpenKeyExW,
};
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_LocalAppData, FOLDERID_Programs, IShellLinkW, KF_FLAG_DEFAULT,
    SHGetKnownFolderPath, ShellLink,
};
use windows::core::{GUID, Interface, PCWSTR};

use super::{
    InstallScope, STUDIO_APP_ID, WindowsInstallRecord, WindowsInstallation, invalid,
    reject_redirected_path,
};

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: this handle was opened here, and is not a predefined hive handle.
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

struct Apartment(bool);
impl Apartment {
    fn enter() -> io::Result<Self> {
        // SAFETY: null reserved pointer, initialization is balanced on this thread.
        let status = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if status.is_ok() {
            return Ok(Self(true));
        }
        // An already initialized STA can also use ShellLink. Do not uninitialize
        // an apartment owned by the GUI or another caller.
        if status.0 == 0x80010106_u32 as i32 {
            return Ok(Self(false));
        }
        Err(io::Error::other(format!(
            "COM initialization failed: {status:?}"
        )))
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: successful CoInitializeEx above, on this same thread.
            unsafe {
                CoUninitialize();
            }
        }
    }
}

fn wide(value: &std::ffi::OsStr) -> io::Result<Vec<u16>> {
    let mut value: Vec<u16> = value.encode_wide().collect();
    if value.contains(&0) {
        return Err(invalid("embedded NUL in native path/value"));
    }
    value.push(0);
    Ok(value)
}

fn open_key(hive: HKEY, view: REG_SAM_FLAGS) -> io::Result<Option<Key>> {
    let path = OsString::from(format!(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{STUDIO_APP_ID}"
    ));
    let path = wide(&path)?;
    let mut handle = HKEY::default();
    // SAFETY: terminated string and valid output storage live through the call.
    let status = unsafe {
        RegOpenKeyExW(
            hive,
            PCWSTR(path.as_ptr()),
            None,
            KEY_READ | view,
            &mut handle,
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status.0 as i32));
    }
    Ok(Some(Key(handle)))
}

fn string_value(key: &Key, name: &str) -> io::Result<OsString> {
    let name = wide(std::ffi::OsStr::new(name))?;
    // One bounded buffer avoids trusting registry sizes and any retry race.
    let mut value = vec![0_u16; 32768];
    let mut bytes = (value.len() * 2) as u32;
    // SAFETY: output buffer has exactly `bytes` writable bytes; handle is live.
    let status = unsafe {
        RegGetValueW(
            key.0,
            PCWSTR::null(),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(value.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status.0 as i32));
    }
    let count = bytes as usize / 2;
    if !bytes.is_multiple_of(2)
        || count == 0
        || count > value.len()
        || value[count - 1] != 0
        || value[..count - 1].contains(&0)
    {
        return Err(invalid("invalid or truncated installation registry string"));
    }
    Ok(OsString::from_wide(&value[..count - 1]))
}

fn known_folder(id: &GUID) -> io::Result<PathBuf> {
    // SAFETY: COM initialized by caller, GUID is live; returned memory is owned
    // by this call and freed even if UTF-16 conversion fails.
    let pointer =
        unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }.map_err(io::Error::other)?;
    let result = unsafe { pointer.to_string() }
        .map(PathBuf::from)
        .map_err(io::Error::other);
    unsafe {
        CoTaskMemFree(Some(pointer.0.cast()));
    }
    result
}

fn buffer_path(value: &[u16]) -> io::Result<PathBuf> {
    let end = value
        .iter()
        .position(|&unit| unit == 0)
        .ok_or_else(|| invalid("unterminated shortcut path"))?;
    if end == 0 {
        return Err(invalid("empty shortcut target/icon"));
    }
    Ok(PathBuf::from(OsString::from_wide(&value[..end])))
}

fn verify_shortcut(path: &Path, executable: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
        Ok(metadata) if metadata.len() > 1024 * 1024 => return Err(invalid("oversized shortcut")),
        Ok(_) => {}
    }
    reject_redirected_path(path)?;
    let path = wide(path.as_os_str())?;
    // SAFETY: initialized COM, known in-process ShellLink class; all buffers are
    // bounded and live. Load read-only; never call Resolve (search/repair).
    unsafe {
        let shell: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(io::Error::other)?;
        let persist: IPersistFile = shell.cast().map_err(io::Error::other)?;
        persist
            .Load(PCWSTR(path.as_ptr()), STGM_READ)
            .map_err(io::Error::other)?;
        let mut target = vec![0_u16; 32768];
        let mut icon = vec![0_u16; 32768];
        let mut index = -1;
        shell
            .GetPath(&mut target, std::ptr::null_mut(), 0)
            .map_err(io::Error::other)?;
        shell
            .GetIconLocation(&mut icon, &mut index)
            .map_err(io::Error::other)?;
        let target = buffer_path(&target)?;
        let icon = buffer_path(&icon)?;
        reject_redirected_path(&target)?;
        reject_redirected_path(&icon)?;
        if index != 0
            || fs::canonicalize(target)? != executable
            || fs::canonicalize(icon)? != executable
        {
            return Err(invalid(
                "shortcut target or icon differs from current installation",
            ));
        }
    }
    Ok(true)
}

/// Discover only the exact running Studio installation. Any machine-scope
/// registration conflict, missing user record, unknown path or foreign shortcut
/// blocks automatic replacement. This does not authorize or execute an update.
pub fn discover_current_installation() -> io::Result<WindowsInstallation> {
    discover_installation_at(&std::env::current_exe()?, env!("CARGO_PKG_VERSION"))
}

pub(super) fn updater_private_root() -> io::Result<PathBuf> {
    let root = known_folder(&FOLDERID_LocalAppData)?.join("InstPlot Studio Updater");
    if let Err(error) = super::create_private_directory(&root)
        && super::validate_private_directory(&root).is_err()
    {
        return Err(error);
    }
    Ok(root)
}

pub(super) fn revalidate_installation(installation: &WindowsInstallation) -> io::Result<()> {
    let current = discover_installation_at(
        installation.executable(),
        &installation.version().to_string(),
    )?;
    if current.directory() != installation.directory()
        || current.desktop_shortcut() != installation.desktop_shortcut()
    {
        return Err(invalid(
            "installation identity/tasks changed since preparation",
        ));
    }
    Ok(())
}

fn discover_installation_at(executable: &Path, version: &str) -> io::Result<WindowsInstallation> {
    let _apartment = Apartment::enter()?;
    for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
        if open_key(HKEY_LOCAL_MACHINE, view)?.is_some() {
            return Err(invalid(
                "machine-scope Studio installation is not supported",
            ));
        }
    }
    let key = open_key(HKEY_CURRENT_USER, KEY_WOW64_64KEY)?
        .ok_or_else(|| invalid("no current-user Studio installation record"))?;
    let directory = PathBuf::from(string_value(&key, "InstallLocation")?);
    let registered_version = string_value(&key, "DisplayVersion")?
        .into_string()
        .map_err(|_| invalid("invalid registered version encoding"))?;
    let record = WindowsInstallRecord {
        app_id: STUDIO_APP_ID.into(),
        scope: InstallScope::CurrentUser,
        directory,
        version: registered_version,
        desktop_shortcut: false,
    };
    let mut installation = WindowsInstallation::bind(&record, executable, version)?;
    let desktop = known_folder(&FOLDERID_Desktop)?.join("InstPlot Studio.lnk");
    let menu = known_folder(&FOLDERID_Programs)?.join("InstPlot Studio.lnk");
    installation.desktop_shortcut = verify_shortcut(&desktop, installation.executable())?;
    if !verify_shortcut(&menu, installation.executable())? {
        return Err(invalid(
            "registered installation has no verified Start Menu shortcut",
        ));
    }
    Ok(installation)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wide_paths_preserve_unicode_and_reject_nul() {
        assert_eq!(
            wide(std::ffi::OsStr::new("数据")).unwrap(),
            vec![0x6570, 0x636e, 0]
        );
        assert!(wide(std::ffi::OsStr::new("bad\0path")).is_err());
        assert!(buffer_path(&[1, 2]).is_err());
        assert!(buffer_path(&[0]).is_err());
    }
}

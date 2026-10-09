//! Private Windows transaction directories, created with an owner-only DACL.
//! Existing foreign/default directories are rejected, never chmod/ACL repaired.
use std::io;
use std::io::Write;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
    GetNamedSecurityInfoW, SDDL_REVISION_1, SE_FILE_OBJECT,
};
use windows::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION, EqualSid, GetAce,
    GetSecurityDescriptorControl, GetTokenInformation, OWNER_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, PSID, SE_DACL_PROTECTED, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    TokenUser,
};
use windows::Win32::Storage::FileSystem::{
    CREATE_NEW, CreateDirectoryW, CreateFileW, FILE_ALL_ACCESS, FILE_ATTRIBUTE_NORMAL,
    FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{PCWSTR, PWSTR};

use super::reject_redirected_path;

struct LocalMemory(*mut core::ffi::c_void);
impl Drop for LocalMemory {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: exclusively owns memory allocated by the security APIs.
            unsafe {
                let _ = LocalFree(Some(HLOCAL(self.0)));
            }
        }
    }
}
struct Token(HANDLE);
impl Drop for Token {
    fn drop(&mut self) {
        // SAFETY: exclusively owns a successful OpenProcessToken handle.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct CurrentUser(Vec<usize>);
impl CurrentUser {
    fn read() -> io::Result<Self> {
        let mut token = HANDLE::default();
        // SAFETY: valid process pseudo-handle, query only, writable handle output.
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.map_err(error)?;
        let token = Token(token);
        let mut required = 0;
        // SAFETY: documented size query; failure is expected for zero capacity.
        let _ = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &mut required) };
        if required < size_of::<TOKEN_USER>() as u32 || required > 4096 {
            return Err(io::Error::other("invalid token user information length"));
        }
        // usize storage gives TOKEN_USER pointer alignment, unlike a Vec<u8>.
        let mut words = vec![0_usize; (required as usize).div_ceil(size_of::<usize>())];
        let capacity = words.len() * size_of::<usize>();
        // SAFETY: aligned initialized allocation of capacity bytes, exact output size.
        unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                Some(words.as_mut_ptr().cast()),
                capacity as u32,
                &mut required,
            )
        }
        .map_err(error)?;
        if required < size_of::<TOKEN_USER>() as u32 || required as usize > capacity {
            return Err(io::Error::other(
                "token user information exceeded allocation",
            ));
        }
        let user = Self(words);
        let sid = user.sid().0 as usize;
        let start = user.0.as_ptr() as usize;
        if sid < start || sid >= start + required as usize {
            return Err(io::Error::other("token user SID is outside its allocation"));
        }
        Ok(user)
    }
    fn sid(&self) -> PSID {
        // SAFETY: aligned, bounded TOKEN_USER returned by GetTokenInformation;
        // its allocation is retained without resizing for this object's lifetime.
        unsafe { (*self.0.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }
    fn text(&self) -> io::Result<String> {
        let mut text = PWSTR::null();
        // SAFETY: valid SID retained above; API returns LocalAlloc-owned UTF-16.
        unsafe { ConvertSidToStringSidW(self.sid(), &mut text) }.map_err(error)?;
        let _owned = LocalMemory(text.0.cast());
        // SAFETY: successful native conversion returns a NUL-terminated SID string.
        unsafe { text.to_string() }.map_err(error)
    }
}

fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let mut value: Vec<u16> = path.as_os_str().encode_wide().collect();
    if value.contains(&0) || value.len() >= 32767 {
        return Err(io::Error::other("invalid private directory path"));
    }
    value.push(0);
    Ok(value)
}
fn error(value: impl std::fmt::Display) -> io::Error {
    io::Error::other(value.to_string())
}

/// Parent must already exist. Do not recursively change parent permissions or
/// repair an existing directory; only create one explicitly private object.
pub fn create_private_directory(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| error("private directory has no parent"))?;
    reject_redirected_path(parent)?;
    if path.exists() {
        return validate_private_directory(path);
    }
    let user = CurrentUser::read()?;
    let sddl: Vec<u16> = format!("O:{}D:P(A;OICI;FA;;;{})", user.text()?, user.text()?)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: NUL-terminated SDDL, valid revision, native allocation output.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl.as_ptr()),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
    }
    .map_err(error)?;
    let _owned = LocalMemory(descriptor.0);
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    let name = wide(path)?;
    // SAFETY: descriptor is alive for this call; object gets its ACL at creation.
    unsafe { CreateDirectoryW(PCWSTR(name.as_ptr()), Some(&attributes)) }.map_err(error)?;
    validate_private_directory(path)
}

/// Create a fresh file with explicit owner and DACL (not token-default owner,
/// which can be an administrator group). Existing files are never repaired.
pub fn create_private_file(path: &Path) -> io::Result<std::fs::File> {
    validate_private_directory(
        path.parent()
            .ok_or_else(|| error("private file has no parent"))?,
    )?;
    let user = CurrentUser::read()?;
    let sid = user.text()?;
    let sddl: Vec<u16> = format!("O:{sid}D:P(A;;FA;;;{sid})")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: valid terminated SDDL and native writable allocation output.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl.as_ptr()),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
    }
    .map_err(error)?;
    let _owned = LocalMemory(descriptor.0);
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    let name = wide(path)?;
    // SAFETY: CREATE_NEW does not open/overwrite an existing link or file;
    // explicit descriptor remains alive during atomic native creation.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(name.as_ptr()),
            FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            Some(&attributes),
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .map_err(error)?;
    use std::os::windows::io::FromRawHandle;
    // SAFETY: successful handle ownership moves exactly once into File.
    let file = unsafe { std::fs::File::from_raw_handle(handle.0) };
    validate_private_file(path)?;
    Ok(file)
}

/// Durable bounded metadata replacement in one private directory. On failure
/// retain this transaction's temporary evidence, never overwrite foreign ACLs.
pub fn write_private_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_private_atomic_impl(path, bytes, true)
}

/// Publish a complete immutable signal; an existing target is never replaced.
pub(super) fn write_private_atomic_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_private_atomic_impl(path, bytes, false)
}

fn write_private_atomic_impl(path: &Path, bytes: &[u8], replace: bool) -> io::Result<()> {
    if bytes.len() > 256 * 1024 {
        return Err(error("private update metadata is too large"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| error("private metadata has no parent"))?;
    validate_private_directory(parent)?;
    match std::fs::symlink_metadata(path) {
        Ok(_) => validate_private_file(path)?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let mut random = [0; 16];
    getrandom::fill(&mut random).map_err(error)?;
    let temporary = parent.join(format!(".metadata-{:x}.tmp", u128::from_le_bytes(random)));
    let mut file = create_private_file(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    let from = wide(&temporary)?;
    let to = wide(path)?;
    // SAFETY: same-directory private paths, fresh owned temporary and previously
    // checked target. No cross-volume copying or permission/ownership changes.
    unsafe {
        MoveFileExW(
            PCWSTR(from.as_ptr()),
            PCWSTR(to.as_ptr()),
            if replace {
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH
            } else {
                MOVEFILE_WRITE_THROUGH
            },
        )
    }
    .map_err(error)?;
    validate_private_file(path)
}

/// Strict policy for updater-owned roots: current user owns the directory,
/// protected DACL, exactly one inheritable full-access grant to that user.
pub fn validate_private_directory(path: &Path) -> io::Result<()> {
    validate_private_object(path, true)
}

/// Cache evidence files must also be owned/granted only to this user; a
/// private parent is not sufficient for a file brought in with a foreign ACL.
pub fn validate_private_file(path: &Path) -> io::Result<()> {
    validate_private_object(path, false)
}

fn validate_private_object(path: &Path, directory: bool) -> io::Result<()> {
    reject_redirected_path(path)?;
    if (directory && !path.is_dir()) || (!directory && !path.is_file()) {
        return Err(error("private transaction path has the wrong object type"));
    }
    let user = CurrentUser::read()?;
    let name = wide(path)?;
    let mut owner = PSID::default();
    let mut acl: *mut ACL = std::ptr::null_mut();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: named real directory and valid writable native output pointers.
    let status = unsafe {
        GetNamedSecurityInfoW(
            PCWSTR(name.as_ptr()),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            Some(&mut owner),
            None,
            Some(&mut acl),
            None,
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status.0 as i32));
    }
    let _owned = LocalMemory(descriptor.0);
    if owner.0.is_null() || acl.is_null() || descriptor.0.is_null() {
        return Err(error("private directory lacks owner or DACL"));
    }
    // SAFETY: pointers are inside the retained native descriptor/current token.
    unsafe { EqualSid(owner, user.sid()) }
        .map_err(|_| error("private directory belongs to another user"))?;
    let mut control = 0;
    let mut revision = 0;
    // SAFETY: native descriptor retained above and initialized writable outputs.
    unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) }
        .map_err(error)?;
    // SAFETY: ACL is a validated native security descriptor's DACL.
    if (directory && control & SE_DACL_PROTECTED.0 == 0) || unsafe { (*acl).AceCount } != 1 {
        return Err(error("private directory DACL is not protected owner-only"));
    }
    let mut ace = std::ptr::null_mut();
    // SAFETY: one ACE was checked above, index zero and valid output pointer.
    unsafe { GetAce(acl, 0, &mut ace) }.map_err(error)?;
    if ace.is_null() {
        return Err(error("private directory ACE is missing"));
    }
    // SAFETY: GetAce returned a native ACE with a common header. Check its
    // type and size before creating a reference to a larger typed ACE.
    let header = unsafe { &*ace.cast::<ACE_HEADER>() };
    if header.AceType != 0
        || usize::from(header.AceSize) < size_of::<ACCESS_ALLOWED_ACE>()
        || (directory && header.AceFlags != 3)
        || (!directory && !matches!(header.AceFlags, 0 | 16))
    {
        return Err(error(
            "private directory grant does not match the updater policy",
        ));
    }
    // SAFETY: ACCESS_ALLOWED_ACE type and minimum size checked above.
    let ace = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
    if ace.Mask != FILE_ALL_ACCESS.0 {
        return Err(error("private directory grant is not full owner access"));
    }
    let sid = PSID(std::ptr::addr_of!(ace.SidStart).cast_mut().cast());
    // SAFETY: this is a validated ACCESS_ALLOWED_ACE's SID, descriptor retained.
    unsafe { EqualSid(sid, user.sid()) }
        .map_err(|_| error("private directory grants another user access"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_directory_is_created_atomically_and_foreign_acl_is_not_repaired() {
        let mut random = [0; 8];
        getrandom::fill(&mut random).unwrap();
        let root = std::env::temp_dir().join(format!(
            "studio-private-cache-{:x}",
            u64::from_le_bytes(random)
        ));
        std::fs::create_dir(&root).unwrap();
        let private = root.join("private 数据");
        create_private_directory(&private).unwrap();
        let evidence = private.join("manifest.json");
        create_private_file(&evidence)
            .unwrap()
            .write_all(b"fixture evidence")
            .unwrap();
        validate_private_file(&evidence).unwrap();
        assert!(validate_private_file(&private).is_err());
        assert!(validate_private_directory(&evidence).is_err());
        assert!(create_private_file(&evidence).is_err());
        write_private_atomic(&evidence, b"new fixture evidence").unwrap();
        assert_eq!(std::fs::read(&evidence).unwrap(), b"new fixture evidence");
        validate_private_directory(&private).unwrap();
        create_private_directory(&private).unwrap();
        assert!(validate_private_directory(&root).is_err());
        assert!(create_private_directory(&root).is_err());
        assert!(validate_private_directory(&root.join("missing")).is_err());
        std::fs::remove_file(&evidence).unwrap();
        std::fs::remove_dir(&private).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }
}

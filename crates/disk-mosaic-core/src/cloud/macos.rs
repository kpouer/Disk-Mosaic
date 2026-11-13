use std::ffi::CStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path};

/// FUSE filesystem types backing cloud sync clients (rclone, Google Drive
/// OCamlFuse and similar tools). Plain network shares (smbfs, webdav) are not
/// cloud mounts and stay scannable, like on the other targets.
const CLOUD_FS_TYPES: &[&str] = &["fuse", "macfuse", "osxfuse"];

pub(super) fn is_cloud_path(path: &Path) -> bool {
    is_provider_managed_folder(path) || is_cloud_filesystem(path)
}

/// Folders managed by macOS File Provider (`~/Library/CloudStorage`, where
/// Dropbox, OneDrive and Google Drive install nowadays) or iCloud Drive.
fn is_provider_managed_folder(path: &Path) -> bool {
    let Some(home) = super::home_dir() else {
        return false;
    };
    let Ok(stripped) = path.strip_prefix(home) else {
        return false;
    };
    let mut components = stripped.components();
    if !matches!(components.next(), Some(Component::Normal(name)) if name == "Library") {
        return false;
    }
    match components.next() {
        Some(Component::Normal(name)) if name == "CloudStorage" => true,
        Some(Component::Normal(name)) if name == "Mobile Documents" => matches!(
            components.next(),
            Some(Component::Normal(name)) if name == "com~apple~CloudDocs"
        ),
        _ => false,
    }
}

/// Whether the filesystem hosting the path is a FUSE or remote mount, which is
/// how rclone, Google Drive OCamlFuse and similar tools expose cloud content.
fn is_cloud_filesystem(path: &Path) -> bool {
    let Ok(c_path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: c_path is a valid nul terminated path and statfs only writes
    // into the provided buffer.
    let fstype = unsafe {
        let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
        if libc::statfs(c_path.as_ptr(), stat.as_mut_ptr()) != 0 {
            return false;
        }
        let stat = stat.assume_init();
        CStr::from_ptr(stat.f_fstypename.as_ptr())
    };
    let Ok(fstype) = fstype.to_str() else {
        return false;
    };
    CLOUD_FS_TYPES.contains(&fstype)
}

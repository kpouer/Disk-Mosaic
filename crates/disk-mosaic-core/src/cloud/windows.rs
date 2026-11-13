use std::os::windows::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS, FILE_ATTRIBUTE_RECALL_ON_OPEN, GetDriveTypeW,
    GetFileAttributesW, INVALID_FILE_ATTRIBUTES,
};
use windows_sys::Win32::System::WindowsProgramming::DRIVE_REMOTE;

/// Attributes carried by cloud placeholder folders (OneDrive, Google Drive).
const CLOUD_ATTRIBUTES: u32 = FILE_ATTRIBUTE_RECALL_ON_OPEN | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS;

pub(super) fn is_cloud_path(path: &Path) -> bool {
    is_known_cloud_root(path) || is_remote_drive(path) || has_cloud_attributes(path)
}

/// OneDrive publishes its sync roots through environment variables, they also
/// cover custom install locations the name based fallback would miss.
fn known_cloud_roots() -> &'static [PathBuf] {
    static CLOUD_ROOTS: OnceLock<Vec<PathBuf>> = OnceLock::new();
    CLOUD_ROOTS.get_or_init(|| {
        ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"]
            .iter()
            .filter_map(|name| std::env::var_os(name))
            .map(PathBuf::from)
            .collect()
    })
}

fn is_known_cloud_root(path: &Path) -> bool {
    has_cloud_root(path, known_cloud_roots())
}

fn has_cloud_root(path: &Path, roots: &[PathBuf]) -> bool {
    roots
        .iter()
        .any(|root| !root.as_os_str().is_empty() && path.starts_with(root))
}

/// Mapped network drives and rclone mounts report as remote drives.
fn is_remote_drive(path: &Path) -> bool {
    let Some(root) = drive_root(path) else {
        return false;
    };
    // SAFETY: root is a nul terminated wide string.
    unsafe { GetDriveTypeW(root.as_ptr()) == DRIVE_REMOTE }
}

/// Returns the root of the drive hosting the path (`C:\` or `\\server\share\`).
fn drive_root(path: &Path) -> Option<Vec<u16>> {
    let Component::Prefix(prefix) = path.components().next()? else {
        return None;
    };
    let mut root: Vec<u16> = prefix.as_os_str().encode_wide().collect();
    if !root.ends_with(&[b'\\' as u16]) {
        root.push(b'\\' as u16);
    }
    root.push(0);
    Some(root)
}

fn has_cloud_attributes(path: &Path) -> bool {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: wide is a nul terminated string, GetFileAttributesW only reads it.
    let attributes = unsafe { GetFileAttributesW(wide.as_ptr()) };
    attributes != INVALID_FILE_ATTRIBUTES && attributes & CLOUD_ATTRIBUTES != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide_root(value: &str) -> Option<Vec<u16>> {
        drive_root(Path::new(value))
    }

    #[test]
    fn builds_drive_roots() {
        assert_eq!(
            wide_root(r"C:\Users\me\Documents"),
            Some(vec![u16::from(b'C'), u16::from(b':'), u16::from(b'\\'), 0])
        );
        assert_eq!(
            wide_root(r"\\server\share\folder"),
            Some(
                r"\\server\share\"
                    .encode_utf16()
                    .chain(Some(0))
                    .collect::<Vec<u16>>()
            )
        );
        assert_eq!(wide_root(r"relative\folder"), None);
    }

    #[test]
    fn matches_only_paths_under_a_cloud_root() {
        let roots = vec![PathBuf::from(r"C:\Users\me\OneDrive")];
        assert!(has_cloud_root(
            Path::new(r"C:\Users\me\OneDrive\Documents"),
            &roots
        ));
        assert!(!has_cloud_root(Path::new(r"C:\Users\me\Documents"), &roots));
        // An empty root would otherwise match every path.
        assert!(!has_cloud_root(
            Path::new(r"C:\Users\me\Documents"),
            &[PathBuf::new()]
        ));
    }
}

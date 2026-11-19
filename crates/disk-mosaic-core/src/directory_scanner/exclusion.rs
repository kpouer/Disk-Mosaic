use std::path::Path;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(crate) fn is_excluded(_path: &Path) -> bool {
    false
}

#[cfg(target_os = "macos")]
pub(crate) fn is_excluded(path: &Path) -> bool {
    path.starts_with("/System/Volumes")
}

#[cfg(target_os = "linux")]
pub(crate) fn is_excluded(path: &Path) -> bool {
    path.starts_with("/proc")
}

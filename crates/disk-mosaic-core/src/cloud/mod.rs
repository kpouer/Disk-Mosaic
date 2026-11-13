//! Detection of cloud storage locations.
//!
//! Detection is target specific: each OS applies its own strategy (filesystem
//! type, mount table, cloud file attributes, provider managed folders) and a
//! shared name based fallback is applied last for sync clients that leave no
//! filesystem signature.

use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// Top level folders under HOME commonly created by cloud sync clients.
const CLOUD_HOME_FOLDERS: &[&str] = &[
    "Dropbox",
    "OneDrive",
    "OneDrive - Personal",
    "Google Drive",
    "Google Drive (Shared)",
    "Box",
    "Nextcloud",
    "SynologyDrive",
    "pCloud Drive",
    "MEGA",
];

/// Returns true if the path is a common cloud storage location
/// (Dropbox, OneDrive, Google Drive, iCloud, rclone mount, etc.)
pub fn is_common_cloud_path(path: &Path) -> bool {
    is_cloud_home_folder(path) || is_platform_cloud_path(path)
}

/// Fallback: the path starts with a well known sync client folder under HOME.
fn is_cloud_home_folder(path: &Path) -> bool {
    let Some(home) = home_dir() else {
        return false;
    };
    let Ok(stripped) = path.strip_prefix(home) else {
        return false;
    };
    let Some(Component::Normal(name)) = stripped.components().next() else {
        return false;
    };
    name.to_str()
        .is_some_and(|name| CLOUD_HOME_FOLDERS.contains(&name))
}

#[cfg(target_os = "macos")]
fn is_platform_cloud_path(path: &Path) -> bool {
    macos::is_cloud_path(path)
}

#[cfg(target_os = "linux")]
fn is_platform_cloud_path(path: &Path) -> bool {
    linux::is_cloud_path(path)
}

#[cfg(windows)]
fn is_platform_cloud_path(path: &Path) -> bool {
    windows::is_cloud_path(path)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn is_platform_cloud_path(_path: &Path) -> bool {
    false
}

/// Cached home directory, the scan calls the detection once per directory.
fn home_dir() -> Option<&'static Path> {
    static HOME: OnceLock<Option<PathBuf>> = OnceLock::new();
    HOME.get_or_init(home::home_dir).as_deref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_known_cloud_folder_under_home() {
        let Some(home) = home_dir() else {
            return;
        };
        assert!(is_common_cloud_path(&home.join("Dropbox")));
        assert!(is_common_cloud_path(
            &home.join("OneDrive").join("Documents")
        ));
        assert!(is_common_cloud_path(
            &home.join("Google Drive").join("photos")
        ));
    }

    #[test]
    fn ignores_regular_home_folder() {
        let Some(home) = home_dir() else {
            return;
        };
        assert!(!is_common_cloud_path(&home.join("Documents")));
        assert!(!is_common_cloud_path(&home.join("Desktop")));
    }

    #[test]
    fn ignores_unknown_path() {
        assert!(!is_common_cloud_path(Path::new(
            "/definitely/not/a/cloud/location"
        )));
    }
}

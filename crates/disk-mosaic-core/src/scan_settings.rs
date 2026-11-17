use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const BIG_FILE_THRESHOLD: u64 = 10000000;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanSettings {
    /// List of paths to ignore (might be cloud drives, etc.
    #[serde(default)]
    pub ignored_path: Vec<PathBuf>,
    /// Ignore common cloud folders like Dropbox, OneDrive, Google Drive, iCloud, etc.
    #[serde(default = "default_ignore_cloud_mounts")]
    pub ignore_cloud_mounts: bool,
    /// Threshold for big files (in bytes). Files smaller than this will be displayed as a single block.
    #[serde(default = "default_big_file_threshold")]
    pub big_file_threshold: u64,
}

impl Default for ScanSettings {
    fn default() -> Self {
        Self {
            ignored_path: Vec::new(),
            ignore_cloud_mounts: default_ignore_cloud_mounts(),
            big_file_threshold: default_big_file_threshold(),
        }
    }
}

impl ScanSettings {
    /// Returns true if `path` is exactly one of the explicitly ignored paths.
    pub fn is_explicitly_ignored(&self, path: &Path) -> bool {
        self.ignored_path.iter().any(|ignored| ignored == path)
    }
}

const fn default_ignore_cloud_mounts() -> bool {
    true
}

const fn default_big_file_threshold() -> u64 {
    BIG_FILE_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicitly_ignored_matches_exactly() {
        let s = ScanSettings {
            ignored_path: vec![PathBuf::from("/a/b")],
            ..ScanSettings::default()
        };
        assert!(s.is_explicitly_ignored(Path::new("/a/b")));
        assert!(!s.is_explicitly_ignored(Path::new("/a")));
        assert!(!s.is_explicitly_ignored(Path::new("/a/b/c")));
    }

    #[test]
    fn missing_ignored_path_defaults_to_empty() {
        let s: ScanSettings = serde_json::from_str("{}").unwrap();
        assert!(s.ignored_path.is_empty());
        assert!(s.ignore_cloud_mounts);
        assert_eq!(s.big_file_threshold, BIG_FILE_THRESHOLD);
    }
}

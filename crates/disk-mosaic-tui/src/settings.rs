use disk_mosaic_core::directory_scanner::{BIG_FILE_THRESHOLD, ScanConfig};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Default, Clone)]
pub struct Settings {
    /// List of paths to ignore (might be cloud drives, etc.
    pub ignored_path: Arc<Vec<PathBuf>>,
}

impl ScanConfig for Settings {
    fn big_file_threshold(&self) -> u64 {
        BIG_FILE_THRESHOLD
    }

    fn is_path_ignored(&self, path: &Path) -> bool {
        if self
            .ignored_path
            .iter()
            .any(|ignored_path| ignored_path == path)
        {
            return true;
        }
        false
    }
}

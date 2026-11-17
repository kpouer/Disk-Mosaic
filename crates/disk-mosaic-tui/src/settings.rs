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
        self.ignored_path
            .iter()
            .any(|ignored_path| ignored_path == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ignores_nothing() {
        let settings = Settings::default();
        assert!(settings.ignored_path.is_empty());
        assert!(!settings.is_path_ignored(Path::new("/some/path")));
    }

    #[test]
    fn big_file_threshold_is_default() {
        assert_eq!(Settings::default().big_file_threshold(), BIG_FILE_THRESHOLD);
    }

    #[test]
    fn ignored_path_matches_exactly() {
        let settings = Settings {
            ignored_path: Arc::new(vec![PathBuf::from("/a/b")]),
        };
        assert!(settings.is_path_ignored(Path::new("/a/b")));
        assert!(!settings.is_path_ignored(Path::new("/a")));
        assert!(!settings.is_path_ignored(Path::new("/a/b/c")));
    }

    #[test]
    fn clone_shares_ignored_paths() {
        let settings = Settings {
            ignored_path: Arc::new(vec![PathBuf::from("/x")]),
        };
        let cloned = settings.clone();
        assert!(Arc::ptr_eq(&settings.ignored_path, &cloned.ignored_path));
    }
}

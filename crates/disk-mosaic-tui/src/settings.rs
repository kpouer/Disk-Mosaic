use disk_mosaic_core::directory_scanner::ScanConfig;
use disk_mosaic_core::scan_settings::ScanSettings;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Settings {
    pub scan_settings: ScanSettings,
}

impl Settings {
    /// Builds settings ignoring the given paths. Cloud folders are not filtered in the TUI.
    pub fn new(ignored_path: Vec<PathBuf>) -> Self {
        Self {
            scan_settings: ScanSettings {
                ignored_path,
                ignore_cloud_mounts: false,
                ..ScanSettings::default()
            },
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl ScanConfig for Settings {
    fn big_file_threshold(&self) -> u64 {
        self.scan_settings.big_file_threshold
    }

    fn is_path_ignored(&self, path: &Path) -> bool {
        self.scan_settings.is_explicitly_ignored(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use disk_mosaic_core::scan_settings::BIG_FILE_THRESHOLD;

    #[test]
    fn default_ignores_nothing() {
        let settings = Settings::default();
        assert!(settings.scan_settings.ignored_path.is_empty());
        assert!(!settings.is_path_ignored(Path::new("/some/path")));
    }

    #[test]
    fn big_file_threshold_is_default() {
        assert_eq!(Settings::default().big_file_threshold(), BIG_FILE_THRESHOLD);
    }

    #[test]
    fn ignored_path_matches_exactly() {
        let settings = Settings::new(vec![PathBuf::from("/a/b")]);
        assert!(settings.is_path_ignored(Path::new("/a/b")));
        assert!(!settings.is_path_ignored(Path::new("/a")));
        assert!(!settings.is_path_ignored(Path::new("/a/b/c")));
    }
}

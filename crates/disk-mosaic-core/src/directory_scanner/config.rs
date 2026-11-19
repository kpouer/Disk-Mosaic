use std::path::Path;

pub trait ScanConfig: Sync {
    fn big_file_threshold(&self) -> u64;
    fn is_path_ignored(&self, path: &Path) -> bool;
    #[cfg(unix)]
    fn ignore_different_device(&self) -> bool;
}

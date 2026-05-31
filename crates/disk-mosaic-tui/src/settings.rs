use disk_mosaic_core::directory_scanner::{BIG_FILE_THRESHOLD, ScanConfig};
use std::path::Path;

#[derive(Debug, Default)]
pub(crate) struct Settings;

impl ScanConfig for Settings {
    fn big_file_threshold(&self) -> u64 {
        BIG_FILE_THRESHOLD
    }

    fn is_path_ignored(&self, _path: &Path) -> bool {
        // todo implement something
        false
    }
}

use std::path::Path;
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

pub(crate) const FONT_SIZE: f32 = 18.0;

#[derive(Error, Debug)]
pub(crate) enum MyError {
    #[error("IO Error")]
    IOError(#[from] std::io::Error),
    #[error("Receiver Dropped")]
    ReceiverDropped,
}

/// Return the on-disk size of a file and ignore sparse files (return 0 for them).
pub fn get_file_size(path: &Path) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = path.metadata() {
            // `blocks()` is a cross-Unix accessor; it returns 512-byte block counts.
            let physical = meta.blocks().saturating_mul(512);
            let logical = meta.len();
            // If the physical size is smaller than the logical size, and we're on a filesystem
            // that represents holes (sparse), ignore this file by returning 0.
            if physical < logical {
                return 0;
            }
            return physical;
        }
        0
    }
    #[cfg(not(unix))]
    {
        // On non-Unix targets, fall back to logical file size.
        // Detecting sparse files portably requires platform-specific APIs which we avoid here.
        path.metadata().map(|m| m.len()).unwrap_or(0)
    }
}

pub(crate) trait PathBufToString {
    fn name(&self) -> String;
    fn absolute_path(&self) -> String;
}

impl PathBufToString for Path {
    fn name(&self) -> String {
        self.file_name()
            .and_then(|f| f.to_str())
            .map(|f| f.nfc().collect::<String>())
            .unwrap_or_default()
    }

    fn absolute_path(&self) -> String {
        self.as_os_str()
            .to_str()
            .map(|f| f.nfc().collect::<String>())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_name() {
        let path = PathBuf::from("test.txt");
        assert_eq!(path.name(), "test.txt");
    }

    #[test]
    fn test_absolute_path() {
        let path = PathBuf::from("/home/user/test.txt");
        assert_eq!(path.absolute_path(), "/home/user/test.txt");
    }

    #[test]
    fn test_name_with_unicode() {
        let path = PathBuf::from("tést.txt");
        assert_eq!(path.name(), "tést.txt");
    }

    #[test]
    fn test_absolute_path_with_unicode() {
        let path = PathBuf::from("/home/user/tést.txt");
        assert_eq!(path.absolute_path(), "/home/user/tést.txt");
    }
}

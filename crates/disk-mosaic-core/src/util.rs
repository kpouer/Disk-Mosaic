use std::fs::Metadata;
use std::path::Path;
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

#[derive(Error, Debug)]
pub(crate) enum MyError {
    #[error("IO Error")]
    IOError(#[from] std::io::Error),
    #[error("Receiver Dropped")]
    ReceiverDropped,
}

/// Return the space a file actually occupies on disk.
///
/// On Unix this is the allocated size, so sparse files (holes) and compressed files
/// (APFS, ZFS, btrfs) are reported by what they really use, not by their logical length.
/// On other targets it falls back to the logical file size.
pub(crate) fn get_file_size(metadata: &Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // `blocks()` is a cross-Unix accessor; it returns 512-byte block counts.
        metadata.blocks().saturating_mul(512)
    }
    #[cfg(not(unix))]
    {
        // Querying the allocated size on non-Unix targets requires platform-specific APIs
        // which we avoid here.
        metadata.len()
    }
}

pub trait PathBufToString {
    fn name(&self) -> String;
    fn absolute_path(&self) -> String;
}

impl PathBufToString for Path {
    fn name(&self) -> String {
        self.file_name()
            .map(|f| f.to_string_lossy().nfc().collect::<String>())
            .unwrap_or_default()
    }

    fn absolute_path(&self) -> String {
        self.as_os_str().to_string_lossy().nfc().collect::<String>()
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

    #[cfg(unix)]
    #[test]
    fn test_non_utf8_names_are_not_empty() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let path = PathBuf::from(OsStr::from_bytes(b"/home/user/bad\xffname.txt"));
        assert_eq!(path.name(), "bad\u{FFFD}name.txt");
        assert_eq!(path.absolute_path(), "/home/user/bad\u{FFFD}name.txt");
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

use std::fs::Metadata;
use std::path::Path;
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

#[derive(Error, Debug)]
pub(crate) enum MyError {
    #[error("Receiver Dropped")]
    ReceiverDropped,
}

/// Return the space a file actually occupies on disk.
///
/// On Unix this is the allocated size, so sparse files (holes) and compressed files
/// (APFS, ZFS, btrfs) are reported by what they really use, not by their logical length.
#[cfg(unix)]
pub(crate) fn get_file_size(path: &Path, metadata: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    let _ = path;
    // `blocks()` is a cross-Unix accessor; it returns 512-byte block counts.
    metadata.blocks().saturating_mul(512)
}

/// Return the space a file actually occupies on disk.
///
/// On Windows this is the size reported by `GetCompressedFileSizeW`, which accounts for NTFS
/// compression and sparse files, and is 0 for dehydrated cloud placeholders (OneDrive, ...).
/// If that call fails, it falls back to the logical file size.
#[cfg(windows)]
fn get_file_size(path: &Path, metadata: &Metadata) -> u64 {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{GetLastError, NO_ERROR};
    use windows_sys::Win32::Storage::FileSystem::{GetCompressedFileSizeW, INVALID_FILE_SIZE};

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut high: u32 = 0;
    // SAFETY: `wide` is a NUL-terminated UTF-16 string that outlives the call, and `high` is a
    // valid pointer to a u32.
    let low = unsafe { GetCompressedFileSizeW(wide.as_ptr(), &mut high) };
    // `INVALID_FILE_SIZE` is also a legitimate low word, so check the last error to tell.
    if low == INVALID_FILE_SIZE && unsafe { GetLastError() } != NO_ERROR {
        return metadata.len();
    }
    (u64::from(high) << 32) | u64::from(low)
}

/// Return the space a file actually occupies on disk.
#[cfg(not(any(unix, windows)))]
pub(crate) fn get_file_size(path: &Path, metadata: &Metadata) -> u64 {
    let _ = path;
    metadata.len()
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

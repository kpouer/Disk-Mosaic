use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Filesystem types used by cloud sync clients exposing a virtual filesystem.
const CLOUD_FS_TYPES: &[&str] = &[
    "fuse.rclone",
    "fuse.gdrivefs",
    "fuse.google-drive-ocamlfuse",
    "fuse.onedrive",
    "fuse.dropbox",
    "fuse.pcloudfs",
    "fuse.megasync",
    "fuse.filen",
];

pub(super) fn is_cloud_path(path: &Path) -> bool {
    cloud_mounts()
        .iter()
        .any(|mount_point| path.starts_with(mount_point))
}

/// Mount points of cloud filesystems, read once from the mount table.
fn cloud_mounts() -> &'static [PathBuf] {
    static CLOUD_MOUNTS: OnceLock<Vec<PathBuf>> = OnceLock::new();
    CLOUD_MOUNTS.get_or_init(|| {
        std::fs::read_to_string("/proc/self/mounts")
            .map(|content| parse_cloud_mounts(&content))
            .unwrap_or_default()
    })
}

fn parse_cloud_mounts(mount_table: &str) -> Vec<PathBuf> {
    mount_table
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let _device = fields.next()?;
            let mount_point = fields.next()?;
            let fstype = fields.next()?;
            if !CLOUD_FS_TYPES.contains(&fstype) {
                return None;
            }
            Some(PathBuf::from(unescape_mount_field(mount_point)))
        })
        .collect()
}

/// `/proc/self/mounts` escapes space, tab, newline and backslash as octal codes.
fn unescape_mount_field(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 3 < bytes.len()
            && let Ok(escaped) = u8::from_str_radix(&field[i + 1..i + 4], 8)
        {
            out.push(escaped);
            i += 4;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn only_keeps_cloud_mounts() {
        let mount_table = "\
/dev/nvme0n1p2 / ext4 rw,relatime 0 0\n\
/dev/sda1 /mnt/usb exfat rw 0 0\n\
rclone-gdrive: /mnt/gdrive fuse.rclone rw,nosuid,nodev 0 0\n\
GoogleDrive: /home/user/Google\\040Drive fuse.google-drive-ocamlfuse rw 0 0\n";

        assert_eq!(
            parse_cloud_mounts(mount_table),
            vec![
                PathBuf::from("/mnt/gdrive"),
                PathBuf::from("/home/user/Google Drive"),
            ]
        );
    }

    #[test]
    fn ignores_malformed_lines() {
        assert!(parse_cloud_mounts("not a mount table\n\n").is_empty());
    }
}

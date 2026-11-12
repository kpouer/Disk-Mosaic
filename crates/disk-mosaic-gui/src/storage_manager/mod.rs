use crate::storage_manager::storage::Storage;
use sysinfo::Disks;

pub(crate) mod storage;

#[derive(Debug)]
pub(crate) struct StorageManager {
    storages: Vec<Storage>,
}

impl StorageManager {
    pub(crate) fn new() -> Result<Self, sysinfo::Error> {
        let disks = if cfg!(target_os = "macos") {
            Disks::new_with_refreshed_list()?
                .into_iter()
                .filter(|d| d.mount_point().to_string_lossy() != "/System/Volumes/Data")
                .map(Storage::from)
                .collect()
        } else {
            Disks::new_with_refreshed_list()?
                .into_iter()
                .map(Storage::from)
                .collect()
        };
        Ok(Self { storages: disks })
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Storage> {
        self.storages.iter()
    }
}

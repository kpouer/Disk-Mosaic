use crate::service::storage_manager::storage::Storage;
use sysinfo::Disks;

pub(crate) mod storage;

#[derive(Debug)]
pub(crate) struct StorageManager {
    storages: Vec<Storage>,
}

impl Default for StorageManager {
    fn default() -> Self {
        let disks = if cfg!(target_os = "macos") {
            Disks::new_with_refreshed_list()
                .iter()
                .filter(|d| d.mount_point().to_string_lossy() != "/System/Volumes/Data")
                .map(Storage::from)
                .collect()
        } else {
            Disks::new_with_refreshed_list()
                .iter()
                .map(Storage::from)
                .collect()
        };
        Self { storages: disks }
    }
}

impl StorageManager {
    pub(crate) fn is_empty(&self) -> bool {
        self.storages.is_empty()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Storage> {
        self.storages.iter()
    }
}

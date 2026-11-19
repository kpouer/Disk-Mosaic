use crate::data::{Data, Kind};
use crate::directory_scanner::DirectoryEntryIterator;
use crate::directory_scanner::config::ScanConfig;
use crate::directory_scanner::counters::CountAndSize;
use crate::directory_scanner::exclusion::is_excluded;
use crate::directory_scanner::progress::TopLevelProgress;
use crate::model::message::Message;
use crate::model::scan_result::ScanResult;
use crate::util;
use crate::util::PathBufToString;
use log::{debug, info, warn};
use rayon::iter::{IntoParallelRefIterator, ParallelBridge, ParallelIterator};
use std::fs::DirEntry;
use std::io::ErrorKind;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;

pub(crate) enum ScannedEntry {
    Dir(PathBuf),
    File(PathBuf, u64),
}

#[cfg(unix)]
type Dev = u64;
#[cfg(unix)]
type Ino = u64;
#[cfg(unix)]
type FileId = (Dev, Ino);

#[derive(Debug)]
pub struct Scanner<'a, T>
where
    T: ScanConfig,
{
    stopper: &'a Arc<AtomicBool>,
    pub(crate) sender: &'a Sender<Message>,
    settings: &'a T,
    pub(crate) progress: TopLevelProgress,
    #[cfg(unix)]
    seen_inodes: std::sync::Mutex<std::collections::HashSet<FileId>>,
    /// The device where the directory being scanned resides.
    #[cfg(unix)]
    pub(crate) device: Option<Dev>,
}

impl<'a, T> Scanner<'a, T>
where
    T: ScanConfig,
{
    // `path` is only used on unix, to record the device of the scanned directory.
    #[cfg_attr(not(unix), allow(unused_variables))]
    pub(crate) fn new(
        path: &Path,
        stopper: &'a Arc<AtomicBool>,
        sender: &'a Sender<Message>,
        settings: &'a T,
    ) -> Self {
        Self {
            stopper,
            sender,
            settings,
            progress: TopLevelProgress::default(),
            #[cfg(unix)]
            seen_inodes: std::sync::Mutex::new(std::collections::HashSet::new()),
            #[cfg(unix)]
            device: std::fs::metadata(path).map(|m| m.dev()).ok(),
        }
    }

    fn is_path_ignored(&self, path: &Path) -> bool {
        self.settings.is_path_ignored(path) || is_excluded(path)
    }

    pub(crate) fn classify(&self, entry: &DirEntry) -> Option<ScannedEntry> {
        let file_type = entry
            .file_type()
            .inspect_err(|e| warn!("Error getting file type: {e}"))
            .ok()?;

        let path = entry.path();
        let metadata = entry
            .metadata()
            .inspect_err(|e| debug!("Failed to get metadata for {path:?}: {e}"))
            .ok();

        if file_type.is_dir() {
            #[cfg(unix)]
            if metadata
                .as_ref()
                .is_some_and(|m| self.is_other_device(m.dev()))
            {
                return None;
            }
            if self.is_path_ignored(&path) {
                info!("Ignoring path: {path:?}");
                return None;
            }
            Some(ScannedEntry::Dir(path))
        } else if file_type.is_file() {
            let metadata = metadata?;
            #[cfg(unix)]
            {
                let dev = metadata.dev();
                if self.is_other_device(dev) {
                    return None;
                }
                let ino = metadata.ino();
                let nlink = metadata.nlink();
                if nlink > 1 && !self.seen_inodes.lock().unwrap().insert((dev, ino)) {
                    // already seen
                    return None;
                }
            }
            let size = util::get_file_size(&path, &metadata);
            Some(ScannedEntry::File(path, size))
        } else {
            None
        }
    }

    /// Whether `dev` differs from the scan root device while the user asked to stay on it.
    #[cfg(unix)]
    fn is_other_device(&self, dev: Dev) -> bool {
        self.settings.ignore_different_device() && self.device.is_some_and(|d| d != dev)
    }

    /// Scan a directory recursively, adding the size of the files found to `progress`.
    pub(crate) fn scan_directory_recursive(
        &self,
        path: &Path,
        progress: &AtomicU64,
    ) -> Option<Data> {
        if self.stopper.load(Ordering::Relaxed) {
            return None;
        }
        if let Err(e) = self
            .sender
            .send(Message::DirectoryScanStart(path.absolute_path()))
        {
            debug!("Received dropped {e}");
            return None;
        }
        let scanned_children = Self::entry_iterator(path)
            .map(|entries| self.collect_children(entries, progress))
            .unwrap_or_default();

        let scan_result = scanned_children
            .par_iter()
            .filter_map(|data| match data.kind {
                Kind::Dir(_) => None,
                Kind::File => Some(ScanResult::new(1, data.size)),
                Kind::SmallFiles(count) => Some(ScanResult::new(count, data.size)),
            })
            .reduce(ScanResult::default, |left, right| left + right);
        progress.fetch_add(scan_result.size, Ordering::Relaxed);
        if scan_result.file_count != 0
            && let Err(e) = self.sender.send(Message::DirectoryScanDone(scan_result))
        {
            debug!("Received dropped {e}");
        }
        let mut data = Data::new_directory(path);
        data.set_nodes(scanned_children);
        Some(data)
    }

    pub(crate) fn entry_iterator(path: &Path) -> Option<DirectoryEntryIterator> {
        match path.read_dir() {
            // Excluded paths are handled in `classify` via `is_path_ignored`.
            Ok(iter) => Some(iter.flatten()),
            Err(e) => {
                if e.kind() != ErrorKind::PermissionDenied {
                    debug!("Error reading directory: {path:?}, {e:?}");
                }
                None
            }
        }
    }

    fn collect_children<IT>(&self, entries: IT, progress: &AtomicU64) -> Vec<Data>
    where
        IT: Iterator<Item = DirEntry> + ParallelBridge + Send,
    {
        let big_file_threshold = self.settings.big_file_threshold();

        let (mut data, small_files) = entries
            .par_bridge()
            .fold(ChildrenAccumulator::default, |accumulator, entry| {
                self.fold_children(big_file_threshold, progress, accumulator, entry)
            })
            .reduce(ChildrenAccumulator::default, merge_children_accumulators);
        if small_files.size > 0 {
            data.push(Data::remaining(small_files));
        }

        data
    }

    fn fold_children(
        &self,
        big_file_threshold: u64,
        progress: &AtomicU64,
        (mut items, mut small_files): ChildrenAccumulator,
        entry: DirEntry,
    ) -> ChildrenAccumulator {
        if self.stopper.load(Ordering::Relaxed) {
            return (items, small_files);
        }

        match self.classify(&entry) {
            Some(ScannedEntry::Dir(path)) => {
                items.extend(self.scan_directory_recursive(&path, progress))
            }
            Some(ScannedEntry::File(_, file_size)) if file_size < big_file_threshold => {
                small_files.push(file_size)
            }
            Some(ScannedEntry::File(path, file_size)) => {
                items.push(Data::new_file(path.name(), file_size))
            }
            None => {}
        }

        (items, small_files)
    }
}

type ChildrenAccumulator = (Vec<Data>, CountAndSize);

pub fn merge_children_accumulators(
    (mut left_children, left_count_and_size): ChildrenAccumulator,
    (right_children, right_count_and_size): ChildrenAccumulator,
) -> ChildrenAccumulator {
    left_children.extend(right_children);
    (left_children, left_count_and_size + right_count_and_size)
}

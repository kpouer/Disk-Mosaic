use crate::data::{Data, Kind};
use crate::model::message::Message;
use crate::model::scan_result::ScanResult;
use crate::util;
use crate::util::{MyError, PathBufToString};
use log::{debug, info, warn};
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs::{DirEntry, ReadDir};
use std::io::ErrorKind;
use std::iter::Flatten;
use std::ops::Add;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

type DirectoryEntryIterator = Flatten<ReadDir>;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn is_excluded(_path: &Path) -> bool {
    false
}

#[cfg(target_os = "macos")]
fn is_excluded(path: &Path) -> bool {
    path.starts_with("/System/Volumes")
}

#[cfg(target_os = "linux")]
fn is_excluded(path: &Path) -> bool {
    path.starts_with("/proc")
}

pub fn scan_directory_channel<T: ScanConfig>(
    path: &Path,
    sender: &Sender<Message>,
    stopper: &Arc<AtomicBool>,
    settings: T,
) {
    if let Err(e) = sender.send(Message::DirectoryScanStart(
        path.to_string_lossy().to_string(),
    )) {
        warn!("Receiver dropped {e}");
        return;
    }
    let mut scanner = Scanner::new(stopper, sender, &settings);
    let mut scan_result = ScanResult::default();
    for entry in Scanner::<T>::entry_iterator(path).into_iter().flatten() {
        if stopper.load(Ordering::Relaxed) {
            info!("Stop requested");
            break;
        }
        match scanner.classify(&entry) {
            Some(ScannedEntry::Dir(p)) => {
                if let Some(data) = scanner.scan_dir(&p)
                    && data.size > 0
                {
                    send_message(sender, Message::Data(data));
                }
            }
            Some(ScannedEntry::File(path, size)) => {
                scan_result.add_size(size);
                send_message(sender, Message::Data(Data::new_file(&path, size)));
            }
            None => {}
        }
    }
    send_message(sender, Message::DirectoryScanDone(scan_result));
}

fn send_message(sender: &Sender<Message>, message: Message) -> bool {
    if let Err(e) = sender.send(message) {
        warn!("Receiver dropped {e}");
        return false;
    }
    true
}

type ChildrenAccumulator = (Vec<Data>, CountAndSize);

pub(crate) enum ScannedEntry {
    Dir(PathBuf),
    File(PathBuf, u64),
}

#[derive(Debug)]
struct Scanner<'a, T>
where
    T: ScanConfig,
{
    stopper: &'a Arc<AtomicBool>,
    sender: &'a Sender<Message>,
    settings: &'a T,
    #[cfg(unix)]
    seen_inodes: Mutex<HashSet<u64>>,
}

impl<'a, T> Scanner<'a, T>
where
    T: ScanConfig,
{
    fn new(stopper: &'a Arc<AtomicBool>, sender: &'a Sender<Message>, settings: &'a T) -> Self {
        Self {
            stopper,
            sender,
            settings,
            #[cfg(unix)]
            seen_inodes: Mutex::new(HashSet::new()),
        }
    }

    fn is_path_ignored(&self, path: &Path) -> bool {
        self.settings.is_path_ignored(path) || is_excluded(path)
    }

    fn classify(&self, entry: &DirEntry) -> Option<ScannedEntry> {
        let file_type = entry
            .file_type()
            .inspect_err(|e| warn!("Error getting file type: {e}"))
            .ok()?;

        if file_type.is_dir() {
            let path = entry.path();
            if self.is_path_ignored(&path) {
                info!("Ignoring path: {path:?}");
                return None;
            }
            Some(ScannedEntry::Dir(path))
        } else if file_type.is_file() {
            let path = entry.path();
            let metadata = entry
                .metadata()
                .inspect_err(|e| debug!("Failed to get metadata for {path:?}: {e}"))
                .ok()?;
            #[cfg(unix)]
            {
                let ino = metadata.ino();
                if !self.seen_inodes.lock().unwrap().insert(ino) {
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

    fn scan_directory_recursive(&self, path: &Path) -> Result<Vec<Data>, MyError> {
        if self.stopper.load(Ordering::Relaxed) {
            return Ok(Vec::new());
        }
        if let Err(e) = self
            .sender
            .send(Message::DirectoryScanStart(path.absolute_path()))
        {
            warn!("Received dropped {e}");
            return Err(MyError::ReceiverDropped);
        }
        let big_file_threshold = self.settings.big_file_threshold();
        let scanned_children = match Self::entry_iterator(path)
            .map(|entries| self.collect_children(big_file_threshold, entries))
        {
            Some(scanned_children) => {
                let scan_result = scanned_children
                    .par_iter()
                    .filter_map(|data| match data.kind {
                        Kind::Dir(_) => None,
                        Kind::File => Some(ScanResult::new(1, data.size)),
                        Kind::SmallFiles(count) => Some(ScanResult::new(count, data.size)),
                    })
                    .reduce(ScanResult::default, |left, right| left + right);
                if scan_result.file_count != 0
                    && let Err(e) = self.sender.send(Message::DirectoryScanDone(scan_result))
                {
                    warn!("Received dropped {e}");
                }
                scanned_children
            }
            None => Vec::new(),
        };
        Ok(scanned_children)
    }

    fn entry_iterator(path: &Path) -> Option<DirectoryEntryIterator> {
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

    fn collect_children<IT>(&self, big_file_threshold: u64, entries: IT) -> Vec<Data>
    where
        IT: Iterator<Item = DirEntry> + ParallelBridge + Send,
    {
        let (mut data, count_and_size) = entries
            .par_bridge()
            .fold(ChildrenAccumulator::default, |accumulator, entry| {
                self.fold_children(big_file_threshold, accumulator, entry)
            })
            .reduce(ChildrenAccumulator::default, merge_children_accumulators);
        if count_and_size.size > 0 {
            data.push(Data::remaining(count_and_size));
        }

        data
    }

    fn fold_children(
        &self,
        big_file_threshold: u64,
        (mut items, mut count_and_size): ChildrenAccumulator,
        entry: DirEntry,
    ) -> ChildrenAccumulator {
        if self.stopper.load(Ordering::Relaxed) {
            return (items, count_and_size);
        }

        match self.classify(&entry) {
            Some(ScannedEntry::Dir(path)) => items.extend(self.scan_dir(&path)),
            Some(ScannedEntry::File(_, file_size)) if file_size < big_file_threshold => {
                count_and_size.push(file_size)
            }
            Some(ScannedEntry::File(path, file_size)) => {
                items.push(Data::new_file(&path, file_size))
            }
            None => {}
        }

        (items, count_and_size)
    }

    fn scan_dir(&self, entry_path: &Path) -> Option<Data> {
        match self.scan_directory_recursive(entry_path) {
            Ok(children) => {
                let mut data = Data::new_directory(entry_path);
                data.set_nodes(children);
                Some(data)
            }
            Err(e) => {
                warn!("Error recursively scanning directory {entry_path:?}: {e}");
                None
            }
        }
    }
}

pub const BIG_FILE_THRESHOLD: u64 = 10000000;

pub trait ScanConfig: Sync {
    fn big_file_threshold(&self) -> u64;
    fn is_path_ignored(&self, path: &Path) -> bool;
}

#[derive(Default)]
pub(crate) struct CountAndSize {
    pub(crate) count: u64,
    pub(crate) size: u64,
}

impl CountAndSize {
    const fn push(&mut self, file_size: u64) {
        self.count += 1;
        self.size += file_size;
    }
}

impl Add<CountAndSize> for CountAndSize {
    type Output = Self;

    fn add(mut self, rhs: CountAndSize) -> Self::Output {
        self.count += rhs.count;
        self.size += rhs.size;
        self
    }
}

fn merge_children_accumulators(
    (mut left_children, left_count_and_size): ChildrenAccumulator,
    (right_children, right_count_and_size): ChildrenAccumulator,
) -> ChildrenAccumulator {
    left_children.extend(right_children);
    (left_children, left_count_and_size + right_count_and_size)
}

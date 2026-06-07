use crate::data::{Data, Kind};
use crate::model::message::Message;
use crate::model::scan_result::ScanResult;
use crate::util;
use crate::util::{MyError, PathBufToString};
use log::{debug, info, warn};
use rayon::prelude::*;
use std::fs::{DirEntry, ReadDir};
use std::io::ErrorKind;
use std::iter::{Filter, Flatten};
use std::ops::Add;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

#[derive(Debug)]
pub struct DirectoryScanner<'a, T>
where
    T: ScanConfig,
{
    path: PathBuf,
    stopper: &'a Arc<AtomicBool>,
    sender: Sender<Message>,
    settings: &'a T,
}

impl<'a, T> DirectoryScanner<'a, T>
where
    T: ScanConfig,
{
    const fn new(
        path: PathBuf,
        stopper: &'a Arc<AtomicBool>,
        sender: Sender<Message>,
        settings: &'a T,
    ) -> Self {
        Self {
            path,
            stopper,
            sender,
            settings,
        }
    }

    fn run(self) -> Data {
        let Self {
            path,
            stopper,
            sender,
            settings,
        } = self;

        let mut data = Data::new_directory(&path);

        match Scanner::new(stopper, &sender, settings).scan_directory_recursive(&path) {
            Ok(children) => {
                data.set_nodes(children);
            }
            Err(e) => {
                warn!("Error scanning directory {path:?}: {e}");
            }
        }
        data
    }

    pub fn scan_directory_channel(
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
        let mut scan_result = ScanResult::default();
        match path.read_dir() {
            Ok(iter) => {
                iter.flatten().map(|p| p.path()).for_each(|path| {
                    if stopper.load(Ordering::Relaxed) {
                        info!("Stop requested");
                        return;
                    }
                    if path.is_dir() {
                        if settings.is_path_ignored(&path) {
                            info!("Ignoring path: {path:?}");
                            return;
                        }
                        let data =
                            DirectoryScanner::new(path, stopper, sender.clone(), &settings).run();
                        if data.size > 0 {
                            Self::send_message(sender, Message::Data(data));
                        }
                    } else if path.is_file() {
                        let size = path
                            .metadata()
                            .map_or(0, |metadata| util::get_file_size(&metadata));
                        scan_result.add_size(size);
                        Self::send_message(sender, Message::Data(Data::new_file(&path, size)));
                    }
                });
            }
            Err(e) => match e.kind() {
                ErrorKind::PermissionDenied => {}
                _ => debug!("Error reading directory: {path:?}, {e:?}"),
            },
        }
        Self::send_message(sender, Message::DirectoryScanDone(scan_result));
    }

    fn send_message(sender: &Sender<Message>, message: Message) -> bool {
        if let Err(e) = sender.send(message) {
            warn!("Receiver dropped {e}");
            return false;
        }
        true
    }
}

#[derive(Debug)]
struct Scanner<'a, T>
where
    T: ScanConfig,
{
    stopper: &'a Arc<AtomicBool>,
    sender: &'a Sender<Message>,
    settings: &'a T,
}

impl<'a, T> Scanner<'a, T>
where
    T: ScanConfig,
{
    const fn new(
        stopper: &'a Arc<AtomicBool>,
        sender: &'a Sender<Message>,
        settings: &'a T,
    ) -> Self {
        Self {
            stopper,
            sender,
            settings,
        }
    }

    fn scan_directory_recursive(&self, path: &Path) -> Result<Vec<Data>, MyError> {
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
                    .filter(|data| matches!(data.kind, Kind::File))
                    .map(|data| ScanResult {
                        file_count: 1,
                        size: data.size,
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

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn entry_iterator(path: &Path) -> Option<Flatten<fs::ReadDir>> {
        match path.read_dir() {
            Ok(iter) => {
                let iter = iter.flatten();
                Some(iter)
            }
            Err(e) => {
                if e.kind() != ErrorKind::PermissionDenied {
                    debug!("Error reading directory: {path:?}, {e:?}");
                }
                None
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn entry_iterator(path: &Path) -> Option<Filter<Flatten<ReadDir>, fn(&DirEntry) -> bool>> {
        match path.read_dir() {
            Ok(iter) => {
                let iter: Filter<Flatten<ReadDir>, fn(&DirEntry) -> bool> = iter
                    .flatten()
                    .filter(|p| !p.path().starts_with("/System/Volumes"));
                Some(iter)
            }
            Err(e) => {
                if e.kind() != ErrorKind::PermissionDenied {
                    debug!("Error reading directory: {path:?}, {e:?}");
                }
                None
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn entry_iterator(path: &Path) -> Option<Filter<Flatten<ReadDir>, fn(&DirEntry) -> bool>> {
        match path.read_dir() {
            Ok(iter) => {
                let iter = iter.flatten().filter(|p| !p.path().starts_with("/proc"));
                Some(iter)
            }
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
            .fold(
                || (Vec::new(), CountAndSize::default()),
                |(mut items, mut count_and_size), entry| {
                    if self.stopper.load(Ordering::Relaxed) {
                        return (items, count_and_size);
                    }

                    let entry_path = entry.path();
                    let metadata = match entry.metadata() {
                        Ok(m) => m,
                        Err(e) => {
                            debug!("Failed to get metadata for {entry_path:?}: {e}");
                            return (items, count_and_size);
                        }
                    };
                    if metadata.is_dir() {
                        if let Some(dir_data) = self.process_dir(&entry_path) {
                            items.push(dir_data);
                        }
                    } else if metadata.is_file() {
                        let file_size = util::get_file_size(&metadata);
                        if file_size < big_file_threshold {
                            count_and_size.push(file_size);
                        } else {
                            items.push(Data::new_file(&entry_path, file_size));
                        }
                    }
                    (items, count_and_size)
                },
            )
            .reduce(
                || (Vec::new(), CountAndSize::default()),
                |(mut items_l, left), (items_r, right)| {
                    items_l.extend(items_r);
                    (items_l, left + right)
                },
            );
        if count_and_size.size > 0 {
            data.push(Data::remaining(count_and_size));
        }

        data
    }

    fn process_dir(&self, entry_path: &PathBuf) -> Option<Data> {
        {
            if self.settings.is_path_ignored(entry_path) {
                info!("Ignoring path: {entry_path:?}");
                return None;
            }
        }
        match self.scan_directory_recursive(entry_path) {
            Ok(grandchildren) => {
                let mut dir_data = Data::new_directory(entry_path);
                dir_data.set_nodes(grandchildren);
                Some(dir_data)
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

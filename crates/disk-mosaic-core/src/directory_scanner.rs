use crate::data::{Data, Kind};
use crate::model::message::Message;
use crate::model::scan_result::ScanResult;
use crate::util;
use crate::util::PathBufToString;
use log::{debug, info, warn};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::{DirEntry, ReadDir};
use std::io::ErrorKind;
use std::iter::Flatten;
use std::ops::Add;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;
use std::{fs, thread};

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
        debug!("Receiver dropped {e}");
        return;
    }
    let scanner = Scanner::new(path, stopper, sender, &settings);
    let counters = RootCounters::default();
    let big_file_threshold = settings.big_file_threshold();
    // dropping this sender stops the progress reporter
    let (done_tx, done_rx) = mpsc::channel::<()>();
    thread::scope(|scope| {
        let reporter = &scanner;
        scope.spawn(move || reporter.report_progress(&done_rx));
        scan_top_level(
            path,
            &scanner,
            sender,
            stopper,
            big_file_threshold,
            &counters,
        );
        drop(done_tx);
    });
    let small_count = counters.small_files_count.load(Ordering::Relaxed);
    let small_size = counters.small_files_size.load(Ordering::Relaxed);
    if small_size > 0 {
        send_message(
            sender,
            Message::Data(Data::remaining(CountAndSize {
                count: small_count,
                size: small_size,
            })),
        );
    }
    send_message(
        sender,
        Message::DirectoryScanDone(ScanResult::new(
            counters.file_count.load(Ordering::Relaxed) + small_count,
            counters.total_size.load(Ordering::Relaxed) + small_size,
        )),
    );
}

/// Counters of the files found directly in the scanned root.
#[derive(Default)]
struct RootCounters {
    file_count: AtomicU64,
    total_size: AtomicU64,
    small_files_count: AtomicU64,
    small_files_size: AtomicU64,
}

fn scan_top_level<T: ScanConfig>(
    path: &Path,
    scanner: &Scanner<T>,
    sender: &Sender<Message>,
    stopper: &Arc<AtomicBool>,
    big_file_threshold: u64,
    counters: &RootCounters,
) {
    let RootCounters {
        file_count,
        total_size,
        small_files_count,
        small_files_size,
    } = counters;
    Scanner::<T>::entry_iterator(path)
        .into_iter()
        .flatten()
        .par_bridge()
        .for_each(|entry| {
            if stopper.load(Ordering::Relaxed) {
                debug!("Stop requested");
                return;
            }
            match scanner.classify(&entry) {
                Some(ScannedEntry::Dir(p)) => {
                    let name = p.name();
                    let progress = scanner.progress.start(&name);
                    let data = scanner.scan_directory_recursive(&p, &progress);
                    // The lock is held while sending so that the reporter cannot send
                    // a progress message for this directory after its final data.
                    let mut running = scanner.progress.lock();
                    running.remove(&name);
                    if let Some(data) = data {
                        // a zero size removes the provisional entry on the UI side
                        send_message(sender, Message::Data(data));
                    }
                }
                Some(ScannedEntry::File(_, size)) if size < big_file_threshold => {
                    small_files_count.fetch_add(1, Ordering::Relaxed);
                    small_files_size.fetch_add(size, Ordering::Relaxed);
                }
                Some(ScannedEntry::File(path, size)) if size > 0 => {
                    file_count.fetch_add(1, Ordering::Relaxed);
                    total_size.fetch_add(size, Ordering::Relaxed);
                    send_message(sender, Message::File(path.name(), size));
                }
                Some(ScannedEntry::File(_, _)) | None => {}
            }
        });
}

/// Interval between two progress messages.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(150);

/// Size scanned so far for each top-level directory being scanned.
#[derive(Debug, Default)]
struct TopLevelProgress {
    running: Mutex<HashMap<String, Arc<AtomicU64>>>,
}

impl TopLevelProgress {
    fn start(&self, name: &str) -> Arc<AtomicU64> {
        let counter = Arc::new(AtomicU64::new(0));
        self.lock().insert(name.to_string(), Arc::clone(&counter));
        counter
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Arc<AtomicU64>>> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn send_message(sender: &Sender<Message>, message: Message) -> bool {
    if let Err(e) = sender.send(message) {
        debug!("Receiver dropped {e}");
        return false;
    }
    true
}

type ChildrenAccumulator = (Vec<Data>, CountAndSize);

pub(crate) enum ScannedEntry {
    Dir(PathBuf),
    File(PathBuf, u64),
}

type Dev = u64;
type Ino = u64;
type FileId = (Dev, Ino);

#[derive(Debug)]
struct Scanner<'a, T>
where
    T: ScanConfig,
{
    stopper: &'a Arc<AtomicBool>,
    sender: &'a Sender<Message>,
    settings: &'a T,
    progress: TopLevelProgress,
    #[cfg(unix)]
    seen_inodes: Mutex<std::collections::HashSet<FileId>>,
    /// The device where the directory being scanned resides.
    #[cfg(unix)]
    device: Option<Dev>,
}

impl<'a, T> Scanner<'a, T>
where
    T: ScanConfig,
{
    fn new(
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
            seen_inodes: Mutex::new(std::collections::HashSet::new()),
            #[cfg(unix)]
            device: fs::metadata(path).map(|m| m.dev()).ok(),
        }
    }

    /// Periodically send the provisional size of the top-level directory being scanned,
    /// until `done` is disconnected.
    fn report_progress(&self, done: &Receiver<()>) {
        let mut last_sent: HashMap<String, u64> = HashMap::new();
        while let Err(RecvTimeoutError::Timeout) = done.recv_timeout(PROGRESS_INTERVAL) {
            let running = self.progress.lock();
            for (name, counter) in running.iter() {
                let size = counter.load(Ordering::Relaxed);
                if size > 0 && last_sent.get(name) != Some(&size) {
                    if !send_message(self.sender, Message::DirectoryProgress(name.clone(), size)) {
                        return;
                    }
                    last_sent.insert(name.clone(), size);
                }
            }
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
    fn scan_directory_recursive(&self, path: &Path, progress: &AtomicU64) -> Option<Data> {
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

pub trait ScanConfig: Sync {
    fn big_file_threshold(&self) -> u64;
    fn is_path_ignored(&self, path: &Path) -> bool;
    #[cfg(unix)]
    fn ignore_different_device(&self) -> bool;
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    struct TestConfig {
        ignore_different_device: bool,
    }

    impl ScanConfig for TestConfig {
        fn big_file_threshold(&self) -> u64 {
            0
        }

        fn is_path_ignored(&self, _path: &Path) -> bool {
            false
        }

        fn ignore_different_device(&self) -> bool {
            self.ignore_different_device
        }
    }

    /// Returns the final result and the number of top-level directories reported.
    fn scan(path: &Path, ignore_different_device: bool) -> (ScanResult, usize) {
        let (tx, rx) = mpsc::channel();
        let stopper = Arc::new(AtomicBool::new(false));
        scan_directory_channel(
            path,
            &tx,
            &stopper,
            TestConfig {
                ignore_different_device,
            },
        );
        drop(tx);
        let mut result = None;
        let mut dirs = 0;
        for message in rx {
            match message {
                Message::DirectoryScanDone(r) => result = Some(r),
                Message::Data(_) => dirs += 1,
                _ => {}
            }
        }
        (
            result.expect("scan should end with DirectoryScanDone"),
            dirs,
        )
    }

    #[test]
    fn same_device_files_are_scanned_with_ignore_different_device() {
        let root = std::env::temp_dir().join(format!("disk-mosaic-test-{}", std::process::id()));
        let sub = root.join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(root.join("a.txt"), [0u8; 10]).unwrap();
        fs::write(sub.join("b.txt"), [0u8; 20]).unwrap();

        let (filtered, filtered_dirs) = scan(&root, true);
        let (unfiltered, unfiltered_dirs) = scan(&root, false);
        fs::remove_dir_all(&root).unwrap();

        // Everything lives on the scanned device: nothing may be filtered out.
        assert_eq!(filtered.file_count, 1);
        assert_eq!(filtered_dirs, 1);
        assert_eq!(filtered.file_count, unfiltered.file_count);
        assert_eq!(filtered.size, unfiltered.size);
        assert_eq!(filtered_dirs, unfiltered_dirs);
    }

    #[test]
    fn entries_on_another_device_are_skipped() {
        let root =
            std::env::temp_dir().join(format!("disk-mosaic-test-dev-{}", std::process::id()));
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("a.txt"), [0u8; 10]).unwrap();

        let classify_all = |ignore: bool, fake_device: bool| {
            let (tx, _rx) = mpsc::channel();
            let stopper = Arc::new(AtomicBool::new(false));
            let config = TestConfig {
                ignore_different_device: ignore,
            };
            let mut scanner = Scanner::new(&root, &stopper, &tx, &config);
            if fake_device {
                // Pretend the scan started on a device that none of the entries live on.
                scanner.device = scanner.device.map(|d| d.wrapping_add(1));
            }
            fs::read_dir(&root)
                .unwrap()
                .flatten()
                .filter_map(|e| scanner.classify(&e))
                .count()
        };

        let same_device = classify_all(true, false);
        let other_device_ignored = classify_all(true, true);
        let other_device_allowed = classify_all(false, true);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(same_device, 2);
        assert_eq!(other_device_ignored, 0);
        assert_eq!(other_device_allowed, 2);
    }
}

use crate::data::Data;
use crate::model::message::Message;
use crate::model::scan_result::ScanResult;
use crate::scan_settings::ScanSettings;
use crate::util::PathBufToString;
use counters::{CountAndSize, RootCounters};
use log::debug;
use rayon::prelude::*;
use scanner::{ScannedEntry, Scanner};
use std::fs::ReadDir;
use std::iter::Flatten;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread;

pub mod config;
pub(crate) mod counters;
mod exclusion;
mod progress;
mod scanner;

type DirectoryEntryIterator = Flatten<ReadDir>;

pub fn scan_directory_channel(
    path: &Path,
    sender: &Sender<Message>,
    stopper: &Arc<AtomicBool>,
    scan_settings: ScanSettings,
) {
    if let Err(e) = sender.send(Message::DirectoryScanStart(
        path.to_string_lossy().to_string(),
    )) {
        debug!("Receiver dropped {e}");
        return;
    }
    let scanner = Scanner::new(path, stopper, sender, &scan_settings);
    let counters = RootCounters::default();
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
            scan_settings.big_file_threshold,
            &counters,
        );
        drop(done_tx);
    });
    let small_count = counters.small_files_count.load(Ordering::Relaxed);
    let small_size = counters.small_files_size.load(Ordering::Relaxed);
    if small_size > 0 {
        progress::send_message(
            sender,
            Message::Data(Data::remaining(CountAndSize {
                count: small_count,
                size: small_size,
            })),
        );
    }
    progress::send_message(
        sender,
        Message::DirectoryScanDone(ScanResult::new(
            counters.file_count.load(Ordering::Relaxed) + small_count,
            counters.total_size.load(Ordering::Relaxed) + small_size,
        )),
    );
}

fn scan_top_level(
    path: &Path,
    scanner: &Scanner,
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
    Scanner::entry_iterator(path)
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
                        progress::send_message(sender, Message::Data(data));
                    }
                }
                Some(ScannedEntry::File(_, size)) if size < big_file_threshold => {
                    small_files_count.fetch_add(1, Ordering::Relaxed);
                    small_files_size.fetch_add(size, Ordering::Relaxed);
                }
                Some(ScannedEntry::File(path, size)) if size > 0 => {
                    file_count.fetch_add(1, Ordering::Relaxed);
                    total_size.fetch_add(size, Ordering::Relaxed);
                    progress::send_message(sender, Message::File(path.name(), size));
                }
                Some(ScannedEntry::File(_, _)) | None => {}
            }
        });
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;

    /// Returns the final result and the number of top-level directories reported.
    fn scan(path: &Path, ignore_different_device: bool) -> (ScanResult, usize) {
        let (tx, rx) = mpsc::channel();
        let stopper = Arc::new(AtomicBool::new(false));
        scan_directory_channel(
            path,
            &tx,
            &stopper,
            ScanSettings {
                ignore_different_device,
                ..ScanSettings::default()
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
            let mut scanner = Scanner::new(
                &root,
                &stopper,
                &tx,
                &ScanSettings {
                    ignore_different_device: ignore,
                },
            );
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

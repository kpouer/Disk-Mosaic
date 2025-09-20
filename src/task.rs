use crate::data::{Data, Kind};
use crate::settings::Settings;
use crate::ui::app_state::analyzer::{Message, ScanResult};
use crate::util;
use crate::util::{MyError, PathBufToString};
use log::{debug, info, warn};
use rayon::prelude::*;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct Task<'a> {
    path: PathBuf,
    tx: &'a Sender<Message>,
    stopper: &'a Arc<AtomicBool>,
    sender: Sender<Message>,
    settings: &'a Arc<Mutex<Settings>>,
}

impl<'a> Task<'a> {
    pub const fn new(
        path: PathBuf,
        tx: &'a Sender<Message>,
        stopper: &'a Arc<AtomicBool>,
        sender: Sender<Message>,
        settings: &'a Arc<Mutex<Settings>>,
    ) -> Self {
        Self {
            path,
            tx,
            stopper,
            sender,
            settings,
        }
    }

    pub fn run(self) {
        let Self {
            path,
            tx,
            stopper,
            sender,
            settings,
        } = self;

        let mut data = Data::new_directory(&path);

        match Self::scan_directory_recursive(&path, stopper, &sender, settings) {
            Ok(children) => {
                data.set_nodes(children);
            }
            Err(e) => {
                warn!("Error scanning directory {path:?}: {e}");
            }
        }

        if let Err(e) = tx.send(Message::Data(data)) {
            warn!("Failed to send data message: {e}");
        }
    }

    fn scan_directory_recursive(
        path: &Path,
        stopper: &Arc<AtomicBool>,
        sender: &Sender<Message>,
        settings: &Arc<Mutex<Settings>>,
    ) -> Result<Vec<Data>, MyError> {
        if let Err(e) = sender.send(Message::DirectoryScanStart(path.absolute_path())) {
            warn!("Received dropped {e}");
            return Err(MyError::ReceiverDropped);
        }
        let big_file_threshold = settings.lock().unwrap().big_file_threshold();
        let entries = match path.read_dir() {
            Ok(iter) => {
                let iter = iter.flatten();
                #[cfg(target_os = "macos")]
                let iter = iter.filter(|p| !p.path().starts_with("/System/Volumes"));
                #[cfg(target_os = "linux")]
                let iter = iter.filter(|p| !p.path().starts_with("/proc"));
                iter.collect::<Vec<_>>()
            }
            Err(e) => {
                if e.kind() != ErrorKind::PermissionDenied {
                    debug!("Error reading directory: {path:?}, {e:?}");
                }
                return Ok(Vec::new());
            }
        };

        let small_file_data = Arc::new(Mutex::new(Data {
            name: "Remaining".to_string(),
            kind: Kind::SmallFiles(0),
            size: 0,
            color: Data::next_color(),
            ..Default::default()
        }));

        let mut children: Vec<Data> = entries
            .par_iter()
            .filter_map(|entry| {
                if stopper.load(Ordering::Relaxed) {
                    debug!("Stop requested during recursive scan");
                    return None;
                }

                let entry_path = entry.path();
                let metadata = match entry.metadata() {
                    Ok(m) => m,
                    Err(e) => {
                        debug!("Failed to get metadata for {entry_path:?}: {e}");
                        return None;
                    }
                };
                if metadata.is_dir() {
                    {
                        let settings = settings.lock().unwrap();
                        if settings.is_path_ignored(&entry_path) {
                            info!("Ignoring path: {entry_path:?}");
                            return None;
                        }
                    }
                    match Self::scan_directory_recursive(&entry_path, stopper, sender, settings) {
                        Ok(grandchildren) => {
                            let mut dir_data = Data::new_directory(&entry_path);
                            dir_data.set_nodes(grandchildren);
                            Some(dir_data)
                        }
                        Err(e) => {
                            warn!("Error recursively scanning directory {entry_path:?}: {e}");
                            None
                        }
                    }
                } else if metadata.is_file() {
                    let size = util::get_file_size(&entry_path);
                    if size < big_file_threshold {
                        let mut d = small_file_data.lock().unwrap();
                        if let Kind::SmallFiles(count) = &mut d.kind {
                            *count += 1;
                        }
                        d.size += size;
                        None
                    } else {
                        Some(Data::new_file(&entry_path, size))
                    }
                } else {
                    // Ignore symlinks, sockets, etc.
                    None
                }
            })
            .collect();
        let mut file_result = children
            .par_iter()
            .filter(|data| matches!(data.kind, Kind::File))
            .map(|data| ScanResult {
                file_count: 1,
                size: data.size,
            })
            .reduce(ScanResult::default, |d1, d2| d1 + d2);

        {
            let small_file_data = small_file_data.lock().unwrap();
            if small_file_data.size > 0 {
                children.push(small_file_data.clone());
            }
            if let Kind::SmallFiles(count) = small_file_data.kind {
                file_result.file_count += count;
            }
            file_result.size += small_file_data.size;
        }
        if file_result.file_count != 0
            && let Err(e) = sender.send(Message::DirectoryScanDone(file_result))
        {
            warn!("Received dropped {e}");
        }
        Ok(children)
    }

    pub fn scan_directory_channel(
        path: &Path,
        sender: &Sender<Message>,
        stopper: &Arc<AtomicBool>,
        settings: Arc<Mutex<Settings>>,
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
                let vec = iter.collect::<Vec<_>>();
                vec.iter().flatten().map(|p| p.path()).for_each(|path| {
                    if stopper.load(Ordering::Relaxed) {
                        info!("Stop requested");
                        return;
                    }
                    if path.is_dir() {
                        {
                            let settings = settings.lock().unwrap();
                            if settings.is_path_ignored(&path) {
                                info!("Ignoring path: {path:?}");
                                return;
                            }
                        }
                        Task::new(path, sender, stopper, sender.clone(), &settings).run();
                    } else if path.is_file() {
                        let size = util::get_file_size(&path);
                        scan_result.add_size(size);
                        if let Err(e) = sender.send(Message::Data(Data::new_file(&path, size))) {
                            warn!("Receiver dropped {e}");
                        }
                    }
                });
            }
            Err(e) => match e.kind() {
                ErrorKind::PermissionDenied => {}
                _ => debug!("Error reading directory: {path:?}, {e:?}"),
            },
        }
        if let Err(e) = sender.send(Message::DirectoryScanDone(scan_result)) {
            warn!("Receiver dropped {e}");
        }
    }
}

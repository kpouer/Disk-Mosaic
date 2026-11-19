use crate::directory_scanner::config::ScanConfig;
use crate::directory_scanner::scanner::Scanner;
use crate::model::message::Message;
use log::debug;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// Interval between two progress messages.
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(150);

/// Size scanned so far for each top-level directory being scanned.
#[derive(Debug, Default)]
pub(super) struct TopLevelProgress {
    running: Mutex<HashMap<String, Arc<AtomicU64>>>,
}

impl TopLevelProgress {
    pub(crate) fn start(&self, name: &str) -> Arc<AtomicU64> {
        let counter = Arc::new(AtomicU64::new(0));
        self.lock().insert(name.to_string(), Arc::clone(&counter));
        counter
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, HashMap<String, Arc<AtomicU64>>> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub fn send_message(sender: &Sender<Message>, message: Message) -> bool {
    if let Err(e) = sender.send(message) {
        debug!("Receiver dropped {e}");
        return false;
    }
    true
}

impl<'a, T> Scanner<'a, T>
where
    T: ScanConfig,
{
    /// Periodically send the provisional size of the top-level directory being scanned,
    /// until `done` is disconnected.
    pub(super) fn report_progress(&self, done: &Receiver<()>) {
        let mut last_sent = HashMap::new();
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
}

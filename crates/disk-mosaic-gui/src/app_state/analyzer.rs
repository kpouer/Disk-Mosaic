use crate::about_dialog::AboutDialog;
use crate::path_bar::PathBar;
use crate::settings::Settings;
use crate::treemap_panel::TreeMapPanel;
use disk_mosaic_core::analysis_result::AnalysisResult;
use disk_mosaic_core::data::Data;
use disk_mosaic_core::directory_scanner::scan_directory_channel;
use disk_mosaic_core::model::message::Message;
use disk_mosaic_core::model::scan_result::ScanResult;
use egui::{Label, Ui};
use humansize::DECIMAL;
use log::info;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

/// Maximum number of scan messages processed per frame while the scan is running,
/// so that a flood of messages cannot freeze the UI.
const MAX_MESSAGES_PER_FRAME: usize = 1000;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AnalyzerUpdate {
    Running,
    Finished,
    GoBack,
}

#[derive(Debug)]
pub(crate) struct Analyzer {
    pub(crate) analysis_result: AnalysisResult,
    rx: Receiver<Message>,
    stopper: Arc<AtomicBool>,
    handle: thread::JoinHandle<()>,
    scanning: String,
    scanned_directories: u64,
    scan_result: ScanResult,
    about_open: bool,
    settings: Settings,
}

impl Analyzer {
    /// Create a new analyzer.
    /// The analyzer will scan the given directory and all subdirectories in a thread.
    pub(crate) fn new(root: PathBuf, settings: Settings) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let stopper = Arc::new(AtomicBool::new(false));
        let root_copy = root.clone();
        let stopper_copy = stopper.clone();
        let settings_copy = settings.clone();
        let handle = thread::spawn(move || {
            let start = std::time::Instant::now();
            scan_directory_channel(&root_copy, &tx, &stopper_copy, settings_copy);
            info!("Done in {}ms", start.elapsed().as_millis());
        });
        let root_data = Data::new_directory(&root);
        Self {
            analysis_result: AnalysisResult::new(root, vec![root_data]),
            rx,
            stopper,
            handle,
            scanning: String::new(),
            scanned_directories: 0,
            scan_result: ScanResult::default(),
            about_open: false,
            settings,
        }
    }

    /// Whether the user asked to stop the scan, in which case the results are partial.
    pub(crate) fn was_stopped(&self) -> bool {
        self.stopper.load(Ordering::Relaxed)
    }

    pub(crate) fn show(&mut self, ui: &mut Ui) -> AnalyzerUpdate {
        let scan_finished = self.poll_scan();
        let top_panel_result = self.show_top_panel(ui);

        if top_panel_result == AnalyzerUpdate::GoBack {
            info!("Stop requested via Back button");
            self.stopper.store(true, Ordering::Relaxed);
            return AnalyzerUpdate::GoBack;
        }

        egui::CentralPanel::default().show(ui, |ui| {
            TreeMapPanel::new(&mut self.analysis_result, &self.settings, false).show(ui);
        });
        ui.ctx().request_repaint_after(Duration::from_millis(60));

        if scan_finished {
            AnalyzerUpdate::Finished
        } else {
            AnalyzerUpdate::Running
        }
    }

    fn poll_scan(&mut self) -> bool {
        let scan_finished = self.handle.is_finished();
        if scan_finished {
            self.receive_data(usize::MAX);
        } else {
            self.receive_data(MAX_MESSAGES_PER_FRAME);
        }
        scan_finished
    }

    fn receive_data(&mut self, max_messages: usize) {
        for message in self.rx.try_iter().take(max_messages) {
            match message {
                Message::DirectoryScanStart(d) => {
                    self.scanning = d;
                    self.scanned_directories += 1;
                }
                Message::DirectoryScanDone(scan_result) => self.scan_result += scan_result,
                Message::Data(data) => match self.analysis_result.data_stack.last_mut() {
                    Some(current_data) => current_data.upsert_child(data),
                    None => log::error!("Data stack is empty when receiving data"),
                },
                Message::DirectoryProgress(name, size) => {
                    match self.analysis_result.data_stack.last_mut() {
                        Some(current_data) => current_data.set_child_size(name, size),
                        None => log::error!("Data stack is empty when receiving data"),
                    }
                }
                Message::File(name, size) => match self.analysis_result.data_stack.last_mut() {
                    Some(current_data) => current_data.push(Data::new_file(name, size)),
                    None => log::error!("Data stack is empty when receiving data"),
                },
            }
        }
    }

    fn show_top_panel(&mut self, ui: &mut Ui) -> AnalyzerUpdate {
        let mut update_status = AnalyzerUpdate::Running;
        egui::Panel::top("top_panel").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("⬅").clicked() {
                    update_status = AnalyzerUpdate::GoBack;
                }
                if ui.button("Stop").clicked() {
                    info!("Stop requested via Stop button");
                    self.stopper.store(true, Ordering::Relaxed);
                }
                PathBar::new(&mut self.analysis_result).show(ui);

                let scanning_label = Label::new(format!(
                    "Dirs: {}, Files: {}, Size: {}, scanning {}",
                    self.scanned_directories,
                    self.scan_result.file_count,
                    humansize::format_size(self.scan_result.size, DECIMAL),
                    self.scanning,
                ));
                ui.add(scanning_label);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    AboutDialog::new(&mut self.about_open).show_button(ui);
                });
            });
        });

        update_status
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use disk_mosaic_core::data::Kind;
    use std::fs;

    /// More top-level directories than messages processed per frame: the final results must
    /// all be received even if the scan finishes before the UI drained the channel.
    #[test]
    fn all_messages_are_processed_when_scan_finishes() {
        const DIRS: usize = MAX_MESSAGES_PER_FRAME + 500;
        let root =
            std::env::temp_dir().join(format!("disk-mosaic-analyzer-{}", std::process::id()));
        for i in 0..DIRS {
            let dir = root.join(format!("dir{i}"));
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("file"), b"x").unwrap();
        }

        let mut analyzer = Analyzer::new(root.clone(), Settings::default());
        while !analyzer.handle.is_finished() {
            thread::sleep(Duration::from_millis(10));
        }
        // the scan is over before the first poll, as when the UI lags behind
        assert!(analyzer.poll_scan());
        fs::remove_dir_all(&root).unwrap();

        let Kind::Dir(children) = &analyzer.analysis_result.data_stack[0].kind else {
            panic!("root must be a directory");
        };
        assert_eq!(children.len(), DIRS);
    }
}

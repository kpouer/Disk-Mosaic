use crate::about_dialog::AboutDialog;
use crate::path_bar::PathBar;
use crate::settings::Settings;
use crate::treemap_panel::TreeMapPanel;
use disk_mosaic_core::analysis_result::AnalysisResult;
use disk_mosaic_core::data::Data;
use disk_mosaic_core::directory_scanner::{DirectoryScanner, ScanConfig};
use disk_mosaic_core::model::message::Message;
use disk_mosaic_core::model::scan_result::ScanResult;
use egui::{Label, Ui};
use humansize::DECIMAL;
use log::info;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use treemap::Mappable;

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
    settings: Arc<Mutex<Settings>>,
}

impl Analyzer {
    /// Create a new analyzer.
    /// The analyzer will scan the given directory and all subdirectories in a thread.
    pub(crate) fn new(root: PathBuf, settings: Arc<Mutex<Settings>>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let stopper = Arc::new(AtomicBool::new(false));
        let root_copy = root.clone();
        let stopper_copy = stopper.clone();
        let settings_copy = Arc::clone(&settings);
        let handle = thread::spawn(move || {
            let start = std::time::Instant::now();
            DirectoryScanner::scan_directory_channel(&root_copy, &tx, &stopper_copy, ScannerConfig::new(settings_copy));
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

    pub(crate) fn show(&mut self, ui: &mut Ui) -> AnalyzerUpdate {
        self.receive_data();
        let top_panel_result = self.show_top_panel(ui);

        if top_panel_result == AnalyzerUpdate::GoBack {
            info!("Stop requested via Back button");
            self.stopper.store(true, Ordering::Relaxed);
            return AnalyzerUpdate::GoBack;
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            TreeMapPanel::new(&mut self.analysis_result, &self.settings, false).show(ui);
        });
        ui.ctx().request_repaint_after(Duration::from_millis(60));

        if self.handle.is_finished() {
            AnalyzerUpdate::Finished
        } else {
            AnalyzerUpdate::Running
        }
    }

    fn receive_data(&mut self) {
        let mut count = 1000;
        for message in self.rx.try_iter() {
            match message {
                Message::DirectoryScanStart(d) => {
                    self.scanning = d;
                    self.scanned_directories += 1;
                }
                Message::DirectoryScanDone(scan_result) => self.scan_result += scan_result,
                Message::Data(data) => {
                    if data.size() > 0.0 {
                        match self.analysis_result.data_stack.last_mut() {
                            Some(current_data) => current_data.push(data),
                            None => log::error!("Data stack is empty when receiving data"),
                        }
                    }
                }
            }
            count -= 1;
            if count == 0 {
                break;
            }
        }
    }

    fn show_top_panel(&mut self, ui: &mut Ui) -> AnalyzerUpdate {
        let mut update_status = AnalyzerUpdate::Running;
        egui::Panel::top("top_panel").show_inside(ui, |ui| {
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

#[derive(Debug)]
struct ScannerConfig {
    big_file_threshold: u64,
    settings: Arc<Mutex<Settings>>,
}

impl ScannerConfig {
    fn new(settings: Arc<Mutex<Settings>>) -> Self {
        let big_file_threshold = settings.lock().unwrap().big_file_threshold();
        Self {
            big_file_threshold,
            settings
        }
    }
}

impl ScanConfig for ScannerConfig {
    fn big_file_threshold(&self) -> u64 {
        self.big_file_threshold
    }

    fn is_path_ignored(&self, path: &Path) -> bool {
        self.settings.lock().unwrap().is_path_ignored(path)
    }
}
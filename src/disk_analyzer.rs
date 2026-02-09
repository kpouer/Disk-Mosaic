use crate::disk_analyzer::AppState::SelectDisk;
use crate::settings::Settings;
use crate::ui::app_state::analyzer::{Analyzer, AnalyzerUpdate}; // Added AnalyzerUpdate
use crate::ui::app_state::result_view::ResultView;
use crate::ui::app_state::select_target::SelectTarget;
use log::info;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
enum AppState {
    SelectDisk(SelectTarget),
    Analyzing(Analyzer),
    Analyzed(ResultView),
}

impl DiskAnalyzerApp {
    pub(crate) fn new(settings: Settings, initial_path: Option<PathBuf>) -> Self {
        let settings = Arc::new(Mutex::new(settings));
        let state = match initial_path {
            Some(path) => {
                info!(
                    "CLI path provided: {:?}, starting analysis immediately",
                    path
                );
                AppState::Analyzing(Analyzer::new(path, Arc::clone(&settings)))
            }
            None => SelectDisk(SelectTarget::new(Arc::clone(&settings))),
        };
        Self { settings, state }
    }
}

#[derive(Debug)]
pub struct DiskAnalyzerApp {
    settings: Arc<Mutex<Settings>>,
    state: AppState,
}

impl eframe::App for DiskAnalyzerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        match &mut self.state {
            AppState::SelectDisk(select_target) => {
                if let Some(selected_path) = select_target.show(ctx) {
                    info!("Selected path: {selected_path:?}");
                    self.state = AppState::Analyzing(Analyzer::new(
                        selected_path,
                        Arc::clone(&self.settings),
                    ));
                }
            }
            AppState::Analyzing(analyzer) => match analyzer.show(ctx) {
                AnalyzerUpdate::Finished => {
                    info!("Analysis finished, transitioning to ResultView");
                    let analysis_result = std::mem::take(&mut analyzer.analysis_result);
                    self.state = AppState::Analyzed(ResultView::new(
                        analysis_result,
                        Arc::clone(&self.settings),
                    ));
                }
                AnalyzerUpdate::GoBack => {
                    info!("Back requested from Analyzer, transitioning to SelectTarget");
                    self.state =
                        AppState::SelectDisk(SelectTarget::new(Arc::clone(&self.settings)));
                }
                AnalyzerUpdate::Running => {}
            },
            AppState::Analyzed(result_view) => {
                if result_view.show(ctx) {
                    info!("Back requested from ResultView, transitioning to SelectTarget");
                    self.state =
                        AppState::SelectDisk(SelectTarget::new(Arc::clone(&self.settings)));
                }
            }
        }

        if ctx.input(|i| i.viewport().close_requested()) {
            let settings = self.settings.lock().unwrap();
            settings.save().expect("Unable to save settings");
        }
    }
}

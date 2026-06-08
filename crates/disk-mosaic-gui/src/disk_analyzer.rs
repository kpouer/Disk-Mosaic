use crate::app_state::analyzer::{Analyzer, AnalyzerUpdate};
use crate::app_state::result_view::ResultView;
use crate::app_state::select_target::SelectTarget;
use crate::disk_analyzer::AppState::SelectDisk;
use crate::settings::Settings;
use eframe::Frame;
use egui::{Context, Ui};
use log::info;
use std::path::PathBuf;

#[derive(Debug)]
enum AppState {
    SelectDisk(SelectTarget),
    Analyzing(Analyzer),
    Analyzed(ResultView),
}

impl DiskAnalyzerApp {
    pub(crate) fn new(_settings: Settings, initial_path: Option<PathBuf>) -> Self {
        let settings = Settings::default();
        let state = match initial_path {
            Some(path) => {
                info!("CLI path provided: {path:?}, starting analysis immediately");
                AppState::Analyzing(Analyzer::new(path, settings.clone()))
            }
            None => SelectDisk(SelectTarget::new(settings.clone())),
        };
        Self { settings, state }
    }
}

#[derive(Debug)]
pub(crate) struct DiskAnalyzerApp {
    settings: Settings,
    state: AppState,
}

impl eframe::App for DiskAnalyzerApp {
    fn logic(&mut self, _: &Context, _: &mut Frame) {}

    fn ui(&mut self, ui: &mut Ui, _: &mut Frame) {
        match &mut self.state {
            AppState::SelectDisk(select_target) => {
                if let Some(selected_path) = select_target.show(ui) {
                    info!("Selected path: {selected_path:?}");
                    self.state =
                        AppState::Analyzing(Analyzer::new(selected_path, self.settings.clone()));
                }
            }
            AppState::Analyzing(analyzer) => match analyzer.show(ui) {
                AnalyzerUpdate::Finished => {
                    info!("Analysis finished, transitioning to ResultView");
                    let analysis_result = std::mem::take(&mut analyzer.analysis_result);
                    self.state =
                        AppState::Analyzed(ResultView::new(analysis_result, self.settings.clone()));
                }
                AnalyzerUpdate::GoBack => {
                    info!("Back requested from Analyzer, transitioning to SelectTarget");
                    self.state = AppState::SelectDisk(SelectTarget::new(self.settings.clone()));
                }
                AnalyzerUpdate::Running => {}
            },
            AppState::Analyzed(result_view) => {
                if result_view.show(ui) {
                    info!("Back requested from ResultView, transitioning to SelectTarget");
                    self.state = AppState::SelectDisk(SelectTarget::new(self.settings.clone()));
                }
            }
        }

        if ui.ctx().input(|i| i.viewport().close_requested()) {
            self.settings.save().expect("Unable to save settings");
        }
    }
}

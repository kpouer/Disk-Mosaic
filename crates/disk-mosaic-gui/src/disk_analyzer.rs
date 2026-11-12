use crate::app_state::analyzer::{Analyzer, AnalyzerUpdate};
use crate::app_state::result_view::ResultView;
use crate::app_state::select_target::SelectTarget;
use crate::disk_analyzer::AppState::{Error, SelectDisk};
use crate::settings::Settings;
use eframe::Frame;
use egui::{Context, Ui};
use log::{error, info};
use std::path::PathBuf;
use crate::settings_panel::{SettingsContext, SettingsDialog};

#[derive(Debug)]
enum AppState {
    Error(sysinfo::Error),
    SelectDisk(SelectTarget),
    Analyzing(Analyzer),
    Analyzed(ResultView),
}

impl DiskAnalyzerApp {
    pub(crate) fn new(_settings: Settings, initial_path: Option<PathBuf>, _ctx: &Context) -> Self {
        let settings = Settings::default();
        let state = match initial_path {
            Some(path) => {
                info!("CLI path provided: {path:?}, starting analysis immediately");
                AppState::Analyzing(Analyzer::new(path, settings.clone()))
            }
            None => SelectTarget::new(settings.clone())
                .map(SelectDisk)
                .unwrap_or_else(Error),
        };
        Self {
            settings,
            state,
            settings_context: SettingsContext::default(),
            #[cfg(target_os = "macos")]
            native_menu: match crate::macos_menu::NativeMenu::new(_ctx.clone()) {
                Ok(menu) => Some(menu),
                Err(err) => {
                    log::error!("Unable to create native menu: {err}");
                    None
                }
            },
        }
    }
}

#[derive(Debug)]
pub(crate) struct DiskAnalyzerApp {
    settings: Settings,
    state: AppState,
    settings_context: SettingsContext,
    #[cfg(target_os = "macos")]
    native_menu: Option<crate::macos_menu::NativeMenu>,
}

impl eframe::App for DiskAnalyzerApp {
    fn logic(&mut self, ctx: &Context, _: &mut Frame) {
        #[cfg(target_os = "macos")]
        if let Some(menu) = &self.native_menu
            && let Some(action) = menu.poll()
        {
            match action {
                crate::macos_menu::MenuAction::SelectDisk => {
                    info!("Select disk requested from native menu");
                    self.state = SelectTarget::new(self.settings.clone())
                        .map(SelectDisk)
                        .unwrap_or_else(Error);
                    ctx.request_repaint();
                }
                crate::macos_menu::MenuAction::Settings => {
                    info!("Open settings requested from native menu");
                    self.settings_context.open();
                    ctx.request_repaint();
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = ctx;
    }

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
                    self.state = SelectTarget::new(self.settings.clone())
                        .map(SelectDisk)
                        .unwrap_or_else(Error)
                }
                AnalyzerUpdate::Running => {}
            },
            AppState::Analyzed(result_view) => {
                if result_view.show(ui) {
                    info!("Back requested from ResultView, transitioning to SelectTarget");
                    self.state = SelectTarget::new(self.settings.clone())
                        .map(SelectDisk)
                        .unwrap_or_else(Error)
                }
            }
            AppState::Error(error) => error!("{error}"),
        }

        SettingsDialog::new(&mut self.settings_context, &self.settings).show_if_open(ui);

        if ui.ctx().input(|i| i.viewport().close_requested()) {
            self.settings.save().expect("Unable to save settings");
        }
    }
}

use crate::analysis_result::AnalysisResult;
use crate::settings::Settings;
use crate::ui::about_dialog::AboutDialog;
use crate::ui::path_bar::PathBar;
use crate::ui::treemap_panel::TreeMapPanel;
use egui::Context;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub(crate) struct ResultView {
    analysis_result: AnalysisResult,
    about_open: bool,
    settings: Arc<Mutex<Settings>>,
}

impl ResultView {
    pub const fn new(analysis_result: AnalysisResult, settings: Arc<Mutex<Settings>>) -> Self {
        Self {
            analysis_result,
            about_open: false,
            settings,
        }
    }
}

impl ResultView {
    pub(crate) fn show(&mut self, ctx: &Context) -> bool {
        let mut go_back = false;
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("⬅").clicked() {
                    go_back = true;
                }
                PathBar::new(&mut self.analysis_result).show(ui);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    AboutDialog::new(&mut self.about_open).show_button(ctx, ui);
                });
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            TreeMapPanel::new(&mut self.analysis_result, &self.settings, true).show(ui);
        });

        go_back
    }
}

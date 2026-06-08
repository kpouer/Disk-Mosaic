use crate::about_dialog::AboutDialog;
use crate::path_bar::PathBar;
use crate::settings::Settings;
use crate::treemap_panel::TreeMapPanel;
use disk_mosaic_core::analysis_result::AnalysisResult;
use egui::Ui;

#[derive(Debug)]
pub(crate) struct ResultView {
    analysis_result: AnalysisResult,
    about_open: bool,
    settings: Settings,
}

impl ResultView {
    pub(crate) const fn new(analysis_result: AnalysisResult, settings: Settings) -> Self {
        Self {
            analysis_result,
            about_open: false,
            settings,
        }
    }

    pub(crate) fn show(&mut self, ui: &mut Ui) -> bool {
        let mut go_back = false;
        egui::Panel::top("top_panel").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("⬅").clicked() {
                    go_back = true;
                }
                PathBar::new(&mut self.analysis_result).show(ui);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    AboutDialog::new(&mut self.about_open).show_button(ui);
                });
            });
        });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            TreeMapPanel::new(&mut self.analysis_result, &self.settings, true).show(ui);
        });

        go_back
    }
}

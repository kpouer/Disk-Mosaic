use crate::analysis_result::AnalysisResult;
use egui::{Button, Ui, Vec2};

#[derive(Debug)]
pub struct PathBar<'a> {
    analysis_result: &'a mut AnalysisResult,
}

impl<'a> PathBar<'a> {
    pub const fn new(analysis_result: &'a mut AnalysisResult) -> Self {
        Self { analysis_result }
    }

    // Return the index of the clicked component
    pub(crate) fn show(&mut self, ui: &mut Ui) {
        let mut clicked_index = None;
        ui.horizontal(|ui| {
            {
                let spacing = ui.spacing_mut();
                spacing.item_spacing.x = 0.0;
                spacing.button_padding = Vec2::ZERO;
            }
            self.analysis_result
                .data_stack
                .iter()
                .enumerate()
                .for_each(|(index, data)| {
                    let is_last = index == self.analysis_result.data_stack.len() - 1;
                    if ui
                        .add_enabled(!is_last, Button::new(format!("{}/", &data.name)))
                        .clicked()
                    {
                        clicked_index = Some(index);
                    }
                })
        });

        if let Some(index) = clicked_index {
            self.analysis_result.selected_index(index)
        } else if ui
            .ctx()
            .input(|i| i.pointer.button_clicked(egui::PointerButton::Extra1))
            && self.analysis_result.data_stack.len() >= 2
        {
            let index = self.analysis_result.data_stack.len() - 2;
            self.analysis_result.selected_index(index);
        }
    }
}

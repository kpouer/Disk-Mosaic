use crate::analysis_result::AnalysisResult;
use crate::data::Kind;
use crate::settings::Settings;
use crate::ui::data_widget::DataWidget;
use egui::{Event, Label, TextWrapMode, Tooltip, Ui, UiKind, Widget};
use humansize::DECIMAL;
use log::error;
use std::sync::{Arc, Mutex};
use treemap::{Mappable, TreemapLayout};

pub(crate) struct TreeMapPanel<'a> {
    analysis_result: &'a mut AnalysisResult,
    settings: &'a Arc<Mutex<Settings>>,
    can_zoom_in: bool,
}

impl<'a> TreeMapPanel<'a> {
    pub(crate) fn new(
        analysis_result: &'a mut AnalysisResult,
        settings: &'a Arc<Mutex<Settings>>,
        can_zoom_in: bool,
    ) -> Self {
        TreeMapPanel {
            analysis_result,
            settings,
            can_zoom_in,
        }
    }

    pub(crate) fn show(&mut self, ui: &mut Ui) {
        let clip_rect = ui.clip_rect();
        let rect = treemap::Rect::from_points(
            clip_rect.left() as f64,
            clip_rect.top() as f64,
            clip_rect.width() as f64,
            clip_rect.height() as f64,
        );
        let mut clicked_data_index = None;
        let hovered_data_index = None;
        let mut full_path = self.analysis_result.root_path.clone();
        for item in self.analysis_result.data_stack[1..].iter() {
            full_path.push(&item.name);
        }
        if let Some(current_data) = self.analysis_result.data_stack.last_mut()
            && let Kind::Dir(children) = &mut current_data.kind
        {
            TreemapLayout::new().layout_items(children, rect);
            children
                .iter()
                .enumerate()
                .filter(|(_, data)| data.bounds.w > 0.0 && data.bounds.h > 0.0)
                .for_each(|(index, data)| {
                    let mut show_context_menu = false;
                    let mut data_widget = DataWidget::new(data);
                    let response = data_widget.ui(ui);
                    let context_menu_opened = response.context_menu_opened();
                    if !context_menu_opened {
                        if response.double_clicked() && matches!(data.kind, Kind::Dir(_)) {
                            clicked_data_index = Some(index);
                        } else if response.secondary_clicked() {
                            show_context_menu = true;
                        }
                    }
                    if data_widget.need_tooltip && response.hovered() {
                        Tooltip::for_widget(&response).at_pointer().show(|ui| {
                            ui.heading(&data.name);
                            ui.separator();
                            ui.add(
                                Label::new(format!(
                                    "Size: {}",
                                    humansize::format_size(data.size() as u64, DECIMAL)
                                ))
                                .wrap_mode(TextWrapMode::Extend),
                            );
                        });
                    } else if context_menu_opened || show_context_menu {
                        let full_path = full_path.clone();
                        response.context_menu(|ui| {
                            ui.heading(&data.name);
                            ui.separator();
                            if ui.button("Browse...").clicked() {
                                let mut path = full_path.clone();
                                path.push(&data.name);
                                if let Err(e) = opener::reveal(path.clone()) {
                                    error!("Error opening file: {e}")
                                }
                                ui.close_kind(UiKind::Menu);
                            }
                            if ui.button("Copy full path").clicked() {
                                let mut path = full_path.clone();
                                path.push(&data.name);
                                let text = path.to_string_lossy().to_string();
                                ui.ctx().copy_text(text);
                                ui.close_kind(UiKind::Menu);
                            }
                            if ui.button("Ignore path").clicked() {
                                let mut path = full_path.clone();
                                path.push(&data.name);
                                let mut settings = self.settings.lock().unwrap();
                                settings.add_ignored_path(path);
                                ui.close_kind(UiKind::Menu);
                            }
                            if ui.button("Copy parent path").clicked() {
                                let text = full_path.to_string_lossy().to_string();
                                ui.ctx().copy_text(text);
                                ui.close_kind(UiKind::Menu);
                            }
                        });
                    }
                });
        }

        if let Some(clicked_index) = clicked_data_index {
            self.zoom_in(clicked_index);
        }

        ui.ctx().input(|i| {
            i.events.iter().for_each(|event| {
                if let Event::MouseWheel {
                    unit: _,
                    delta,
                    modifiers: _,
                } = event
                {
                    self.zoom(hovered_data_index, delta.y)
                }
            })
        });
    }

    fn zoom(&mut self, hovered_data_index: Option<usize>, delta: f32) {
        if delta > 0.0 && self.analysis_result.data_stack.len() >= 2 {
            let index = self.analysis_result.data_stack.len() - 2;
            self.analysis_result.selected_index(index);
        } else if delta < 0.0
            && let Some(hovered_index) = hovered_data_index
        {
            self.zoom_in(hovered_index);
        }
    }

    fn zoom_in(&mut self, index: usize) {
        if !self.can_zoom_in {
            return;
        }
        if let Some(parent_node) = self.analysis_result.data_stack.last_mut() {
            let Kind::Dir(children) = &mut parent_node.kind else {
                error!("The parent node is not a directory");
                return;
            };
            if let Some(data) = children.get(index)
                && !matches!(data.kind, Kind::Dir(_))
            {
                return;
            }
            let taken_data = children.swap_remove(index); // swap_remove because it is faster than a normal remove
            self.analysis_result.data_stack.push(taken_data);
        }
    }
}

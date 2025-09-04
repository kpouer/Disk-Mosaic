mod folder_list_panel;

use crate::settings::{ColorScheme, Settings, ThemePreference};
use crate::ui::settings_panel::folder_list_panel::SearchFolderPanel;
use egui::Context;
use humansize::DECIMAL;
use std::ops::Index;
use std::sync::{Arc, Mutex};
use strum::IntoEnumIterator;

pub(crate) struct SettingsDialog<'a> {
    settings_context: &'a mut SettingsContext,
    settings: &'a Arc<Mutex<Settings>>,
}

const GEAR: &str = "\u{2699}";

impl<'a> SettingsDialog<'a> {
    pub(crate) fn new(
        settings_context: &'a mut SettingsContext,
        settings: &'a Arc<Mutex<Settings>>,
    ) -> Self {
        Self {
            settings_context,
            settings,
        }
    }

    pub(crate) fn show_button(&mut self, ctx: &Context, ui: &mut egui::Ui) {
        if ui.button(GEAR).clicked() {
            self.settings_context.open = true;
        }
        if self.settings_context.open {
            self.show(ctx);
        }
    }

    fn show(&mut self, ctx: &Context) {
        let mut settings = self.settings.lock().unwrap();
        egui::Window::new("Settings")
            .open(&mut self.settings_context.open)
            .show(ctx, |ui| {
                egui::Grid::new("my_grid")
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Color scheme: ");
                        egui::ComboBox::from_id_salt("ColorScheme")
                            .selected_text(format!("{:?}", settings.color_scheme()))
                            .show_ui(ui, |ui| {
                                ColorScheme::iter().for_each(|scheme| {
                                    if ui
                                        .selectable_value(
                                            settings.color_scheme_mut(),
                                            scheme,
                                            format!("{scheme:?}"),
                                        )
                                        .clicked()
                                    {
                                        scheme.apply(ctx);
                                    }
                                });
                            });
                        ui.end_row();
                        ui.label("Theme: ");
                        ui.horizontal(|ui| {
                            if ui
                                .radio(settings.theme() == ThemePreference::System, "System")
                                .clicked()
                            {
                                settings.set_theme(ThemePreference::System);
                                ctx.set_theme(ThemePreference::System);
                            }
                            if ui
                                .radio(settings.theme() == ThemePreference::Dark, "Dark")
                                .clicked()
                            {
                                settings.set_theme(ThemePreference::Dark);
                                ctx.set_theme(ThemePreference::Dark);
                            }
                            if ui
                                .radio(settings.theme() == ThemePreference::Light, "Light")
                                .clicked()
                            {
                                settings.set_theme(ThemePreference::Light);
                                ctx.set_theme(ThemePreference::Light);
                            }
                        });
                        ui.end_row();
                        ui.label("Big file threshold :");
                        let response = ui.add(
                            egui::DragValue::new(&mut settings.big_file_threshold)
                                .speed(1_000_000.0) // 1MB
                                .custom_formatter(|size, _| {
                                    humansize::format_size(size as u64, DECIMAL)
                                }),
                        );
                        if response.changed() {
                            settings.dirty = true;
                        }
                        if response.hovered() {
                            response.show_tooltip_text("Smaller will be be grouped as a remaining group without showing details. \
                        Reducing this threshold allow to show them but will also reduce the performance and consume more memory.")
                        };
                        if ui.button("Default value").clicked() {
                            settings.reset_big_file_threshold();
                        }
                        ui.end_row();
                        ui.label("Ignore common cloud folders:");
                        let chk = ui.checkbox(&mut settings.ignore_cloud_mounts, "Automatically exclude Dropbox, OneDrive, Google Drive, iCloud, etc.");
                        if chk.changed() {
                            settings.dirty = true;
                        }
                        ui.end_row();
                    });
                let modified = SearchFolderPanel::with_title(
                    "ignored_folders",
                    "Ignored folders",
                    HashListPanel::new(
                        settings.ignored_paths_mut(),
                        &mut self.settings_context.ignored_folders_selection,
                    ),
                )
                .show(ui);
                if modified {
                    settings.dirty = true;
                }
            });
    }
}

struct HashListPanel<'a, T> {
    vec: &'a mut Vec<T>,
    selection: &'a mut Option<usize>,
    dirty: bool,
}

impl<T> HashListPanel<'_, T> {
    pub(crate) fn push(&mut self, item: T) {
        self.vec.push(item);
        self.dirty = true;
    }

    pub(crate) fn remove_selection(&mut self) {
        if let Some(selection) = *self.selection {
            self.vec.remove(selection);
            self.dirty = true;
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.vec.len()
    }
}

impl<T> Index<usize> for HashListPanel<'_, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.vec[index]
    }
}

impl<'a, T> HashListPanel<'a, T> {
    fn new(vec: &'a mut Vec<T>, selection: &'a mut Option<usize>) -> Self {
        Self {
            vec,
            selection,
            dirty: false,
        }
    }
}

#[derive(Default, Debug)]
pub(crate) struct SettingsContext {
    pub(crate) open: bool,
    pub(crate) ignored_folders_selection: Option<usize>,
}

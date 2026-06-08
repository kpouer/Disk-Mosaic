mod folder_list_panel;

use crate::settings::{ColorScheme, Settings, ThemePreference};
use crate::settings_panel::folder_list_panel::SearchFolderPanel;
use egui::{Response, Ui, Widget};
use humansize::DECIMAL;
use std::ops::Index;
use strum::IntoEnumIterator;

#[derive(Debug)]
pub(crate) struct SettingsDialog<'a> {
    settings_context: &'a mut SettingsContext,
    settings: &'a Settings,
}

const GEAR: &str = "\u{2699}";

impl<'a> SettingsDialog<'a> {
    pub(crate) const fn new(
        settings_context: &'a mut SettingsContext,
        settings: &'a Settings,
    ) -> Self {
        Self {
            settings_context,
            settings,
        }
    }

    pub(crate) fn show_button(&mut self, ui: &mut Ui) {
        if ui.button(GEAR).clicked() {
            self.settings_context.open = true;
        }
        if self.settings_context.open {
            self.show(ui);
        }
    }

    fn show(&mut self, ui: &mut Ui) {
        egui::Window::new("Settings")
            .open(&mut self.settings_context.open)
            .show(ui, |ui| {
                egui::Grid::new("my_grid")
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ColorSchemeWidget::new(self.settings).ui(ui);
                        ui.end_row();
                        ThemePreferenceWidget::new(self.settings).ui(ui);
                        ui.end_row();
                        ui.label("Big file threshold :");
                        let response = ui.add(
                            egui::DragValue::new(&mut *self.settings.big_file_threshold_mut())
                                .speed(1_000_000.0) // 1MB
                                .custom_formatter(|size, _| {
                                    humansize::format_size(size as u64, DECIMAL)
                                }),
                        );
                        if response.changed() {
                            self.settings.set_dirty(true);
                        }
                        if response.hovered() {
                            response.show_tooltip_text("Smaller will be be grouped as a remaining group without showing details. \
                        Reducing this threshold allow to show them but will also reduce the performance and consume more memory.")
                        };
                        if ui.button("Default value").clicked() {
                            self.settings.reset_big_file_threshold();
                        }
                        ui.end_row();
                        ui.label("Ignore common cloud folders:");
                        let chk = ui.checkbox(&mut *self.settings.ignore_cloud_mounts_mut(), "Automatically exclude Dropbox, OneDrive, Google Drive, iCloud, etc.");
                        if chk.changed() {
                            self.settings.set_dirty(true);
                        }
                        ui.end_row();
                    });
                let modified = SearchFolderPanel::with_title(
                    "ignored_folders",
                    "Ignored folders",
                    HashListPanel::new(
                        &mut *self.settings.ignored_paths_mut(),
                        &mut self.settings_context.ignored_folders_selection,
                    ),
                )
                .show(ui);
                if modified {
                    self.settings.set_dirty(true);
                }
            });
    }
}

struct ColorSchemeWidget<'a> {
    settings: &'a Settings,
}

impl<'a> ColorSchemeWidget<'a> {
    const fn new(settings: &'a Settings) -> Self {
        Self { settings }
    }
}

impl<'a> Widget for ColorSchemeWidget<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.label("Color scheme: ");
        egui::ComboBox::from_id_salt("ColorScheme")
            .selected_text(format!("{:?}", self.settings.color_scheme()))
            .show_ui(ui, |ui| {
                ColorScheme::iter().for_each(|scheme| {
                    if ui
                        .selectable_value(
                            &mut *self.settings.color_scheme_mut(),
                            scheme,
                            format!("{scheme:?}"),
                        )
                        .clicked()
                    {
                        scheme.apply(ui.ctx());
                    }
                });
            })
            .response
    }
}

struct ThemePreferenceWidget<'a> {
    settings: &'a Settings,
}

impl<'a> ThemePreferenceWidget<'a> {
    const fn new(settings: &'a Settings) -> Self {
        Self { settings }
    }
}

impl<'a> Widget for ThemePreferenceWidget<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.label("Theme: ");
        ui.horizontal(|ui| {
            let theme = self.settings.theme();
            for t in ThemePreference::iter() {
                if ui.radio(theme == t, t.to_string()).clicked() {
                    self.settings.set_theme(t);
                    ui.ctx().set_theme(t);
                }
            }
        })
        .response
    }
}

#[derive(Debug)]
struct HashListPanel<'a, T> {
    vec: &'a mut Vec<T>,
    selection: &'a mut Option<usize>,
    dirty: bool,
}

impl<T> HashListPanel<'_, T> {
    fn push(&mut self, item: T) {
        self.vec.push(item);
        self.dirty = true;
    }

    fn remove_selection(&mut self) {
        if let Some(selection) = *self.selection {
            self.vec.remove(selection);
            self.dirty = true;
        }
    }

    const fn len(&self) -> usize {
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
    const fn new(vec: &'a mut Vec<T>, selection: &'a mut Option<usize>) -> Self {
        Self {
            vec,
            selection,
            dirty: false,
        }
    }
}

#[derive(Default, Debug)]
pub(crate) struct SettingsContext {
    open: bool,
    ignored_folders_selection: Option<usize>,
}

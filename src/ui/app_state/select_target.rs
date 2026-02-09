use crate::service::storage_manager::StorageManager;
use crate::service::storage_manager::storage::Storage;
use crate::settings::{ColorScheme, Settings};
use crate::ui::about_dialog::AboutDialog;
use crate::ui::settings_panel::SettingsContext;
use crate::ui::settings_panel::SettingsDialog;
use crate::util::{FONT_SIZE, PathBufToString};
use egui::{Button, Color32, Context, Image, Response, Tooltip, Ui, Vec2, Widget, include_image};
use home::home_dir;
use humansize::DECIMAL;
use serde::de;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub(crate) struct SelectTarget {
    settings: Arc<Mutex<Settings>>,
    storage_manager: StorageManager,
    about_open: bool,
    settings_context: SettingsContext,
}

const HOME_FOLDER: &str = "Home Folder";

impl SelectTarget {
    pub(crate) fn new(settings: Arc<Mutex<Settings>>) -> Self {
        Self {
            settings_context: SettingsContext::default(),
            settings,
            storage_manager: StorageManager::default(),
            about_open: false,
        }
    }

    pub fn show(&mut self, ctx: &Context) -> Option<PathBuf> {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Select Scan Target");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    AboutDialog::new(&mut self.about_open).show_button(ctx, ui);
                    SettingsDialog::new(&mut self.settings_context, &self.settings)
                        .show_button(ctx, ui);
                });
            });
        });
        egui::CentralPanel::default()
            .show(ctx, |ui| {
                let mut selected_path = None;
                self.storage_manager.iter().for_each(|disk| {
                    if StorageWidget::new(disk, &self.settings).ui(ui).clicked() {
                        selected_path = Some(disk.mount_point.to_owned());
                    }
                });
                ui.separator();
                if let Some(home) = home_dir() {
                    let home_response = ui.add_sized(
                        Vec2::new(ui.available_width(), HEIGHT),
                        Button::image_and_text(
                            Image::new(include_image!("../../../assets/home.svg"))
                                .tint(icon_color(&self.settings))
                                .fit_to_exact_size(Vec2::new(HEIGHT, HEIGHT)),
                            HOME_FOLDER,
                        ),
                    );
                    if home_response.clicked() {
                        selected_path = Some(home);
                    } else if home_response.hovered() {
                        Tooltip::for_widget(&home_response).at_pointer().show(|ui| {
                            ui.horizontal(|ui| {
                                ui.add(
                                    Image::new(include_image!("../../../assets/home.svg"))
                                        .tint(icon_color(&self.settings))
                                        .fit_to_exact_size(Vec2::new(FONT_SIZE, FONT_SIZE)),
                                );
                                ui.heading(HOME_FOLDER);
                            });
                            ui.separator();
                            ui.label(home.absolute_path());
                        });
                    }
                }

                if ui
                    .add_sized(
                        Vec2::new(ui.available_width(), HEIGHT),
                        Button::image_and_text(
                            Image::new(include_image!("../../../assets/directory.svg"))
                                .tint(icon_color(&self.settings))
                                .fit_to_exact_size(Vec2::new(HEIGHT, HEIGHT)),
                            "Select Folder...",
                        ),
                    )
                    .clicked()
                {
                    selected_path = rfd::FileDialog::new().pick_folder();
                }

                selected_path
            })
            .inner
    }
}

#[derive(Debug)]
struct StorageWidget<'a> {
    storage: &'a Storage,
    settings: &'a Arc<Mutex<Settings>>,
}

impl<'a> StorageWidget<'a> {
    const fn new(storage: &'a Storage, settings: &'a Arc<Mutex<Settings>>) -> Self {
        Self { storage, settings }
    }
}

const HEIGHT: f32 = 48.0;

impl Widget for StorageWidget<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let image = egui::Image::new(self.storage.icon())
            .tint(icon_color(self.settings))
            .fit_to_exact_size(Vec2::new(HEIGHT, HEIGHT));
        let button = Button::image_and_text(image, self.storage.name());
        let response = ui.add_sized(Vec2::new(ui.available_width(), HEIGHT), button);
        if response.hovered() {
            Tooltip::for_widget(&response).at_pointer().show(|ui| {
                ui.heading(self.storage.name());
                ui.separator();
                ui.label(format!(
                    "Mount: {}",
                    self.storage.mount_point.absolute_path()
                ));
                ui.label(format!(
                    "{} / {}",
                    humansize::format_size(self.storage.total - self.storage.available, DECIMAL),
                    humansize::format_size(self.storage.total, DECIMAL)
                ));
                if self.storage.removable {
                    ui.label("Removable");
                }
            });
        }
        response
    }
}

fn icon_color(settings: &Arc<Mutex<Settings>>) -> Color32 {
    let theme = settings.lock().unwrap().color_scheme();
    match theme {
        ColorScheme::Egui => Color32::LIGHT_BLUE.linear_multiply(0.5),
        ColorScheme::Solarized => egui_solarized::BLUE,
    }
}

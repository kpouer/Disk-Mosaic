//! Menu actions shared by the native macOS menu and the in-window egui menu bar.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuAction {
    SelectDisk,
    Settings,
    #[cfg(not(target_os = "macos"))]
    Quit,
}

/// In-window menu bar used on platforms without a global native menu bar.
#[cfg(not(target_os = "macos"))]
pub(crate) fn show_menu_bar(ui: &mut egui::Ui) -> Option<MenuAction> {
    let mut action = None;
    egui::Panel::top("menu_bar").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Select Disk…").clicked() {
                    action = Some(MenuAction::SelectDisk);
                    ui.close();
                }
                if ui.button("Settings").clicked() {
                    action = Some(MenuAction::Settings);
                    ui.close();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    action = Some(MenuAction::Quit);
                    ui.close();
                }
            });
        });
    });
    action
}

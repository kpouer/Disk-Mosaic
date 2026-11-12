//! Native macOS menu bar built with `muda`.

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

pub(crate) enum MenuAction {
    SelectDisk,
    Settings,
}

pub(crate) struct NativeMenu {
    // Keeps the menu alive for the lifetime of the app.
    _menu: Menu,
    events: std::sync::mpsc::Receiver<MenuEvent>,
    select_disk_id: muda::MenuId,
    settings_id: muda::MenuId,
}

impl std::fmt::Debug for NativeMenu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeMenu").finish_non_exhaustive()
    }
}

impl NativeMenu {
    pub(crate) fn new(ctx: egui::Context) -> Result<Self, muda::Error> {
        // Forward menu events and wake the (otherwise idle) egui event loop.
        let (tx, events) = std::sync::mpsc::channel();
        MenuEvent::set_event_handler(Some(move |event| {
            let _ = tx.send(event);
            ctx.request_repaint();
        }));
        let menu = Menu::new();

        let app_menu = Submenu::new("Disk Mosaic", true);
        app_menu.append_items(&[
            &PredefinedMenuItem::about(
                None,
                Some(AboutMetadata {
                    name: Some("Disk Mosaic".into()),
                    version: Some(env!("CARGO_PKG_VERSION").into()),
                    authors: Some(vec!["Matthieu Casanova".into()]),
                    ..Default::default()
                }),
            ),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::services(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(None),
        ])?;

        let select_disk = MenuItem::new(
            "Select Disk…",
            true,
            Some(Accelerator::new(Modifiers::META, Code::KeyO)),
        );
        let settings = MenuItem::new(
            "Settings",
            true,
            None,
        );
        let file_menu = Submenu::new("File", true);
        file_menu.append_items(&[
            &select_disk,
            &PredefinedMenuItem::separator(),
            &settings,
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::close_window(None),
        ])?;

        let window_menu = Submenu::new("Window", true);
        window_menu.append_items(&[
            &PredefinedMenuItem::minimize(None),
            &PredefinedMenuItem::maximize(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::fullscreen(None),
        ])?;

        menu.append_items(&[&app_menu, &file_menu, &window_menu])?;
        menu.init_for_nsapp();
        window_menu.set_as_windows_menu_for_nsapp();

        Ok(Self {
            select_disk_id: select_disk.id().clone(),
            settings_id: settings.id().clone(),
            _menu: menu,
            events,
        })
    }

    /// Drains pending menu events and returns the first app-specific action.
    pub(crate) fn poll(&self) -> Option<MenuAction> {
        let mut action = None;
        while let Ok(event) = self.events.try_recv() {
            if event.id == self.select_disk_id {
                action = Some(MenuAction::SelectDisk);
            } else if event.id == self.settings_id {
                action = Some(MenuAction::Settings);
            }
        }
        action
    }
}

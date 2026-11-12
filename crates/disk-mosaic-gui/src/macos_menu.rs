//! Native macOS menu bar built with `muda`.

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

pub(crate) enum MenuAction {
    SelectDisk,
}

pub(crate) struct NativeMenu {
    // Keeps the menu alive for the lifetime of the app.
    _menu: Menu,
    select_disk_id: muda::MenuId,
}

impl std::fmt::Debug for NativeMenu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeMenu").finish_non_exhaustive()
    }
}

impl NativeMenu {
    pub(crate) fn new() -> Result<Self, muda::Error> {
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
        let file_menu = Submenu::new("File", true);
        file_menu.append_items(&[
            &select_disk,
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
            _menu: menu,
        })
    }

    /// Drains pending menu events and returns the first app-specific action.
    pub(crate) fn poll(&self) -> Option<MenuAction> {
        let mut action = None;
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.select_disk_id {
                action = Some(MenuAction::SelectDisk);
            }
        }
        action
    }
}

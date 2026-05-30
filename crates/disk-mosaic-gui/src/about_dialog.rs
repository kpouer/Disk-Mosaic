use egui::Ui;

#[derive(Debug)]
pub(crate) struct AboutDialog<'a> {
    open: &'a mut bool,
}

impl<'a> AboutDialog<'a> {
    pub(crate) const fn new(open: &'a mut bool) -> Self {
        Self { open }
    }

    pub(crate) fn show_button(&mut self, ui: &mut egui::Ui) {
        if ui.button("?").clicked() {
            *self.open = true;
        }
        if *self.open {
            self.show(ui);
        }
    }

    fn show(&mut self, ui: &Ui) {
        egui::Window::new("About Disk Mosaic")
            .open(self.open)
            .show(ui.ctx(), |ui| {
                ui.label("Disk Mosaic");
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.label("Created by Matthieu Casanova");
            });
    }
}

use disk_mosaic_core::color::Color;

pub(crate) trait ToEguiColor32 {
    fn to_egui_color32(self) -> egui::Color32;
}

impl ToEguiColor32 for Color {
    fn to_egui_color32(self) -> egui::Color32 {
        egui::Color32::from_rgb(self.r(), self.g(), self.b())
    }
}

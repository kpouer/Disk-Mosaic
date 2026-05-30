use disk_mosaic_core::data::Kind;

pub(crate) const fn get_image(kind: &Kind) -> egui::ImageSource<'_> {
    match kind {
        Kind::Dir(_) => egui::include_image!("../../../assets/directory.svg"),
        Kind::File => egui::include_image!("../../../assets/file.svg"),
        Kind::SmallFiles(_) => egui::include_image!("../../../assets/file.svg"),
    }
}
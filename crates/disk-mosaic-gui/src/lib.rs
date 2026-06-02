use settings::Settings;
use std::path::PathBuf;

mod about_dialog;
mod app_state;
mod color;
mod data;
mod data_widget;
mod disk_analyzer;
mod path_bar;
pub mod settings;
mod settings_panel;
mod storage_manager;
mod treemap_panel;

const FONT_SIZE: f32 = 18.0;

pub fn start(path: Option<PathBuf>) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("disk-mosaic")
            .with_icon(icon_data())
            .with_min_inner_size([320.0, 200.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Disk Mosaic",
        options,
        Box::new(move |ctx| {
            egui_extras::install_image_loaders(&ctx.egui_ctx);
            let settings = Settings::default();
            settings.init(&ctx.egui_ctx);
            Ok(Box::new(crate::disk_analyzer::DiskAnalyzerApp::new(
                settings, path,
            )))
        }),
    )
    .map_err(|e| e.to_string())
}

fn icon_data() -> egui::IconData {
    let app_icon_png_bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icon.png"));

    match eframe::icon_data::from_png_bytes(app_icon_png_bytes) {
        Ok(icon_data) => icon_data,
        Err(err) => panic!("Failed to load app icon: {err}"),
    }
}

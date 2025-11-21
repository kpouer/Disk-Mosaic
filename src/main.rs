#![windows_subsystem = "windows"]
mod analysis_result;
mod data;
mod disk_analyzer;
mod service;
mod settings;
mod task;
mod ui;
mod util;

use crate::settings::Settings;
use disk_analyzer::DiskAnalyzerApp;
use egui_extras::install_image_loaders;
use std::path::PathBuf;

fn main() -> eframe::Result {
    env_logger::init();
    // Parse optional CLI path argument: if provided and valid, start scanning immediately
    let initial_path: Option<PathBuf> = std::env::args().nth(1).map(PathBuf::from).and_then(|p| {
        if p.is_dir() {
            Some(p)
        } else {
            log::warn!("Path provided on CLI is not a readable directory: {:?}", p);
            None
        }
    });
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
            install_image_loaders(&ctx.egui_ctx);
            let settings = Settings::default();
            settings.init(&ctx.egui_ctx);
            Ok(Box::new(DiskAnalyzerApp::new(
                settings,
                initial_path.clone(),
            )))
        }),
    )
}

fn icon_data() -> egui::IconData {
    let app_icon_png_bytes = include_bytes!("../media/icon.png");

    match eframe::icon_data::from_png_bytes(app_icon_png_bytes) {
        Ok(icon_data) => icon_data,
        Err(err) => panic!("Failed to load app icon: {err}"),
    }
}

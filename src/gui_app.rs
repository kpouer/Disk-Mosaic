#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(feature = "gui"))]
compile_error!("You must activate at least one feature among `gui`.");

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    pub path: Option<PathBuf>,
}

fn main() -> Result<(), String> {
    env_logger::init();
    let args = Args::parse();
    disk_mosaic_gui::start(args.path)
}

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(feature = "gui"))]
compile_error!("You must activate at least one feature among `gui`.");

mod args;

use crate::args::Args;
use clap::Parser;

fn main() -> Result<(), String> {
    env_logger::init();
    let args = Args::parse();
    start_gui(&args)
}

pub(crate) fn start_gui(args: &Args) -> Result<(), String> {
    let initial_path = match &args.path {
        Some(p) if p.is_dir() => Some(p.to_owned()),
        _ => None,
    };
    disk_mosaic_gui::start(initial_path)
}

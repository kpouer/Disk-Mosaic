#![windows_subsystem = "windows"]

#[cfg(not(any(feature = "gui", feature = "tui")))]
compile_error!("You must activate at least one feature among `gui` or `tui`.");

mod args;
#[cfg(feature = "gui")]
mod gui_app;
#[cfg(feature = "tui")]
mod tui_app;

use crate::args::Args;
use clap::Parser;

fn main() -> Result<(), String> {
    env_logger::init();
    let args = Args::parse();

    #[cfg(feature = "tui")]
    if args.is_text_mode() {
        return tui_app::start_tui(&args);
    }

    #[cfg(feature = "gui")]
    return gui_app::start_gui(&args);

    #[cfg(not(feature = "gui"))]
    {
        Err("GUI feature is not enabled. Run with text mode enabled or build with the `gui` feature.".to_string())
    }
}

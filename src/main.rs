#![windows_subsystem = "windows"]

#[cfg(not(any(feature = "gui", feature = "tui")))]
compile_error!("You must activate at least one feature among `gui` or `tui`.");

mod args;

use clap::Parser;
use crate::args::Args;

fn main() -> Result<(), String> {
    env_logger::init();
    let args = Args::parse();

    #[cfg(feature = "tui")]
    {
        if args.is_text_mode() {
            let path = match &args.path {
                Some(p) if p.is_dir() => {
                    p.clone()
                }
                Some(p) => {
                    if let Some(parent) = p.parent() && parent.is_dir() {
                        parent.to_path_buf()
                    } else {
                        eprintln!("Error: Path provided is not a directory: {p:?}");
                        std::process::exit(1);
                    }
                }
                None => {
                    std::path::PathBuf::from(".")
                }
            };

            return disk_mosaic_tui::start(path);
        }
    }

    #[cfg(feature = "gui")]
    {
        let initial_path = match &args.path {
            Some(p) if p.is_dir() => Some(p.to_owned()),
            _ => None,
        };
        disk_mosaic_gui::start(initial_path)
    }

    #[cfg(not(feature = "gui"))]
    {
        Err("GUI feature is not enabled. Run with text mode enabled or build with the `gui` feature.".to_string())
    }
}

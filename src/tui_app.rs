#[cfg(any(feature = "gui", not(feature = "tui")))]
compile_error!("tui app is text only, do not activate gui feature. Use bin DiskMosaic instead");

mod args;

use crate::args::Args;
use clap::Parser;

fn main() -> Result<(), String> {
    env_logger::init();
    let args = Args::parse();
    let path = match &args.path {
        Some(p) if p.is_dir() => p.clone(),
        Some(p) => {
            if let Some(parent) = p.parent()
                && parent.is_dir()
            {
                parent.to_path_buf()
            } else {
                eprintln!("Error: Path provided is not a directory: {p:?}");
                std::process::exit(1);
            }
        }
        None => std::path::PathBuf::from("."),
    };

    disk_mosaic_tui::start(path)
}

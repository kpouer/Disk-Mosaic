#[cfg(any(feature = "gui", not(feature = "tui")))]
compile_error!("tui app is text only, do not activate gui feature. Use bin DiskMosaic instead");

use std::path::PathBuf;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    pub path: Option<PathBuf>,
}

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

    let path = std::fs::canonicalize(path).unwrap_or_else(|e| {
        eprintln!("Error: Path is invalid or inaccessible: {e}");
        std::process::exit(1);
    });

    disk_mosaic_tui::start(path)
}

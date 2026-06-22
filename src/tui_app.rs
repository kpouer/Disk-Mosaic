#[cfg(any(feature = "gui", not(feature = "tui")))]
compile_error!("tui app is text only, do not activate gui feature. Use bin DiskMosaic instead");

use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;
use clap::Parser;
use disk_mosaic_tui::settings::Settings;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    path: Option<PathBuf>,
    #[arg(short, long)]
    exclude: Vec<PathBuf>,
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

    let ignored_path = args.exclude
        .iter()
        .map(|f| f.canonicalize())
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    let settings = Settings {
        ignored_path: Arc::new(ignored_path)
    };
    disk_mosaic_tui::start(path, settings)
}

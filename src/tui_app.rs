use crate::args::Args;

pub(crate) fn start_tui(args: &Args) -> Result<(), String> {
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

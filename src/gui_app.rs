use clap::Parser;
use crate::args::Args;

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
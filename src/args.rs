use std::path::PathBuf;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub(crate) struct Args {
    #[cfg(feature = "tui")]
    #[arg(long)]
    text: bool,
    pub(crate) path: Option<PathBuf>,
}

#[cfg(feature = "tui")]
impl Args {
    pub(crate) fn is_text_mode(&self) -> bool {
        self.text || !cfg!(feature = "gui")
    }
}
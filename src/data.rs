use crate::util::PathBufToString;
use egui::{Color32, ImageSource, include_image};
use log::{error, warn};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use treemap::{Mappable, Rect};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Data {
    pub(crate) depth: u16,
    /// The name of the file or directory
    pub name: String,
    pub size: u64,
    pub bounds: treemap::Rect,
    pub color: Color32,
    pub kind: Kind,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Kind {
    Dir(Vec<Data>),
    File,
    SmallFiles(u64),
}

impl Default for Kind {
    fn default() -> Self {
        Self::Dir(Vec::new())
    }
}

impl Kind {
    pub const fn get_image(&self) -> ImageSource<'_> {
        match self {
            Kind::Dir(_) => include_image!("../assets/directory.svg"),
            Kind::File => include_image!("../assets/file.svg"),
            Kind::SmallFiles(_) => include_image!("../assets/file.svg"),
        }
    }
}

static INDEX: AtomicUsize = AtomicUsize::new(0);

impl Data {
    pub fn new_directory(path: &Path) -> Self {
        Self {
            name: path.name(),
            kind: Kind::default(),
            color: Self::next_color(),
            ..Default::default()
        }
    }

    pub fn new_file(path: &Path, size: u64) -> Self {
        Self {
            name: path.name(),
            kind: Kind::File,
            size,
            color: Self::next_color(),
            ..Default::default()
        }
    }

    pub(crate) fn next_color() -> Color32 {
        let idx = INDEX
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                Some((v + 1) % egui_solarized::ACCENT_COLORS.len())
            })
            .unwrap_or_else(|e| {
                warn!("AtomicUsize error: {e}");
                egui_solarized::ACCENT_COLORS.len()
            });
        egui_solarized::ACCENT_COLORS[idx]
    }

    pub(crate) fn push(&mut self, child: Data) {
        if let Kind::Dir(children) = &mut self.kind {
            children.push(child);
        } else {
            error!("Invalid kind ({self:?})");
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_nodes(&mut self, nodes: Vec<Data>) {
        self.size = Self::compute_size(&nodes);
        if let Kind::Dir(_) = &mut self.kind {
            self.kind = Kind::Dir(nodes);
        } else {
            error!("Invalid kind ({self:?})");
        }
    }

    fn compute_size(nodes: &[Data]) -> u64 {
        nodes.iter().fold(0, |acc, x| acc + x.size)
    }
}

impl Mappable for Data {
    fn size(&self) -> f64 {
        self.size as f64
    }

    fn bounds(&self) -> &Rect {
        &self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds
    }
}

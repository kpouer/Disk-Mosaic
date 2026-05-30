use log::info;
use std::path::PathBuf;
use crate::data::{Data, Kind};

#[derive(Debug, Default)]
pub struct AnalysisResult {
    pub root_path: PathBuf,
    pub data_stack: Vec<Data>,
}

impl AnalysisResult {
    pub const fn new(root_path: PathBuf, data_stack: Vec<Data>) -> AnalysisResult {
        Self {
            data_stack,
            root_path,
        }
    }

    pub fn selected_index(&mut self, index: usize) {
        while index < self.data_stack.len() - 1 {
            if let Some(popped_data) = self.data_stack.pop()
                && let Some(parent_data) = self.data_stack.last_mut()
            {
                if let Kind::Dir(children) = &mut parent_data.kind {
                    info!("Pushing {} into {}", popped_data.name, parent_data.name);
                    children.push(popped_data);
                } else {
                    log::error!("Invalid kind ({parent_data:?})");
                }
            }
        }
    }
}

use crate::data::{Data, Kind};
use log::info;
use std::path::PathBuf;

#[derive(Debug, Default)]
pub(crate) struct AnalysisResult {
    pub(crate) root_path: PathBuf,
    pub(crate) data_stack: Vec<Data>,
}

impl AnalysisResult {
    pub(crate) const fn new(root_path: PathBuf, data_stack: Vec<Data>) -> AnalysisResult {
        Self {
            data_stack,
            root_path,
        }
    }

    pub(crate) fn selected_index(&mut self, index: usize) {
        while index < self.data_stack.len() - 1 {
            if let Some(popped_data) = self.data_stack.pop() {
                if let Some(parent_data) = self.data_stack.last_mut() {
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
}

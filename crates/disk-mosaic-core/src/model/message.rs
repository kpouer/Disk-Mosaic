use crate::data::Data;
use crate::model::scan_result::ScanResult;

#[derive(Debug)]
pub enum Message {
    Data(Data),
    /// a single file, name and size
    File(String, u64),
    /// a directory scan started, this is recursive
    DirectoryScanStart(String),
    DirectoryScanDone(ScanResult),
}

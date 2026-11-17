use crate::data::Data;
use crate::model::scan_result::ScanResult;

#[derive(Debug)]
pub enum Message {
    /// A fully scanned top-level directory. It replaces any provisional entry with the same name
    /// (see [`Message::DirectoryProgress`]); a zero size means the entry must be removed.
    Data(Data),
    /// Provisional size of a top-level directory that is still being scanned, name and size
    DirectoryProgress(String, u64),
    /// a single file, name and size
    File(String, u64),
    /// a directory scan started, this is recursive
    DirectoryScanStart(String),
    DirectoryScanDone(ScanResult),
}

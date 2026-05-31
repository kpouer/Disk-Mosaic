use crate::data::Data;
use crate::model::scan_result::ScanResult;

#[derive(Debug)]
pub enum Message {
    Data(Data),
    DirectoryScanStart(String),
    DirectoryScanDone(ScanResult),
}

use std::ops::{Add, AddAssign};

#[derive(Debug, Default, Clone)]
pub struct ScanResult {
    pub file_count: u64,
    pub size: u64,
}

impl ScanResult {
    pub(crate) const fn add_size(&mut self, size: u64) {
        self.file_count += 1;
        self.size += size;
    }
}

impl Add for ScanResult {
    type Output = ScanResult;

    fn add(mut self, rhs: Self) -> Self::Output {
        self += rhs;
        self
    }
}

impl AddAssign for ScanResult {
    fn add_assign(&mut self, rhs: Self) {
        self.file_count += rhs.file_count;
        self.size += rhs.size;
    }
}
